# ReadTrace 发布包与 GitHub Release

本文说明第一阶段的发布约定，以及从本仓库生成和发布压缩包的完整流程。发布包是“解压即用”的目录，不制作系统安装器，也不把 API Key 写进包内。

## 发布目标

每个版本生成两个压缩包：

| 文件 | 目标设备 | 内容 |
| --- | --- | --- |
| `readtrace-<版本>-windows-x86_64.zip` | Windows 10/11 x86_64 | `readtrace.exe`、Tesseract、`chi_sim`/`eng` 语言数据、Poppler、许可证和使用说明 |
| `readtrace-<版本>-macos-arm64.tar.gz` | Apple Silicon macOS（M1/M2/M3/M4） | `readtrace`、Tesseract、`chi_sim`/`eng` 语言数据、Poppler、动态库、许可证和使用说明 |

解压目录中的 `readtrace`/`readtrace.exe` 会从可执行文件旁边的 `tools/` 查找 OCR 工具，因此不要求用户把 Tesseract 或 Poppler 加到系统 PATH。Rust 依赖已经编译进可执行文件，用户不需要另外安装 Rust；Windows 构建还静态链接 MSVC CRT。压缩包同时提供不含密钥的 `.env.example`，仍可用 `.env` 或 GUI 中的 Provider 设置覆盖默认路径。

ReadTrace 自身使用 MIT 许可证。Tesseract、Leptonica、语言数据和 Poppler 继续使用各自的上游许可证；压缩包中的 `LICENSES/` 和 `THIRD_PARTY_NOTICES.md` 必须随程序一起发布，不能把第三方组件重新标成 MIT。详见 [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md)。

## 维护者：本地生成压缩包

以下命令都在仓库根目录执行，即包含 `Cargo.toml` 的 `summer_project` 目录，不是在 Vault 目录中执行。

### Windows x86_64

