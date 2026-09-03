#!/usr/bin/env bash
set -euo pipefail

export HOMEBREW_NO_AUTO_UPDATE=1

VERSION="${VERSION:-$(git describe --tags --always --dirty 2>/dev/null || printf 'dev')}"
VERSION="$(printf '%s' "$VERSION" | tr '/ ' '--')"

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_ROOT="${DIST_ROOT:-"$PROJECT_ROOT/dist"}"

STAGE="$DIST_ROOT/readtrace-$VERSION-macos-arm64"
ARCHIVE="$DIST_ROOT/readtrace-$VERSION-macos-arm64.tar.gz"

TARGET="aarch64-apple-darwin"
TESSDATA_REF="${TESSDATA_REF:-4.1.0}"

cd "$PROJECT_ROOT"


if [[ "$(uname -m)" != "arm64" ]]; then
  echo "This package must be built on an Apple Silicon runner (arm64)." >&2
  exit 1
fi


command -v brew >/dev/null || {
  echo "Homebrew is required." >&2
  exit 1
}


ensure_formula() {
  local formula="$1"

  if ! brew list --formula "$formula" >/dev/null 2>&1; then
    brew install "$formula"
  fi
}


ensure_formula dylibbundler
ensure_formula tesseract
ensure_formula poppler


BREW_PREFIX="$(brew --prefix)"
TESSERACT_PREFIX="$(brew --prefix tesseract)"
POPPLER_PREFIX="$(brew --prefix poppler)"


echo "Homebrew prefix:  $BREW_PREFIX"
echo "Tesseract prefix: $TESSERACT_PREFIX"
echo "Poppler prefix:   $POPPLER_PREFIX"


rustup target add "$TARGET"

cargo build \
  --release \
  --locked \
  --target "$TARGET"


rm -rf "$STAGE"

mkdir -p \
  "$STAGE/tools/tesseract" \
  "$STAGE/tools/poppler" \
  "$STAGE/LICENSES"


cp \
  "target/$TARGET/release/readtrace-cli" \
  "$STAGE/readtrace"

cp \
  "$TESSERACT_PREFIX/bin/tesseract" \
  "$STAGE/tools/tesseract/tesseract"

cp \
  "$POPPLER_PREFIX/bin/pdftoppm" \
  "$STAGE/tools/poppler/pdftoppm"

cp \
  "$POPPLER_PREFIX/bin/pdfinfo" \
  "$STAGE/tools/poppler/pdfinfo"


mkdir -p \
  "$STAGE/tools/tesseract/lib" \
  "$STAGE/tools/poppler/lib"


#
# dylibbundler cannot resolve Homebrew @rpath dependencies merely
# from the copied executable. Give it every relevant Homebrew lib
# directory explicitly.
#
DYLIB_SEARCH_ARGS=(
  -s "$BREW_PREFIX/lib"
  -s "$TESSERACT_PREFIX/lib"
  -s "$POPPLER_PREFIX/lib"
)


while IFS= read -r formula; do
  [[ -n "$formula" ]] || continue

  prefix="$(brew --prefix "$formula" 2>/dev/null || true)"

  if [[ -n "$prefix" && -d "$prefix/lib" ]]; then
    DYLIB_SEARCH_ARGS+=(
      -s "$prefix/lib"
    )
  fi

done < <(
  {
    printf '%s\n' tesseract poppler
    brew deps --formula tesseract
    brew deps --formula poppler
  } |
    awk 'NF && !seen[$0]++'
)


echo
echo "dylibbundler search paths:"

