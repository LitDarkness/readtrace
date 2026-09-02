#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-$(git describe --tags --always --dirty 2>/dev/null || printf 'dev')}"
VERSION="$(printf '%s' "$VERSION" | tr '/ ' '--')"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_ROOT="${DIST_ROOT:-"$PROJECT_ROOT/dist"}"
STAGE="$DIST_ROOT/readtrace-$VERSION-macos-arm64"
ARCHIVE="$DIST_ROOT/readtrace-$VERSION-macos-arm64.tar.gz"
TARGET="aarch64-apple-darwin"

cd "$PROJECT_ROOT"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "This package must be built on an Apple Silicon runner (arm64)." >&2
  exit 1
fi

command -v brew >/dev/null || { echo "Homebrew is required." >&2; exit 1; }
command -v dylibbundler >/dev/null || brew install dylibbundler

brew list tesseract >/dev/null 2>&1 || brew install tesseract
brew list tesseract-lang >/dev/null 2>&1 || brew install tesseract-lang
brew list poppler >/dev/null 2>&1 || brew install poppler

rustup target add "$TARGET"
cargo build --release --locked --target "$TARGET"

rm -rf "$STAGE"
mkdir -p "$STAGE/tools/tesseract" "$STAGE/tools/poppler" "$STAGE/LICENSES"

cp "target/$TARGET/release/readtrace-cli" "$STAGE/readtrace"
cp "$(brew --prefix tesseract)/bin/tesseract" "$STAGE/tools/tesseract/tesseract"
cp "$(brew --prefix poppler)/bin/pdftoppm" "$STAGE/tools/poppler/pdftoppm"
cp "$(brew --prefix poppler)/bin/pdfinfo" "$STAGE/tools/poppler/pdfinfo"

mkdir -p "$STAGE/tools/tesseract/lib" "$STAGE/tools/poppler/lib"
dylibbundler -od -b -x "$STAGE/tools/tesseract/tesseract" -d "$STAGE/tools/tesseract/lib" -p '@executable_path/lib'
dylibbundler -od -b -x "$STAGE/tools/poppler/pdftoppm" -d "$STAGE/tools/poppler/lib" -p '@executable_path/lib'
dylibbundler -od -b -x "$STAGE/tools/poppler/pdfinfo" -d "$STAGE/tools/poppler/lib" -p '@executable_path/lib'

TESSDATA_SOURCE="${TESSDATA_ROOT:-}"
if [[ -z "$TESSDATA_SOURCE" ]]; then
  for candidate in \
    "$(brew --prefix tesseract)/share/tessdata" \
    "$(brew --prefix)/share/tessdata" \
    "/opt/homebrew/share/tessdata"; do
    if [[ -d "$candidate" ]]; then
      TESSDATA_SOURCE="$candidate"
      break
    fi
  done
fi
if [[ -z "$TESSDATA_SOURCE" || ! -d "$TESSDATA_SOURCE" ]]; then
  echo "Could not find Homebrew tessdata. Pass TESSDATA_ROOT=/path/to/tessdata." >&2
  exit 1
fi
mkdir -p "$STAGE/tools/tesseract/tessdata"
cp "$TESSDATA_SOURCE"/*.traineddata "$STAGE/tools/tesseract/tessdata/" 2>/dev/null || true
for language in chi_sim eng; do
  if [[ ! -f "$STAGE/tools/tesseract/tessdata/$language.traineddata" ]]; then
    curl --fail --location --silent --show-error \
      "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/4.1.0/$language.traineddata" \
      -o "$STAGE/tools/tesseract/tessdata/$language.traineddata"
  fi
done
[[ -f "$STAGE/tools/tesseract/tessdata/chi_sim.traineddata" ]] || {
  echo "chi_sim.traineddata is missing from the release." >&2
  exit 1
}

if [[ -d "$(brew --prefix poppler)/share" ]]; then
  cp -R "$(brew --prefix poppler)/share" "$STAGE/tools/poppler/share"
fi

find "$(brew --prefix tesseract)" "$(brew --prefix poppler)" -maxdepth 6 \
  \( -iname 'LICENSE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) \
  -type f -print 2>/dev/null | while IFS= read -r license; do
    base="$(basename "$license")"
    parent="$(basename "$(dirname "$license")")"
    cp "$license" "$STAGE/LICENSES/${parent}-$base" || true
  done

cp LICENSE "$STAGE/LICENSES/readtrace-MIT.txt"
cp THIRD_PARTY_NOTICES.md "$STAGE/THIRD_PARTY_NOTICES.md"
cp README.md "$STAGE/README.md"
cp docs/QUICK_START.md "$STAGE/QUICK_START.md"
cp .env.example "$STAGE/.env.example"

{
  printf '{\n'
  printf '  "version": "%s",\n' "$VERSION"
  printf '  "target": "macos-arm64",\n'
  printf '  "readtrace": "readtrace",\n'
  printf '  "tesseract": "%s",\n' "$("$STAGE/tools/tesseract/tesseract" --version 2>&1 | head -n 1)"
  printf '  "poppler": "%s",\n' "$("$STAGE/tools/poppler/pdftoppm" -v 2>&1 | head -n 1)"
  printf '  "tessdata_ref": "4.1.0"\n'
  printf '}\n'
} > "$STAGE/release-manifest.json"

mkdir -p "$DIST_ROOT"
rm -f "$ARCHIVE"
tar -czf "$ARCHIVE" -C "$STAGE" .
printf '%s\n' "$ARCHIVE"
