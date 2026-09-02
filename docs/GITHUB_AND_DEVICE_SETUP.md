# GitHub、Windows 与 macOS 使用说明

本文把两类数据分开处理：GitHub 只保存 ReadTrace 的源代码和可公开配置模板；导入的原素材、Vault、运行日志、会话和 API Key 放在本机或单独备份中。这样不会把密钥推到远端，也不会让一次测试把仓库变成数据仓库。

除非单独标为 PowerShell 或 zsh/bash，命令都从项目根目录执行，并且适用于 Windows 与 macOS。文中的 `<项目目录>`、`<仓库>` 和 `<用户名>` 必须替换为本机实际值；不要照抄旧电脑的 `E:` 盘路径到 Mac。

## 一、首次上传到 GitHub

### 1. 提交前检查

下面的 Git 命令与平台无关：

```console
git status --short
git status --short --ignored
git check-ignore -v .env workspace runtime tmp deliverables
```

`.env`、`workspace/`、`runtime/`、`tmp/`、`deliverables/` 和作业 PDF 已加入 `.gitignore`。它们可以留在本机，但不要使用 `git add -f .env` 强制提交。

仓库中应提交的内容主要是：

- `crates/`、`Cargo.toml`、`Cargo.lock`：Rust 源代码和依赖锁定文件；
- `docs/`、`README.md`、`ARCHITECTURE*.md`、`CONTEXT.md`：设计与使用说明；
- `.env.example`、`prompts/`、`scripts/`、`tests/README.md`：公开模板和可复现脚本。

如果当前仓库还没有提交，可以执行：

```console
git add .
git diff --cached --stat
git diff --cached --check
git commit -m "feat: publish ReadTrace pipeline and web workspace"
git branch -M main
```

`git diff --cached --stat` 中不应出现 `.env`、图片、PDF、`workspace/` 或 `target/`。如果出现了，先用 `git restore --staged <路径>` 移出暂存区，再检查 `.gitignore`。

### 2. 绑定 GitHub 远端并推送

先在 GitHub 创建空仓库，不自动添加 README、License 或 `.gitignore`，然后执行：

```console
git remote add origin https://github.com/<账号>/<仓库>.git
# 如果 origin 已存在，改用：
# git remote set-url origin https://github.com/<账号>/<仓库>.git
git remote -v
git push -u origin main
```

若远端不是空仓库，不要直接覆盖历史。先备份本地提交，再根据远端内容决定是否 `git pull --rebase`，或重新创建空仓库。

## 二、Windows 与 macOS 全新安装

### 1. 共同依赖

需要：

