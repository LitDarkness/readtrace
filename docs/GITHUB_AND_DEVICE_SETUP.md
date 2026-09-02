# GitHub 与多设备使用说明

本文把两类数据分开处理：GitHub 只保存 ReadTrace 的源代码和可公开的配置模板；导入的原素材、Vault、运行日志、会话和 API Key 放在本机或单独的备份中。这样既不会把密钥推到远端，也不会让一次测试把仓库变成数据仓库。

下面的 PowerShell 命令都假定从项目根目录执行。当前设备的项目根目录是 `E:\AI_diary\summer_project`；`workspace` 是运行数据目录，不是 Git 仓库根目录。

## 一、首次上传到 GitHub

### 1. 提交前检查

```powershell
Set-Location E:\AI_diary\summer_project
git status --short
git status --short --ignored
git check-ignore -v .env workspace runtime tmp deliverables
```

`.env`、`workspace/`、`runtime/`、`tmp/`、`deliverables/` 和作业 PDF 已加入 `.gitignore`。它们可以继续留在本机，但不会被加入下面的提交。不要使用 `git add -f .env`。

仓库中应提交的内容主要是：

- `crates/`、`Cargo.toml`、`Cargo.lock`：Rust 源代码和依赖锁定文件；
- `docs/`、`README.md`、`ARCHITECTURE*.md`、`CONTEXT.md`：设计与使用说明；
- `.env.example`、`prompts/`、`scripts/`、`tests/README.md`：公开模板和可复现脚本。

### 2. 本地提交

如果当前仓库还没有提交，可以执行：

```powershell
git add .
git diff --cached --stat
git diff --cached --check
git commit -m "feat: publish ReadTrace pipeline and web workspace"
git branch -M main
```

`git diff --cached --stat` 中不应出现 `.env`、图片、PDF、`workspace/` 或 `target/`。如果出现了，先用 `git restore --staged <路径>` 移出暂存区，再检查 `.gitignore`。

如果本机已经有提交，跳过 `git add` 和 `git commit`，只确认 `git branch --show-current` 和 `git status --short` 即可。

### 3. 绑定 GitHub 远端并推送

先在 GitHub 创建一个空仓库（不要自动添加 README、License 或 `.gitignore`），然后把地址替换为自己的地址：

```powershell
git remote add origin https://github.com/<账号>/<仓库>.git
# 如果 origin 已存在，改用：
# git remote set-url origin https://github.com/<账号>/<仓库>.git
git remote -v
git push -u origin main
```

HTTPS 推送时按 GitHub 的登录提示完成认证；也可以把 `origin` 换成自己的 SSH 地址。推送前最后看一遍：

```powershell
git ls-files
git status --short --ignored
```

若 GitHub 仓库不是空仓库，不要直接覆盖远端历史。先备份本地提交，再根据远端内容决定 `git pull --rebase` 或重新创建空仓库。

## 二、另一台设备的全新安装

### 1. 安装运行依赖

需要：

1. Git；
2. Rust stable（包含 `cargo`）；
3. 处理图片时的 Tesseract OCR；
4. 处理 PDF 时的 Poppler（`pdfinfo` 和 `pdftoppm`）；
5. 若使用 Codex Provider，还要单独安装并登录可在终端执行的 Codex CLI。

安装后先确认：

```powershell
Get-Command cargo
Get-Command tesseract -ErrorAction SilentlyContinue
Get-Command pdftoppm -ErrorAction SilentlyContinue
Get-Command pdfinfo -ErrorAction SilentlyContinue
Get-Command codex -ErrorAction SilentlyContinue
```

如果程序不在 `PATH`，直接在项目根目录的 `.env` 写绝对路径。用下面的命令查看 ReadTrace 实际选中的程序：

```powershell
cargo run --quiet -p readtrace-cli -- ocr-check
```

### 2. 克隆并构建

```powershell
git clone https://github.com/<账号>/<仓库>.git
Set-Location .\<仓库>
cargo build --workspace
cargo test --workspace
```

### 3. 创建本机 `.env`

`.env.example` 是模板，复制后只在本机编辑：

```powershell
Copy-Item .env.example .env
notepad .env
```

清华或其它 OpenAI-compatible 网关至少需要这些字段（示例中的 Key 只是变量名，不是 Key 本身）：

```dotenv
READTRACE_BASE_URL=https://<网关>/api/v1
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.2
READTRACE_API_KEY_ENV=THU_AI_PLATFORM_API_KEY
THU_AI_PLATFORM_API_KEY=<只保存在本机>
READTRACE_THINKING_MODE=none
READTRACE_USD_TO_CNY=6.8
```

图片/PDF 的路径按新设备实际位置修改：