for ((i = 0; i < ${#DYLIB_SEARCH_ARGS[@]}; i += 2)); do
  printf '  %s\n' "${DYLIB_SEARCH_ARGS[i + 1]}"
done


echo
echo "Original Tesseract dependencies:"
otool -L "$STAGE/tools/tesseract/tesseract"

echo
echo "Original pdftoppm dependencies:"
otool -L "$STAGE/tools/poppler/pdftoppm"

echo
echo "Original pdfinfo dependencies:"
otool -L "$STAGE/tools/poppler/pdfinfo"


#
# Bundle Tesseract dependencies.
#
dylibbundler \
  -od \
  -b \
  "${DYLIB_SEARCH_ARGS[@]}" \
  -x "$STAGE/tools/tesseract/tesseract" \
  -d "$STAGE/tools/tesseract/lib" \
  -p '@executable_path/lib'


#
# IMPORTANT:
# pdftoppm and pdfinfo share one lib directory, therefore process
# both executables in ONE dylibbundler invocation.
#
# Using -od twice on the same destination would erase the output
# of the first invocation.
#
dylibbundler \
  -od \
  -b \
  "${DYLIB_SEARCH_ARGS[@]}" \
  -x "$STAGE/tools/poppler/pdftoppm" \
  -x "$STAGE/tools/poppler/pdfinfo" \
  -d "$STAGE/tools/poppler/lib" \
  -p '@executable_path/lib'


#
# dylibbundler modifies Mach-O binaries.
# Explicitly ad-hoc sign everything after modification so Apple
# Silicon does not reject modified code signatures.
#
sign_macho_directory() {
  local directory="$1"

  while IFS= read -r -d '' file_path; do
    if /usr/bin/file "$file_path" | grep -q 'Mach-O'; then
      codesign \
        --force \
        --sign - \
        "$file_path"
    fi
  done < <(
    find "$directory" -type f -print0
  )
}


sign_macho_directory "$STAGE/tools/tesseract/lib"
sign_macho_directory "$STAGE/tools/poppler/lib"


codesign \
  --force \
  --sign - \
  "$STAGE/tools/tesseract/tesseract"

codesign \
  --force \
  --sign - \
  "$STAGE/tools/poppler/pdftoppm"

codesign \
  --force \
  --sign - \
  "$STAGE/tools/poppler/pdfinfo"

codesign \
  --force \
  --sign - \
  "$STAGE/readtrace"


#
# Bundle only the OCR models ReadTrace actually needs.
# Installing tesseract-lang is unnecessary and huge.
#
mkdir -p "$STAGE/tools/tesseract/tessdata"

for language in chi_sim eng; do
  destination="$STAGE/tools/tesseract/tessdata/$language.traineddata"

  curl \
    --fail \
    --location \
    --silent \
    --show-error \
    "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/$TESSDATA_REF/$language.traineddata" \
    -o "$destination"
done


for language in chi_sim eng; do
  if [[ ! -f "$STAGE/tools/tesseract/tessdata/$language.traineddata" ]]; then
    echo "$language.traineddata is missing from the release." >&2
    exit 1
  fi
done


#
# Poppler also ships runtime data under share/.
#
if [[ -d "$POPPLER_PREFIX/share" ]]; then
  mkdir -p "$STAGE/tools/poppler/share"

  cp -R \
    "$POPPLER_PREFIX/share/." \
    "$STAGE/tools/poppler/share/"
fi


#
# Verify that the supposed self-contained binaries do not still
# refer to Homebrew or unresolved @rpath dependencies.
#
verify_no_external_links() {
  local binary="$1"
  local dependencies
  local bad_homebrew
  local bad_rpath

  dependencies="$(
    otool -L "$binary" |
      awk 'NR > 1 { print $1 }'
  )"

  bad_homebrew="$(
    printf '%s\n' "$dependencies" |
      grep -F "$BREW_PREFIX/" ||
      true
  )"

  bad_rpath="$(
    printf '%s\n' "$dependencies" |
      grep -E '^@rpath/' ||
      true
  )"

  if [[ -n "$bad_homebrew" || -n "$bad_rpath" ]]; then
    echo >&2
    echo "ERROR: non-portable dylib references remain in:" >&2
    echo "  $binary" >&2

    if [[ -n "$bad_homebrew" ]]; then
      echo "$bad_homebrew" >&2
    fi

    if [[ -n "$bad_rpath" ]]; then
      echo "$bad_rpath" >&2
    fi

    exit 1
  fi
}


verify_no_external_links \
  "$STAGE/tools/tesseract/tesseract"

verify_no_external_links \
  "$STAGE/tools/poppler/pdftoppm"

