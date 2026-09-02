# ReadTrace（读迹）

ReadTrace 是一个小而实用的项目式 OCR 文本整理工具。它把文件、文件夹、图片、PDF、TXT 和 Markdown 导入独立 Vault，完成真实 OCR（或 Mock OCR）、确定性空白清洗、整页 LLM 修复，再生成带原始来源链接的 Markdown revision。原始素材、OCR、规范化结果、每页修复结果和调用台账始终分开保存，人工可以随时直接编辑或恢复。

视觉页默认必须有完整的 LLM 修复才能生成可引用 revision；若只是想先得到一份待校对的合并稿，可在 CLI/API/UI 中显式选择 `--allow-unrepaired` / `allow_unrepaired:true`。该路径会在 revision 中留下 warning，且不会让未修复 OCR 进入问答引用。

## 当前架构

单个 batch 的主流程只有四个阶段；多来源和跨 batch 合并是独立的确认阶段：

```text
import → ocr → repair → build
                    │       └─ revisioned Markdown + source anchors
                    └─ per-page checkpoint + runtime/calls.jsonl

source/clean units → merge_plan → 人工确认 → 跨 batch revision
```

LLM 每次处理一整页并返回 `repaired_text`，不再返回逐条 patch、置信度或审查状态。`repair` 失败时只记录该页错误，其余页继续；再次执行会跳过已有且未过期的页，因此可以断点续跑。

## 构建与平台依赖

Rust stable、Git、Tesseract 和 Poppler 均需安装。项目本身不绑定特定 shell，下面两条构建命令在 Windows PowerShell 与 macOS Terminal（zsh/bash）中相同：

```console
cargo build --workspace
cargo test --workspace
```

真实图片/PDF OCR 需要 Tesseract；PDF 还需要 Poppler 的 `pdfinfo` 和 `pdftoppm`。TXT/Markdown 不需要这些工具。

### macOS

使用 Homebrew 安装 Tesseract、简体中文语言包和 Poppler：

```bash
brew install tesseract tesseract-lang poppler
```

Apple Silicon Mac 可在项目根目录的 `.env` 中使用：

```dotenv
READTRACE_TESSERACT_BIN=/opt/homebrew/bin/tesseract
READTRACE_PDFTOPPM_BIN=/opt/homebrew/bin/pdftoppm
READTRACE_PDFINFO_BIN=/opt/homebrew/bin/pdfinfo
```

Intel Mac 把前缀改为 `/usr/local`。通常不必设置 `TESSDATA_PREFIX`；只有 `ocr-check` 找不到 `chi_sim` 时，才按实际 Homebrew 目录设置为 `/opt/homebrew/share/tessdata` 或 `/usr/local/share/tessdata`。

### Windows

保留已经验证可用的 Tesseract/Poppler 安装，在 `.env` 中填写实际绝对路径。dotenv 建议使用正斜杠，避免反斜杠转义和盘符歧义：

```dotenv
READTRACE_TESSERACT_BIN=C:/Program Files/Tesseract-OCR/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/Program Files/Tesseract-OCR/tessdata
```

两套系统共用的 OCR 配置为：

```dotenv
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
```

CLI 会自动加载项目 `.env`；未填写路径时会依次尝试项目内工具、Homebrew 常见路径、Windows 常见安装位置和当前 `PATH`。用 `cargo run -p readtrace-cli -- ocr-check` 查看实际选中的程序。PDF 会先显示页级 `0/N` 栅格化进度，再显示 `n/N` 的 Tesseract 进度；页面默认并行 4 路，可用 `READTRACE_OCR_CONCURRENCY=1..16` 调整。需要更快时可把 `READTRACE_OCR_DPI` 从默认 200 调低（建议先试 150），代价是小字识别质量可能下降。完整的双平台安装、迁移和排错说明见 [`docs/GITHUB_AND_DEVICE_SETUP.md`](docs/GITHUB_AND_DEVICE_SETUP.md)。

## Provider 配置

HTTP Provider 兼容 OpenAI Chat Completions，支持学校网关、自定义 Base URL、认证头、Key 环境变量和模型：

```dotenv
READTRACE_BASE_URL=https://example.edu/api/v1
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.3-flash
READTRACE_API_KEY_ENV=READTRACE_API_KEY
READTRACE_AUTH_HEADER=Authorization
READTRACE_AUTH_SCHEME=Bearer
READTRACE_RESPONSE_FORMAT=json_object
READTRACE_MAX_TOKENS_FIELD=max_tokens
# GLM-5.3-Flash 会自动落到 reasoning_effort=low；GLM-5.2 可用 none 真正关闭思考
READTRACE_THINKING_MODE=none
READTRACE_TIMEOUT_SECONDS=300
```

