# Third-party notices

ReadTrace itself is released under the MIT License in `LICENSE`. Release
archives may also contain external OCR programs and language data. Those files
remain under their own licenses; this notice does not relicense them.

## Tesseract and Leptonica

The Tesseract executable and its accompanying libraries are distributed under
the licenses shipped by the selected Tesseract build. Tesseract is Apache 2.0;
Leptonica and any other bundled dependency keep their own notices. The
`tessdata` files (`chi_sim.traineddata`, `eng.traineddata`, and any additional
language data) are separate data files and retain the license from their
upstream repository.

Upstream references:

- Tesseract: <https://github.com/tesseract-ocr/tesseract>
- Tesseract installation and language data: <https://tesseract-ocr.github.io/tessdoc/Installation.html>
- Official tessdata repositories: <https://github.com/tesseract-ocr/tessdata>

## Poppler

The optional Poppler tools (`pdfinfo`, `pdftoppm`) and their libraries retain
the GPL and other notices shipped by the exact Poppler build. A release archive
must include the build's `COPYING` files and a way to obtain the corresponding
source. Do not treat Poppler as part of the MIT-licensed ReadTrace code.

Upstream references:

- Poppler project: <https://poppler.freedesktop.org/>
- Poppler source: <https://gitlab.freedesktop.org/poppler/poppler>
- Windows prebuilt distribution used by the packaging workflow:
  <https://github.com/oschwartz10612/poppler-windows>

## Rust dependencies

ReadTrace links its Rust dependencies into the application binary. Their
exact versions are pinned in `Cargo.lock`; the crates used by this workspace
are distributed under their own MIT, Apache-2.0, BSD, ISC, Unicode-DFS and
similar notices. The source repository keeps the lockfile and package
metadata so that a recipient can audit those licenses. The OCR runtime files
are the only native tools copied into the release archive.

## Release obligations

The packaging scripts copy license files found beside the downloaded tools and
write a `release-manifest.json` with the tool versions and source URLs. Before
publishing a release, inspect `LICENSES/` in the archive and keep the exact
binary versions recorded in the manifest. If a vendor package has additional
terms, add them to the archive instead of assuming that this notice is enough.