verify_no_external_links \
  "$STAGE/tools/poppler/pdfinfo"


while IFS= read -r -d '' library; do
  verify_no_external_links "$library"
done < <(
  find \
    "$STAGE/tools/tesseract/lib" \
    "$STAGE/tools/poppler/lib" \
    -type f \
    -name '*.dylib' \
    -print0
)


echo
echo "Bundled Tesseract dependencies:"
otool -L "$STAGE/tools/tesseract/tesseract"

echo
echo "Bundled pdftoppm dependencies:"
otool -L "$STAGE/tools/poppler/pdftoppm"

echo
echo "Bundled pdfinfo dependencies:"
otool -L "$STAGE/tools/poppler/pdfinfo"


#
# Actually execute the staged copies, not the Homebrew originals.
#
"$STAGE/tools/tesseract/tesseract" \
  --version \
  >/dev/null

"$STAGE/tools/poppler/pdftoppm" \
  -v \
  >/dev/null 2>&1

"$STAGE/tools/poppler/pdfinfo" \
  -v \
  >/dev/null 2>&1


TESSDATA_PREFIX="$STAGE/tools/tesseract/tessdata" \
  "$STAGE/tools/tesseract/tesseract" \
  --list-langs \
  >/tmp/readtrace-tesseract-langs.txt


grep -Fxq \
  'eng' \
  /tmp/readtrace-tesseract-langs.txt

grep -Fxq \
  'chi_sim' \
  /tmp/readtrace-tesseract-langs.txt

rm -f /tmp/readtrace-tesseract-langs.txt


#
# Licenses.
#
find \
  "$TESSERACT_PREFIX" \
  "$POPPLER_PREFIX" \
  -maxdepth 6 \
  \( \
    -iname 'LICENSE*' \
    -o -iname 'COPYING*' \
    -o -iname 'NOTICE*' \
  \) \
  -type f \
  -print \
  2>/dev/null |
while IFS= read -r license; do
  base="$(basename "$license")"
  parent="$(basename "$(dirname "$license")")"

  cp \
    "$license" \
    "$STAGE/LICENSES/${parent}-$base" \
    || true
done


cp \
  LICENSE \
  "$STAGE/LICENSES/readtrace-MIT.txt"


if [[ -d LICENSES ]]; then
  find \
    LICENSES \
    -maxdepth 1 \
    -type f \
    ! -name 'README.md' \
    -exec cp {} "$STAGE/LICENSES/" \;
fi


cp \
  THIRD_PARTY_NOTICES.md \
  "$STAGE/THIRD_PARTY_NOTICES.md"

cp \
  README.md \
  "$STAGE/README.md"

cp \
  docs/QUICK_START.md \
  "$STAGE/QUICK_START.md"

cp \
  .env.example \
  "$STAGE/.env.example"


#
# Gather version strings without pipefail/head/SIGPIPE surprises.
#
tesseract_output="$(
  "$STAGE/tools/tesseract/tesseract" --version 2>&1 ||
  true
)"

poppler_output="$(
  "$STAGE/tools/poppler/pdftoppm" -v 2>&1 ||
  true
)"

tesseract_version="${tesseract_output%%$'\n'*}"
poppler_version="${poppler_output%%$'\n'*}"

[[ -n "$tesseract_version" ]] || tesseract_version="unknown"
[[ -n "$poppler_version" ]] || poppler_version="unknown"


{
  printf '{\n'
  printf '  "version": "%s",\n' "$VERSION"
  printf '  "target": "macos-arm64",\n'
  printf '  "readtrace": "readtrace",\n'
  printf '  "tesseract": "%s",\n' "$tesseract_version"
  printf '  "poppler": "%s",\n' "$poppler_version"
  printf '  "tessdata_ref": "%s"\n' "$TESSDATA_REF"
  printf '}\n'
} > "$STAGE/release-manifest.json"


mkdir -p "$DIST_ROOT"

rm -f "$ARCHIVE"

COPYFILE_DISABLE=1 \
  tar \
    -czf "$ARCHIVE" \
    -C "$STAGE" \
    .


printf '%s\n' "$ARCHIVE"