1. Git；
2. Rust stable（包含 `cargo`，推荐通过 [rustup](https://rustup.rs/) 安装）；
3. 图片 OCR 所需的 Tesseract；
4. PDF 所需的 Poppler，其中必须包含 `pdfinfo` 和 `pdftoppm`；
5. 若使用 Codex Provider，还要安装并登录能在终端执行的 Codex CLI。Codex Desktop GUI 本身不等于 CLI。

TXT/Markdown 的导入、Mock 流程和本地检索不要求 Tesseract/Poppler。

### 2. macOS 安装

先安装 Homebrew，再安装 OCR 依赖：

```bash
brew install tesseract tesseract-lang poppler
```

`tesseract-lang` 提供 `chi_sim` 等额外语言数据。安装后在同一个 Terminal 中检查：

```bash
command -v cargo
command -v tesseract
command -v pdftoppm
command -v pdfinfo
tesseract --list-langs
command -v codex
```

`command -v codex` 只有在使用 Codex Provider 时才必须有结果。Apple Silicon Homebrew 通常位于 `/opt/homebrew`，Intel Homebrew 通常位于 `/usr/local`。

### 3. Windows 安装

如果旧 Windows 环境已经运行成功，优先保留原来的 Tesseract/Poppler 安装目录。新设备可按 [Tesseract 官方安装说明](https://tesseract-ocr.github.io/tessdoc/Installation.html) 安装带中文语言包的 Windows 构建，并安装包含 `pdfinfo.exe`、`pdftoppm.exe` 的 Poppler Windows 构建。

在 PowerShell 检查：

```powershell
Get-Command cargo
Get-Command tesseract -ErrorAction SilentlyContinue
Get-Command pdftoppm -ErrorAction SilentlyContinue
Get-Command pdfinfo -ErrorAction SilentlyContinue
Get-Command codex -ErrorAction SilentlyContinue
```

如果 OCR 工具不在 `PATH`，不必修改系统级 PATH；在项目 `.env` 中写绝对路径即可。

### 4. 克隆并构建

Windows PowerShell：

```powershell
git clone https://github.com/<账号>/<仓库>.git
Set-Location .\<仓库>
cargo build --workspace
cargo test --workspace
```

macOS zsh/bash：

```bash
git clone https://github.com/<账号>/<仓库>.git
cd ./<仓库>
cargo build --workspace
cargo test --workspace
```

## 三、创建本机 `.env`

`.env.example` 是跨平台模板。复制后只在本机编辑，不提交真实 Key。

Windows PowerShell：

```powershell
Copy-Item .env.example .env
notepad .env
```

macOS zsh/bash：

```bash
cp .env.example .env
open -e .env
```

### 1. GLM/OpenAI-compatible Provider

智谱官方 API 示例：

```dotenv
READTRACE_BASE_URL=https://open.bigmodel.cn/api/paas/v4
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.2
READTRACE_API_KEY_ENV=GLM_API_KEY
GLM_API_KEY=<只保存在本机>
READTRACE_THINKING_MODE=none
READTRACE_USD_TO_CNY=6.8
```

学校或私有 OpenAI-compatible 网关只需换 `READTRACE_BASE_URL`、Key 变量名和必要的协议字段。

截至 2026-09-02，[Z.ai 官方价格页](https://docs.z.ai/guides/overview/pricing)公布的 GLM‑5.2 API 价格为：

| 计费项 | 美元/百万 Token |
| --- | ---: |
| 输入 | `$1.40` |
| 缓存输入 | `$0.26` |
| 输出 | `$4.40` |

程序已内置这组价格及版本 `zai-model-pricing-2026-09-02`；将三项 `READTRACE_*_PRICE` 保持为 `0` 即可自动应用。若学校网关有不同结算价，应同时填写其真实的输入、缓存输入和输出价格，并用 `READTRACE_PRICING_VERSION` 标记来源，避免把 Z.ai 公开 API 价误当成学校账单价。

### 2. OCR 路径

Apple Silicon Mac：

```dotenv
READTRACE_TESSERACT_BIN=/opt/homebrew/bin/tesseract
READTRACE_PDFTOPPM_BIN=/opt/homebrew/bin/pdftoppm
READTRACE_PDFINFO_BIN=/opt/homebrew/bin/pdfinfo
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
```

Intel Mac 把 `/opt/homebrew` 改为 `/usr/local`。Homebrew 通常能自动找到语言目录，不要预先设置 `TESSDATA_PREFIX`；只有 `ocr-check` 或 `tesseract --list-langs` 缺少 `chi_sim` 时，才按实际路径填写 `/opt/homebrew/share/tessdata` 或 `/usr/local/share/tessdata`。

Windows 示例：

```dotenv
READTRACE_TESSERACT_BIN=C:/Program Files/Tesseract-OCR/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/Program Files/Tesseract-OCR/tessdata
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
```

Windows 的 `.env` 推荐使用正斜杠。路径可以包含空格；不要把示例中的目录当成固定安装位置。

检查 ReadTrace 实际解析到的程序：

```console
cargo run --quiet -p readtrace-cli -- ocr-check
```

输出中的 `tesseract_available`、`pdftoppm_available`、`pdfinfo_available` 应为 `true`，并且 `ocr_languages` 应包含 `chi_sim+eng`。

### 3. Codex CLI 与本机 Provider 配置

如果 `codex` 已在启动 ReadTrace 的同一个终端 `PATH` 中，保留默认值即可：

```dotenv
READTRACE_CODEX_BIN=codex
```

Windows 自动发现失败时可改成实际的 `.exe`、`.cmd` 或 `.ps1` 路径，例如：

```dotenv
READTRACE_CODEX_BIN=C:/Users/<用户名>/AppData/Local/OpenAI/Codex/bin/<版本目录>/codex.exe
```

Web“来源与 API”中的 Key 默认保存在：

- Windows：`%LOCALAPPDATA%/ReadTrace/providers.json`；
- macOS：`~/Library/Application Support/ReadTrace/providers.json`；
- 用户目录不可写时：当前 Vault 的 `.readtrace/providers.json`。

可用 `READTRACE_PROVIDER_STORE` 指定其它绝对路径。不要复制旧设备的 `auth.json`；在新设备重新登录 Codex CLI、重新填写 Provider Key。

### 4. Provider 检查

```console
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ai-check --provider http --model glm-5.2 --thinking none
```

`provider-check` 只显示 Key 是否存在，不返回 Key 内容。`ai-check` 会产生一次真实 API 请求并写入调用台账，因此可能计费。

## 四、创建 Workspace 和 Vault

Windows PowerShell：

```powershell
$workspace = ".\workspace"
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
cargo run --quiet -p readtrace-cli -- vault-create $workspace "我的资料"
$vault = cargo run --quiet -p readtrace-cli -- vault-path $workspace "我的资料"
cargo run --quiet -p readtrace-cli -- ls $vault
```

macOS zsh/bash：

```bash
workspace="./workspace"
cargo run --quiet -p readtrace-cli -- workspace-init "$workspace"
cargo run --quiet -p readtrace-cli -- vault-create "$workspace" "我的资料"
vault="$(cargo run --quiet -p readtrace-cli -- vault-path "$workspace" "我的资料")"
cargo run --quiet -p readtrace-cli -- ls "$vault"
```

导入、OCR、修复、构建和问答的详细命令见 [`CLI_TUTORIAL.md`](CLI_TUTORIAL.md)。现有 [`CLI_END_TO_END_EXAMPLE.md`](CLI_END_TO_END_EXAMPLE.md) 和 [`COMPLETE_FLOW_TUTORIAL.md`](COMPLETE_FLOW_TUTORIAL.md) 保留已验证的 Windows PowerShell 复现步骤；macOS 使用上面的变量语法、正斜杠路径和反斜杠 `\` 续行，CLI 参数本身完全相同。

## 五、把已有 Vault 搬到另一台设备

推荐使用 ReadTrace 自己的 `export-vault` 命令。它复制 sources、raw、generated、clean、prompts、events、sessions 和运行台账，但不会复制本机 Provider Key。

Windows PowerShell 导出：

```powershell
cargo run --quiet -p readtrace-cli -- export-vault `
  "E:/AI_diary/summer_project/workspace/vaults/first_run" `
  "E:/ReadTraceBackup/first_run"
```

macOS zsh/bash 导出：

```bash
cargo run --quiet -p readtrace-cli -- export-vault \
  "$HOME/Documents/readtrace/workspace/vaults/first_run" \
  "$HOME/ReadTraceBackup/first_run"
```

通过硬盘、NAS 或其它私有方式复制备份；不要把含 `sources/` 的 Vault 上传到公开 GitHub。新设备先创建目标 Vault，然后把备份导入目标路径并重建索引：

```console
cargo run --quiet -p readtrace-cli -- export-vault <备份目录> <新Vault目录>
cargo run --quiet -p readtrace-cli -- reindex <新Vault目录>
cargo run --quiet -p readtrace-cli -- ls <新Vault目录>
```

如果要搬运整个 Workspace，可以私下复制整个 `workspace/`（包括 `workspace.json`），再对各 Vault 执行 `reindex`；仍然不要把运行数据加入 Git。

## 六、日常更新与启动 Web

```console
git pull --ff-only
cargo test --workspace
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
```

浏览器打开 <http://127.0.0.1:8787/>。关闭服务不会删除 Vault；下一次重新执行 `serve` 即可。

## 七、常见问题

先运行：

```console
cargo run --quiet -p readtrace-cli -- ocr-check
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ls ./workspace
```

- 找不到 `tesseract`：检查 `READTRACE_TESSERACT_BIN` 是否为当前设备的绝对路径，然后重跑 `ocr-check`；
- 缺少 `chi_sim`：macOS 确认安装 `tesseract-lang`，Windows 确认语言包位于 `tessdata`；
- PDF 无法处理：同时检查 `pdfinfo` 和 `pdftoppm`，只安装 Tesseract 不足以处理 PDF；
- `Codex CLI could not be started`：Windows 用 `Get-Command codex`，macOS 用 `command -v codex`，并确认 CLI 已登录；
- `readonly database`、`Permission denied`：在普通 Windows Terminal/PowerShell 或 macOS Terminal 中重试，不要把登录文件复制进项目；
- 网关返回 401/403：检查 `READTRACE_API_KEY_ENV` 指向的变量是否存在于 `.env` 或当前环境；
- GLM‑5.2 费用仍为 unknown：确认响应返回 input/output usage；旧记录必须至少已有 input/output Token 才能回填；
- 学校账单和项目估算不同：学校可能采用独立价格，填写三项 `READTRACE_*_PRICE` 覆盖官方公开 API 价；
- 搜索结果为空：确认内容已经发布到 `clean/`，然后执行 `reindex`；
- 迁移后网页看不到文件：确认传给 `serve` 的是新 Workspace，或传给命令的是其中的 Vault，而不是旧设备绝对路径。