1. 安装 Rust stable、Git、[Tesseract](https://tesseract-ocr.github.io/tessdoc/Installation.html) 和包含 `pdftoppm.exe`/`pdfinfo.exe` 的 [Poppler Windows 构建](https://github.com/oschwartz10612/poppler-windows/releases)。
2. 在 PowerShell 中确认两个程序可发现：

   ```powershell
   tesseract --version
   pdftoppm -v
   pdfinfo -v
   ```

3. 生成包。脚本会自动补齐缺少的 `chi_sim.traineddata` 和 `eng.traineddata`：

   ```powershell
   .\scripts\package-release.ps1 -Version v0.1.0
   ```

   如果工具没有加入 PATH，可传绝对路径：

   ```powershell
   .\scripts\package-release.ps1 `
     -Version v0.1.0 `
     -TesseractBin 'C:\Program Files\Tesseract-OCR\tesseract.exe' `
     -PopplerBin 'C:\tools\poppler\Library\bin\pdftoppm.exe'
   ```

输出在 `dist/readtrace-v0.1.0-windows-x86_64.zip`。解压后可从其它目录启动：

```powershell
Set-Location C:\Users\me
$app = 'C:\Apps\readtrace-v0.1.0-windows-x86_64\readtrace.exe'
& $app workspace-init 'C:\Users\me\ReadTrace\workspace'
& $app vault-create 'C:\Users\me\ReadTrace\workspace' default
& $app serve 'C:\Users\me\ReadTrace\workspace' --bind 127.0.0.1:8787
```

### macOS arm64

在 Apple Silicon Mac 上执行：

```bash
brew install rust tesseract tesseract-lang poppler dylibbundler
VERSION=v0.1.0 bash scripts/package-release-macos.sh
```

脚本会用 `dylibbundler` 收集 Tesseract 和 Poppler 需要的动态库，并把它们放在压缩包的 `tools/*/lib` 目录。输出在 `dist/readtrace-v0.1.0-macos-arm64.tar.gz`。

用户解压后可直接启动：

```bash
tar -xzf readtrace-v0.1.0-macos-arm64.tar.gz
cd readtrace-v0.1.0-macos-arm64
chmod +x readtrace
./readtrace workspace-init "$HOME/ReadTrace/workspace"
./readtrace vault-create "$HOME/ReadTrace/workspace" default
./readtrace serve "$HOME/ReadTrace/workspace" --bind 127.0.0.1:8787
```

如果语言数据在自定义目录，可这样指定：

```bash
VERSION=v0.1.0 TESSDATA_ROOT=/path/to/tessdata bash scripts/package-release-macos.sh
```

## GitHub Actions 自动构建

仓库已包含 [`.github/workflows/release.yml`](../.github/workflows/release.yml)：

- 推送 `v*` 标签时，先在 Ubuntu 上执行格式检查、测试和 Clippy；
- 在 `windows-latest` 上安装 Tesseract/Poppler 并构建 Windows 包；
- 在 `macos-14` Apple Silicon runner 上构建 macOS arm64 包；
- 两个包都通过 GitHub Actions artifact 汇总；
- 只有标签触发的运行会自动创建 GitHub Release 并上传压缩包；手动触发只生成可下载的 Actions artifacts，不会误建一个名为 `main` 的 Release。

### 第一次配置仓库

1. 在 GitHub 创建一个空仓库，例如 `readtrace`，不要让 GitHub 再生成 README、License 或 `.gitignore`。
2. 在本地仓库确认远端并推送源代码：

   ```powershell
   git remote add origin https://github.com/<你的账号>/readtrace.git
   git push -u origin main
   ```

   如果 `origin` 已经存在，使用 `git remote -v` 检查地址，不要重复添加。
3. GitHub Actions 默认的 `GITHUB_TOKEN` 具有写 Release 所需的 `contents: write` 权限；工作流只在 `publish` job 中启用该权限，不需要把个人 Token 写入仓库。相关机制见 [GitHub Actions 的 `GITHUB_TOKEN` 文档](https://docs.github.com/en/actions/concepts/security/github_token)。

### 发布一个版本

在确认工作区干净、文档和版本号无误后：

```powershell
git status
git tag -a v0.1.0 -m "ReadTrace v0.1.0"
git push origin v0.1.0
```

打开 GitHub 仓库的 **Actions** 页面，等待 `Release ReadTrace` 完成。成功后在 **Releases** 页面会看到：

- `readtrace-v0.1.0-windows-x86_64.zip`
- `readtrace-v0.1.0-macos-arm64.tar.gz`
- `SHA256SUMS.txt`

每个包还包含 `release-manifest.json`，记录构建目标、Tesseract/Poppler 版本和语言数据引用，方便核对发布内容。

### 只想测试构建

在 Actions 页面选择 **Release ReadTrace → Run workflow**。这会运行测试并生成两个 artifacts，但不会创建 Release。下载 artifact 后，先解压，再按 [`QUICK_START.md`](QUICK_START.md) 启动 Web 工作台。

## 用户如何使用发布包

1. 下载与设备匹配的压缩包并解压到用户有写权限的目录。
2. 在一个单独的可写目录创建 Workspace 和第一个 Vault，然后运行 `readtrace serve <workspace>`；上面的平台示例可以直接复制。GUI 会在浏览器中打开；Provider（HTTP、Codex CLI、Mock）在 GUI 的设置页中配置，Key 保存在本机用户目录，不进入压缩包和 Git。
3. 在 GUI 中创建或选择 Workspace/Vault。导入的素材和运行数据留在 Vault；发布包目录只保存程序和第三方运行时。
4. Windows 用户不需要再安装 `tesseract`、`tesseract-lang` 或 Poppler。若要使用系统版本，可在 `.env` 中显式设置 `READTRACE_TESSERACT_BIN`、`READTRACE_PDFTOPPM_BIN` 和 `READTRACE_PDFINFO_BIN`。
5. macOS 首次打开未签名二进制时，若系统提示阻止，可在“系统设置 → 隐私与安全性”中允许本次打开；这不是安装器行为，发布包不会修改系统目录。

## 发布前检查表

- [ ] `cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。
- [ ] `.env`、Vault、`runtime/`、`workspace/` 和测试素材没有被 Git 跟踪。
- [ ] 两个压缩包都含 `readtrace`、`tools/tesseract/tessdata/chi_sim.traineddata`、`tools/tesseract/tessdata/eng.traineddata`、Poppler 可执行文件、`LICENSES/` 和 `THIRD_PARTY_NOTICES.md`。
- [ ] 在一台没有预装 Tesseract/Poppler 的干净设备上，导入一个 PNG 和一个 PDF，确认 OCR 能运行。
- [ ] 在 GUI 中配置 Provider 后完成一次 OCR、clean 发布、检索和引用问答流程。
- [ ] Release 页面同时提供源码仓库链接和第三方许可证说明。

## 版本回滚与重新发布

GitHub Release 创建后，如果发现包有问题，不要覆盖同一版本号。修复后递增补丁版本（例如 `v0.1.1`）再打标签发布。这样用户可以保留旧包，问题也能对应到明确的 `release-manifest.json`。