也可以使用 `--preset glm`、`--preset deepseek`、`--preset ollama` 等，再用参数覆盖模型或 thinking。Codex 使用本机登录态，不把 Key 交给 CLI：

```console
cargo run -p readtrace-cli -- ai-check --provider codex-cli --preset codex-luna --thinking high
```

`codex-luna` 对应 `gpt-5.6-luna` 和 High。当前适配器会读取 Codex CLI `--json` 事件中的逐次 Token；旧记录只要已经保存 input/output，就会在 `usage` 读取时按模型价格回填。确实没有 usage 的失败调用仍显示为 `unknown`，不会伪造 Token。

这里的 `codex-cli` 指可以在当前进程中执行的 Codex 命令，不是 Codex Desktop 的图形界面本身。默认值是 `codex`；macOS 从当前 `PATH` 查找，Windows 还会识别 `.exe`、`.cmd`、`.bat`、`.ps1` 和 OpenAI Codex Desktop 的本地安装目录。也可以在项目根目录 `.env` 中写绝对路径：

```dotenv
# Windows：推荐直接指向 codex.exe；npm 安装的 codex.cmd/codex.ps1 也支持
READTRACE_CODEX_BIN=C:/Users/<用户名>/AppData/Local/OpenAI/Codex/bin/<版本目录>/codex.exe
# macOS：如果 codex 已在启动 ReadTrace 的 PATH 中，保留命令名即可
# READTRACE_CODEX_BIN=codex
```

如果出现 `Codex CLI could not be started`，Windows 在同一个 PowerShell 执行 `Get-Command codex` 或 `where.exe codex`，macOS 在同一个 Terminal 执行 `command -v codex`，再把真实路径填入 `READTRACE_CODEX_BIN`。如果错误包含 `readonly database` 或 `Permission denied`，说明当前进程不能写入 Codex 的配置目录；请在普通 Windows Terminal/PowerShell 或 macOS Terminal 中重试，或改用 `--provider http`。证书错误应检查当前系统的证书信任和代理。不要把 `auth.json` 复制到项目目录。只有打开 GUI、选择了 Luna 模型，并不会自动给 Rust 程序提供一个可调用的 shell 命令；没有 CLI 时请改用 `--provider http`（例如 GLM 网关）或先安装并登录 Codex CLI。

Provider 和模型必须配套：`codex-cli` 默认使用 `codex-luna`（`gpt-5.6-luna`）；把 `glm-*` 显式交给 Codex CLI 会直接报错，请改用 `--provider http --model glm-5.3-flash`。这样不会再生成“Codex provider + GLM model”的错误账本记录。

启动 Web 工作台后，左侧“来源与 API”可以管理同一套 Provider profile：内置清华 GLM-5.3 Flash、GLM-5.2、Codex Luna High 和 Mock，也可以新增任意 OpenAI-compatible Base URL。
Key 只写入本机配置：Windows 默认是 `%LOCALAPPDATA%/ReadTrace/providers.json`，macOS 默认是 `~/Library/Application Support/ReadTrace/providers.json`，也可由 `READTRACE_PROVIDER_STORE` 指定。受限进程无法写用户目录时会回退到当前 Vault 的 `.readtrace/providers.json`。这些位置不会返回到网页或进入 Git；repair、导入队列和阅读室问答都从同一来源列表选择模型与 `None/Low/Mid/High` 推理强度。

内置来源和自定义来源彼此独立：在网页里新建并保存 Key，不会把该 Key 注入同模型的内置来源。下拉框会同时显示来源名称、`内置/自定义` 和 `Key 已配置/未配置 Key`；首次加载时优先选择已经配置 Key 的自定义 GLM-5.2。若 repair 报 `missing API key; set READTRACE_API_KEY`，说明实际选中的是没有环境变量 Key 的内置来源，请切换到标有“自定义 · Key 已配置”的条目，或在项目 `.env` 中配置内置来源使用的 `READTRACE_API_KEY` 后重启服务。
导入队列和文件浏览的合并栏都支持自定义 `clean/<名称>/document.md` 发布路径；处理页还可以展开编辑当前 Vault 的 `prompts/repair.md`，下一次 repair 自动使用。阅读室将检索与对话分成两个页面，聊天中的“添加引用”使用弹出式 clean 本地搜索，下面还可以在可折叠文件树中多选 clean Markdown/TXT 作为带文件名的引用；文件浏览中的 Markdown/TXT 预览可以直接编辑保存，保存后自动刷新索引。连接测试会显示耗时/usage，并计入 Vault 的运行台账。详细字段与 API 见 [`docs/WEB_GUI_PROTOCOL.md`](docs/WEB_GUI_PROTOCOL.md)。