```dotenv
READTRACE_TESSERACT_BIN=C:/tools/tesseract/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/tools/tesseract/tessdata
```

如果使用 Codex CLI，在新设备上重新登录 CLI，并把入口写入 `.env`（不复制旧设备的 `auth.json`）：

```dotenv
READTRACE_CODEX_BIN=C:/Users/<用户名>/AppData/Local/OpenAI/Codex/bin/codex.exe
```

检查 Provider 时，`provider-check` 只显示 Key 是否存在，不会显示 Key 内容：

```powershell
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ai-check --provider http --model glm-5.2 --thinking none
```

### 4. 创建 Workspace 和 Vault

克隆只得到代码，不会得到被忽略的运行数据。新建一套空数据：

```powershell
$workspace = ".\workspace"
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
cargo run --quiet -p readtrace-cli -- vault-create $workspace "我的资料"
cargo run --quiet -p readtrace-cli -- ls $workspace
$vault = cargo run --quiet -p readtrace-cli -- vault-path $workspace "我的资料"
cargo run --quiet -p readtrace-cli -- ls $vault
```

导入、OCR、修复、构建和问答的完整例子见 [`COMPLETE_FLOW_TUTORIAL.md`](COMPLETE_FLOW_TUTORIAL.md)；从外部文件夹导入时，外部路径可以是任意位置，目标始终是 `$vault`。

## 三、把已有 Vault 搬到另一台设备

推荐使用 ReadTrace 自己的导出命令。旧设备上，`<旧 Vault>` 是例如 `E:\AI_diary\summer_project\workspace\vaults\first_run` 的完整路径：

```powershell
Set-Location E:\AI_diary\summer_project
cargo run --quiet -p readtrace-cli -- export-vault `
  "E:\AI_diary\summer_project\workspace\vaults\first_run" `
  "E:\ReadTraceBackup\first_run"
```

把 `E:\ReadTraceBackup\first_run` 通过硬盘、NAS 或其它私有方式复制到新设备。不要把包含 `sources/` 的备份上传到公开 GitHub 仓库。

新设备上先创建同名或自定义名称的 Vault，再把导出内容写入它：

```powershell
$workspace = ".\workspace"
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
cargo run --quiet -p readtrace-cli -- vault-create $workspace "我的资料"
$vault = cargo run --quiet -p readtrace-cli -- vault-path $workspace "我的资料"
cargo run --quiet -p readtrace-cli -- export-vault `
  "D:\ReadTraceBackup\first_run" `
  $vault
cargo run --quiet -p readtrace-cli -- reindex $vault
cargo run --quiet -p readtrace-cli -- ls $vault
```

`export-vault` 会带走 sources、raw、generated、clean、prompts、events、sessions 和运行台账，但不会带走 `.readtrace/providers.json`。这是有意的：Provider Key 应在新设备重新填写。索引数据库是可重建的，恢复后执行 `reindex` 即可。

如果要搬运整个 Workspace，可以私下复制整个 `workspace/` 目录（包括 `workspace.json`），然后在新设备执行 `reindex`；仍然不要把它加入 Git。

## 四、日常更新与启动 Web

代码更新：

```powershell
Set-Location .\<仓库>
git pull --ff-only
cargo test --workspace
```

启动网页工作台：

```powershell
cargo run --quiet -p readtrace-cli -- serve .\workspace --bind 127.0.0.1:8787
```

浏览器打开 <http://127.0.0.1:8787/>。网页中的“来源与 API”配置保存在当前用户目录的 `ReadTrace/providers.json`（受限环境下可能回退到 Vault 的 `.readtrace/providers.json`），同样不会进入 Git；换设备后在网页中重新添加自定义来源即可。

关闭服务后，Vault 中的 clean Markdown、原素材、事件、会话和运行台账仍会保留。下一次只需再次运行 `serve`，不需要重新导入。

## 五、遇到问题时先看哪里

```powershell
git status --short --ignored
cargo run --quiet -p readtrace-cli -- ocr-check
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ls .\workspace
```

- 找不到 `tesseract`：填写 `READTRACE_TESSERACT_BIN`，然后重跑 `ocr-check`；
- PDF 无法处理：同时检查 `pdfinfo` 和 `pdftoppm`；
- `Codex CLI could not be started`：在同一个终端检查 `READTRACE_CODEX_BIN` 和 CLI 登录状态；
- 网关返回 401/403：检查 `READTRACE_API_KEY_ENV` 指向的变量是否在 `.env` 或当前环境中存在；
- 搜索结果为空：确认内容已经发布到 `clean/`，然后执行 `reindex`；
- 迁移后网页看不到文件：确认 `$vault` 指向新 Workspace 中的 Vault，而不是 Workspace 根目录。