修复时优先用 `--thinking none|low|medium|high` 指定推理强度；`--speed low|mid|high` 是保留的兼容简写。同一张图可以用 `--refresh` 重跑比较，调用记录会保留 `thinking_mode` 和 `duration_ms`。GLM-5.3/5.3-Flash 是强制思考模型，网关只接受 `reasoning_effort=low|high|max`，因此 `none`/`medium` 会安全映射为最低 `low`；GLM-5.2 及兼容模型仍发送 `thinking:{"type":"disabled"}`，可以真正关闭思考。

多页 repair 默认并行，`.env` 中的 `READTRACE_LLM_CONCURRENCY` 控制在途请求数（默认 4，范围 1–64）；结果按原始页序写入，单页 checkpoint 可断点续跑。OCR 页面同样有界并行，由 `READTRACE_OCR_CONCURRENCY` 控制。

运行账本保存 input、cached-input、output、reasoning、total Token 以及调用时的模型单价快照。已知 OpenAI 模型、GLM‑5.2 和 GLM‑5.3 Flash 自动使用官方价格；旧记录在读取或执行 `usage` 时会补齐费用。根据 [Z.ai 官方价格页](https://docs.z.ai/guides/overview/pricing) 在 2026-09-02 的标价，GLM‑5.2 为 `$1.40 / $0.26 / $4.40`（input/cached input/output，每百万 Token）；代码中的价格版本为 `zai-model-pricing-2026-09-02`。学校网关若采用不同结算价，应显式填写三档 `READTRACE_*_PRICE` 覆盖；真正缺少 usage 或价格时才保持 `null` 并计入 `unknown_cost_calls`，Mock 调用明确记为 `$0`。

`repair` 的完整结构可用 `--format json` 保存，其中包含 `repair_file`、每页 `result_files` 以及前 500 字 `result_previews`；完整修复文本位于每页 JSON 和 build 生成的 `current.md`/revision Markdown 中。

示例 PNG 的一次实测（真实 Tesseract + `gpt-5.6-luna`，单页）repair 耗时为 Low 21.138 s、Mid 18.097 s、High 52.614 s；完整 process 墙钟为 23.908 s、20.738 s、55.340 s。这是单次网络样本，详细复现命令和解释见 [`tests/README.md`](tests/README.md)。

本机 25 页 PDF 的真实 OCR 实测约 29.6 s（默认 DPI 200、4 路并行）；旧的整批 PDF 栅格化流程约 58.7 s。若更重视速度，可把 `READTRACE_OCR_DPI` 调到 150，并保持 `READTRACE_OCR_CONCURRENCY=4`，再用 `ocr-check` 确认当前挡位。

## 最短演示

```console
cargo run -p readtrace-cli -- init ./workspace/vaults/demo
cargo run -p readtrace-cli -- process ./workspace/vaults/demo ./tests/1.png --ocr real --llm codex-cli --preset codex-luna --speed high
```

没有 Tesseract 时先把 `--ocr mock` 用于流程测试。完整 CLI、提示词、断点恢复和费用规则见 [`docs/CLI_TUTORIAL.md`](docs/CLI_TUTORIAL.md)；从零执行一次导入、OCR、修复、合并、引用和问答见 [`docs/CLI_END_TO_END_EXAMPLE.md`](docs/CLI_END_TO_END_EXAMPLE.md)。包含 PDF、Markdown、两张图片和 TXT/MD 直达 clean 分流的可复现实例见 [`docs/COMPLETE_FLOW_TUTORIAL.md`](docs/COMPLETE_FLOW_TUTORIAL.md)。CLI 默认输出人类摘要，需要脚本 JSON 时加 `--format json`。

查看 Vault 和合并单位：

```console
cargo run -p readtrace-cli -- ls ./workspace
cargo run -p readtrace-cli -- sources ./workspace/vaults/demo
```

## 启动 Web 工作台

### 第一次启动

如果刚克隆项目、`./workspace` 还不存在，请在项目根目录依次执行以下三条命令。它们会创建 Workspace、创建第一个 Vault，然后同时启动 Rust 服务端和内置前端；Windows PowerShell 与 macOS Terminal 均可直接使用：

```console
cargo run --quiet -p readtrace-cli -- workspace-init ./workspace
cargo run --quiet -p readtrace-cli -- vault-create ./workspace default
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
```

这里应使用 `workspace-init <WORKSPACE>`。`init <PROJECT>` 用于初始化单个独立 Vault/旧项目目录，而且必须提供路径；因此 `cargo run init` 会因为缺少 `<PROJECT>` 而报错，不是 Web Workspace 的正确初始化方式。

### 后续启动

Workspace 已存在时，只需在项目根目录运行：

```console
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
```

启动后访问 <http://127.0.0.1:8787/>。如果使用其它 Workspace 位置，请把 `./workspace` 替换为相应的绝对路径或相对路径。停止服务时在启动它的终端按 `Ctrl+C`。需要增加其它 Vault 时执行：

```console
cargo run --quiet -p readtrace-cli -- vault-create ./workspace <名称>
```

## 输入边界

- `.txt`、`.md`：直接读取，不经过 OCR。
- `.pdf`、`.png`、`.jpg`、`.jpeg`、`.webp`、`.bmp`：真实 Tesseract/Poppler OCR。
- 文件夹：递归解析支持的文件，其余写入 `skipped_files`。
- 其它格式：暂不处理，不会静默改名或移动。

图片/PDF 的 raw OCR 和简单 normalization 只用于审计，不作为检索、问答或跨 batch 合并的最终证据；必须先有完整 page repair 并 build 到 `clean/`。TXT/Markdown 可以直接 build，生成的 clean 投影才是网页检索和引用的内容边界。

默认导入会把原素材复制到 `sources/<batch_id>/`。若素材很大或不希望复制，使用 `--no-copy`；manifest 会保存绝对 `external_path`，OCR 仍可读取原文件。

## Vault 目录

```text
vault/
├─ sources/<batch_id>/             # copied=true 时的素材快照
├─ raw/<batch_id>/                 # batch.json 与 OcrPage JSON
├─ generated/<batch_id>/
│  ├─ normalization.json           # 可人工编辑的确定性清洗层
│  ├─ repair/<page>.json           # 每页 repaired_text checkpoint
│  ├─ repair.json                  # 本次 repair run 汇总
│  └─ <document>/revisions/0001/   # 不可变 Markdown revision
├─ generated/merges/<merge_id>/    # 跨 batch unit merge plan 与 revision
├─ clean/<name>/document.md        # 生成后自动发布的可读投影，可人工编辑/覆盖
├─ prompts/repair.md               # Vault 级可编辑提示词（可选）
├─ prompts/profile.md              # 角色别名/专名等上下文（可选）
├─ runtime/calls.jsonl             # 每次 LLM 调用的 Token/费用/耗时
├─ events/events.jsonl             # 进度和错误事件
└─ .readtrace/state.db             # 可重建的本地搜索索引
```

搜索是只针对 `clean/` 的普通 SQLite 文本查询，不接入 LLM；问答才会调用 Provider，并保留来源引用。每次 `build`、确认 `merge` 或确认 `merge-units` 都会把最终 Markdown 自动复制到 `clean/<名称>/document.md`；可用 `--clean-name 剧本/第一章` 自定义名称，重复发布同名文件只替换 clean 投影，不删除 generated 历史 revision。引用弹窗和检索只显示 clean 内容。删除 batch/unit 使用 `delete-batch`、`delete-unit`，默认只预览，必须显式 `--confirm`；运行台账和事件流保留。

## 课程交付

设计文档、CLI 教程、交付清单和 API/Token 统计规则在 `docs/`。源代码目录包含本 README、`.env.example` 和演示脚本；不在仓库中提交真实 Key。GitHub 上传、Vault 备份和另一台设备的安装流程见 [`docs/GITHUB_AND_DEVICE_SETUP.md`](docs/GITHUB_AND_DEVICE_SETUP.md)。

本轮逐项功能审计、可复现检查结果和 GUI 状态见 [`docs/IMPLEMENTATION_AUDIT.md`](docs/IMPLEMENTATION_AUDIT.md)；Web/GUI 的端点契约见 [`docs/WEB_GUI_PROTOCOL.md`](docs/WEB_GUI_PROTOCOL.md)。启动 `serve <workspace>` 后访问 `http://127.0.0.1:8787/` 即可进入工作台：创建/切换 Workspace 与 Vault、浏览预览文件、排队导入、处理 batch、跨 batch 合并以及引用问答都在同一界面完成。后台页还会汇总最近事件、任务完成态、Token 和费用。
