# ReadTrace CLI 操作手册

本文中的命令都从项目根目录执行。CLI 启动时读取当前目录的 `.env`，所以 OCR 路径和 Provider 配置不需要每次手工设置。示例主体沿用已经在 Windows 验证过的 PowerShell 变量语法；macOS zsh/bash 对照如下，CLI 子命令和参数不变：

| 操作 | Windows PowerShell | macOS zsh/bash |
| --- | --- | --- |
| 当前目录 | `$project = (Get-Location).Path` | `project="$(pwd)"` |
| 拼接目录 | `Join-Path $project "workspace"` | `"$project/workspace"` |
| 续行 | 行尾反引号 `` ` `` | 行尾反斜杠 `\` |
| 当前目录相对路径 | `.\workspace` | `./workspace` |

双平台依赖安装、`.env` 和迁移步骤见 [`GITHUB_AND_DEVICE_SETUP.md`](GITHUB_AND_DEVICE_SETUP.md)。

第一次使用不必先掌握 CLI：先按 [`QUICK_START.md`](QUICK_START.md) 安装 OCR 依赖并启动 Web；本文用于需要脚本化、断点恢复或批量操作时的命令速查。Provider 配置是可选的，`mock` 可以在没有 Key 的情况下验证流程。

CLI 默认输出适合人阅读的摘要；脚本需要完整 JSON 时，在命令前或后加 `--format json`。例如 `ls $workspace` 会输出 Vault 列表，`ls $workspace --format json` 保留完整原始结构；所有 JSON 命令都遵循这条规则。

如果希望从“我完全不知道目录和 batch 是什么”开始照着走，请先看 [`CLI_END_TO_END_EXAMPLE.md`](CLI_END_TO_END_EXAMPLE.md)；本文保留各命令的速查说明。

## 0. 路径约定

| 名称 | 示例 | 作用 |
| --- | --- | --- |
| 项目根目录 | Windows `E:\readtrace`；macOS `/Users/me/readtrace` | Rust workspace，运行 `cargo` |
| Workspace | `<项目>/workspace` | 登记多个 Vault，包含 `workspace.json` |
| Vault | `<项目>/workspace/vaults/first_run` | 保存 source、raw、generated、sessions、runtime |
| 外部输入 | 任意本机图片、PDF、TXT/MD 文件或目录 | 待导入的文件或文件夹，不是 Vault |
| 临时测试目录 | `<项目>/tmp/...` | 测试产物；可纳入运行时费用扫描 |

`.\vault` 只表示“当前目录下的 vault”。为了避免混淆，建议脚本统一使用绝对路径：

```powershell
Set-Location E:\AI_diary\summer_project
$workspace = "E:\AI_diary\summer_project\workspace"
$vault = "E:\AI_diary\summer_project\workspace\vaults\first_run"
```

macOS 对应写法：

```bash
cd /Users/me/readtrace
workspace="$(pwd)/workspace"
vault="$workspace/vaults/first_run"
```

Workspace 不能直接传给 `ocr`、`repair`、`answer`；先用 `vault-path`，或直接使用 Vault 的绝对路径。

## 1. Workspace、Vault 和查看命令

```powershell
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
cargo run --quiet -p readtrace-cli -- vault-create $workspace first_run
cargo run --quiet -p readtrace-cli -- vault-list $workspace
cargo run --quiet -p readtrace-cli -- vault-path $workspace first_run
```

现在也可以使用类似 `ls` 的命令：

```powershell
# 查看 Workspace 中的所有 Vault
cargo run --quiet -p readtrace-cli -- ls $workspace

# 查看某个 Vault 的 batch 和所有可合并单元
cargo run --quiet -p readtrace-cli -- ls $vault

# 只查看可选资料单元
cargo run --quiet -p readtrace-cli -- sources $vault
cargo run --quiet -p readtrace-cli -- sources $vault --kind source
cargo run --quiet -p readtrace-cli -- sources $vault --batch-id <batch_id>
```

`sources` 输出的 `unit_id` 是后续跨 batch 合并的最小选择单位。`source:<batch>/<source>` 表示一份导入文件；`clean:clean/...md` 表示 `clean/` 下的人工 Markdown；`clean:generated/.../current.md` 表示一份已经生成并允许人工编辑的 Markdown 文件。

## 2. OCR 环境

项目使用 `.env` 保存 OCR 程序路径，CLI 会自动读取：

```dotenv
# Windows 示例；必须换成实际安装位置
READTRACE_TESSERACT_BIN=C:/Program Files/Tesseract-OCR/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/Program Files/Tesseract-OCR/tessdata
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
```

macOS Apple Silicon 使用 `/opt/homebrew/bin/tesseract`、`/opt/homebrew/bin/pdftoppm`、`/opt/homebrew/bin/pdfinfo`；Intel Homebrew 使用 `/usr/local/bin/...`。macOS 通常不需要 `TESSDATA_PREFIX`。如果没有设置绝对路径，程序会依次尝试项目内工具、Homebrew 常见路径、Windows 常见安装位置，最后回退到 `PATH` 中的命令名。

先检查当前环境：

```powershell
cargo run --quiet -p readtrace-cli -- ocr-check
```

输出中的 `tesseract_available`、`pdftoppm_available` 和 `pdfinfo_available` 必须为 `true` 才能完整处理图片/PDF。TXT/Markdown 不需要 OCR；PDF 需要 Poppler 的 `pdfinfo`（页数）和 `pdftoppm`（栅格化），图片需要 Tesseract。其它扩展名会被跳过并记录在 batch 的 `skipped_files`。`ocr_dpi` 默认为 200，可降到 150 换取速度；`ocr_concurrency` 是页面并行上限，默认 4，范围 1–16。

PDF 不再把一个文件当成一个不可解释的 `0/1`：任务会先显示例如 `0/25 · rendering PDF (25 pages)`，随后显示页级栅格化和识别进度 `n/25 · rendered PDF page p/25`、`n/25 · OCR page p/25`，最后显示 `25/25 · OCR complete (25 pages)`。并行处理时页完成顺序可能交错，但 `raw` page number 和后续文档顺序仍按 PDF 原页序保存。

## 3. Provider 和推理强度

```powershell
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ai-check --provider http
cargo run --quiet -p readtrace-cli -- ai-check --provider codex-cli --preset codex-luna --speed high
```

HTTP Provider 从 `.env` 读取清华网关或其它 OpenAI-compatible Endpoint：

```dotenv
READTRACE_BASE_URL=https://school.example/v1
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.3-flash
READTRACE_API_KEY_ENV=READTRACE_API_KEY
READTRACE_AUTH_HEADER=Authorization
READTRACE_AUTH_SCHEME=Bearer
READTRACE_MAX_TOKENS_FIELD=max_tokens
```

`--provider` 可选 `http`、`codex-cli`、`mock`。`--speed low|mid|high` 统一映射到 reasoning effort；`--preset codex-luna` 会选择 `gpt-5.6-luna` 和 `high`，但仍要求本机 Codex 登录或网关确实提供该模型。对清华网关的 GLM：GLM-5.3/5.3-Flash 强制思考，`--thinking none` 或 `--speed low` 会发送 `thinking.type=enabled` 与 `reasoning_effort=low`（这是该模型允许的最低挡位）；`--thinking high`/`max` 分别使用更高挡位。GLM-5.2 则支持真正的 `--thinking none`，请求中发送 `thinking.type=disabled`。

先用探针确认实际网关行为，并查看服务端返回的 usage：

```powershell
# 当前 .env 的 GLM-5.3-Flash（none 会安全落到 low）
cargo run --quiet -p readtrace-cli -- ai-check --provider http --thinking none --format json

# 真正关闭思考的 GLM-5.2 对照
cargo run --quiet -p readtrace-cli -- ai-check --provider http --model glm-5.2 --thinking none --format json
```

探针失败时，JSON 中的 `response_preview` 会包含网关返回的短错误正文；不会包含 Authorization 或 API Key。

repair 对不同页面默认并行调用，`.env` 中的 `READTRACE_LLM_CONCURRENCY` 控制同时在途的页数，默认是 `4`，允许 `1..64`。它只限制 LLM 请求，不改变导入顺序、checkpoint 文件名或最终文档顺序；网络不稳定时可以先调成 `1` 排查问题。

已知 OpenAI 模型、`glm-5.2` 和 `glm-5.3-flash` 会按价格表自动填入 input/cached-input/output 三档单价；Codex preset 会直接启用这项规则。GLM‑5.2 在 2026-09-02 的 [Z.ai 官方价格](https://docs.z.ai/guides/overview/pricing)为 `$1.40/$0.26/$4.40`（每百万 Token），程序记录版本 `zai-model-pricing-2026-09-02`。清华/学校网关若采用不同结算价，需在 `.env` 明确填写三项真实价格：

```dotenv
READTRACE_INPUT_PRICE=0        # 已知模型留 0 即使用内置价格
READTRACE_CACHED_INPUT_PRICE=0 # 学校另有价格时填写实际值
READTRACE_OUTPUT_PRICE=0       # 三项应一起配置
READTRACE_PRICING_VERSION=school-unknown
READTRACE_USD_TO_CNY=6.8
```

### Codex CLI 的 Windows 入口

`codex-cli` 调用的是 shell 中可以执行的 Codex CLI，不是桌面 GUI 的内部会话。ReadTrace 默认使用 `codex`，会在当前进程的 `PATH` 中寻找 `codex.exe`、`codex.cmd`、`codex.bat` 或 `codex.ps1`；若 PATH 尚未刷新，还会尝试 OpenAI Codex 的本地安装目录。最稳定的做法是在项目根目录 `.env` 指定绝对路径：

```dotenv
# 推荐 codex.exe；codex.cmd/codex.ps1 也可以
READTRACE_CODEX_BIN=C:/Users/<用户名>/AppData/Local/OpenAI/Codex/bin/<版本目录>/codex.exe
```

先用下面两条命令检查“当前 PowerShell”和“ReadTrace”看到的入口是否一致：

```powershell
Get-Command codex -ErrorAction SilentlyContinue
where.exe codex
cargo run --quiet -p readtrace-cli -- ai-check --provider codex-cli --preset codex-luna --speed low
```

`Codex CLI could not be started` 表示进程根本没有启动到模型请求阶段，通常是 PATH、绝对路径或 CLI 登录问题，并不是 OCR 文本或 prompt 错误。若错误进一步包含 `拒绝访问 (os error 5)`/`readonly database`，是 Codex 内置受限宿主不能写入 `CODEX_HOME`；请切换普通 PowerShell/Windows Terminal。若包含 `UnknownIssuer`/`invalid peer certificate`，请在普通终端重试或修复 Windows 信任库/代理。不要复制 `auth.json` 到项目。Codex Desktop 本身没有可供本项目直接访问的本地 HTTP API；只有安装并登录 CLI 后才能使用 `codex-cli`。如果只配置了学校网关，请使用 `--provider http`。

## 4. 导入与单个 batch 流程

导入永远是“外部输入 → 目标 Vault”。默认复制原素材，`--no-copy` 只保存外部绝对路径。

```powershell
# 单个文件
cargo run --quiet -p readtrace-cli -- import-file $vault E:\AI_diary\tests\1.png

# 递归导入文件夹；未知格式写入 skipped_files
cargo run --quiet -p readtrace-cli -- import-folder $vault D:\captures\chapter-03 --order filename

# 一键：导入 → OCR → 规范化 → 每页修复 → build
cargo run --quiet -p readtrace-cli -- process $vault E:\AI_diary\tests\1.png --ocr real --llm http
```

`process` 也支持 `--clean-name "剧本/第一章"`；一键流程在 build 阶段自动发布到这个 clean 路径。

分阶段执行：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault <batch_id> --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault <batch_id>
cargo run --quiet -p readtrace-cli -- repair $vault <batch_id> --provider codex-cli --preset codex-luna --speed high
cargo run --quiet -p readtrace-cli -- build $vault <batch_id> --clean-name "剧本/第一章"
```

单个 TXT/Markdown 不需要调用 LLM 也能直接成为可检索的 clean 文档。先导入，再执行：

```powershell
$text = cargo run --quiet -p readtrace-cli -- --format json `
  import-file $vault E:\notes\chapter.md --mode plain_text | ConvertFrom-Json
cargo run --quiet -p readtrace-cli -- direct-clean $vault $text.batch_id `
  --clean-name "笔记/第一章"
```

`direct-clean` 只接受一个 TXT/MD 来源，输出固定为 `clean/<名称>/document.md`，保留 `rt:block` 来源锚点且不产生 LLM 费用。需要模型改写时，仍使用上面的 `ocr → normalize → repair → build`；PDF、图片始终走视觉 OCR，不能使用 `direct-clean`。

`--no-apply` 会运行到 repair 但不生成 revision；`--refresh-normalization` 和 `--refresh-repair` 强制重跑对应阶段；`--prompt-file` 替换当前修复提示词；`--target notes/book.md` 在生成 revision 后追加到 Vault 内的目标文件，并先保存旧文件副本。

视觉来源若尚未生成完整的 `repair/<page_id>.json`，`build`/`apply` 会拒绝写入最终文档；这样不会把半成品 OCR 变成可搜索或可引用内容。若只是要交付一份待人工校对的稿件，可以显式执行 `build ... --allow-unrepaired` 或 `merge ... --confirm --allow-unrepaired`；revision 会留下 warning，但仍不会进入引用上下文。每次成功 build 会自动复制一份到 `clean/<document_id>/document.md`；用 `--clean-name "剧本/第一章"` 可改为 `clean/剧本/第一章/document.md`。TXT/Markdown 可以在规范化后直接 build。

对于多个源文件，`process` 会完成每页 OCR/repair，但默认停在合并确认前：

```text
merge_confirmation_required: true
generated/<batch_id>/merge_plan.json
```

查看或人工编辑这个 JSON 后执行：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault <batch_id> --confirm
```

确认合并并指定 clean 名称：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault <batch_id> --confirm --clean-name "剧本/第一章"
```

也可以在一条命令中确认：

```powershell
cargo run --quiet -p readtrace-cli -- process $vault D:\captures\chapter-03 --ocr real --llm http --confirm-merge
```

`merge_plan.json` 的页序可以人工调整；确认后的 revision 会按调整后的页序写入。未确认的多源 batch 不能通过 `build`/`apply` 绕过确认。

## 5. 跨 batch 合并

一个 batch 只是导入记录，不是合并边界。跨 batch 合并以 `source` 文件或 `clean` Markdown 为最小单位。

先列出可选单位：

```powershell
cargo run --quiet -p readtrace-cli -- sources $vault
```

用两个不同 batch 的 source unit 创建预览计划：

```powershell
$plan = cargo run --quiet -p readtrace-cli -- --format json merge-units $vault `
  "source:batch-a/src-aaa" `
  "source:batch-b/src-bbb" | ConvertFrom-Json
$mergeId = $plan.plan.merge_id
```

也可以直接用 batch id；它会展开该 batch 中的所有 source 文件：

```powershell
cargo run --quiet -p readtrace-cli -- merge-units $vault batch-a batch-b
```

把人工整理文件加入同一计划时，先把它放在 `$vault\clean\manual.md`，再用：

```powershell
cargo run --quiet -p readtrace-cli -- merge-units $vault `
  "source:batch-a/src-aaa" "clean:clean/manual.md"
```

预览计划保存在：

```text
generated/merges/<merge_id>/merge_plan.json
```

确认前可以直接编辑 `units` 或 `pages` 的顺序，但不要增删页面、unit 或 `source_ref`；构建器会校验完整性并保护来源锚点。确认并生成：

```powershell
cargo run --quiet -p readtrace-cli -- merge-units $vault $mergeId --confirm
```

跨 batch 确认时同样可以指定 `--clean-name "合集/第一章"`。同名 clean 文件会被新的最终正文替换，`generated/` 下的旧 revision 和原素材不会删除。

跨 batch 合并规则：

- 图片/PDF unit 默认必须已有完整 `repair/<page_id>.json`，不会把 raw OCR 或仅规范化文本当成最终内容；跨 batch 应急合并可加 `--allow-unrepaired`，结果会带 warning。
- 原始 TXT/Markdown unit 可以直接使用，因为它们本身就是可读文本。
- clean unit 使用 `clean/` 或 `generated/.../current.md` 的现有 Markdown 内容，并保留其已有 source block。
- 最终文档的每一页仍有 `rt:block` source 锚点，原素材不会被覆盖。

## 6. 引用问答与 session

没有显式引用时，`answer` 执行只针对 `clean/` 的本地 SQLite 搜索；搜索本身不调用 LLM。指定 `source-ref`、`quote` 或 `session-id` 后，证据边界只包含用户选定内容和该 session 的历史证据。`search` 命令和网页“检索”页遵循同一规则，不会命中 raw OCR、sources、generated 历史或 notes。

```powershell
cargo run --quiet -p readtrace-cli -- answer $vault `
  "这段剧情发生了什么？" `
  --provider http `
  --speed high `
  --source-ref "sources/batch-xxx/01.png:page:1" `
  --quote "用户补充的上下文"
```

`--source-ref` 必须复制 `repair` 输出的 `source_ref`（或 `sources`/`ls` 输出中已有的值），不要手写一个看似相近的路径。Mock OCR 的旧测试数据可能显示为 `page:1 image:...`；两种格式都按原样传入。

选项可以重复：

```powershell
--source-ref "..." --source-ref "..."
--quote "..." --quote-file E:\notes\context.txt
```

命令会返回 `session_id`。下一轮使用：

```powershell
cargo run --quiet -p readtrace-cli -- answer $vault `
  "结合上一轮继续解释" `
  --provider codex-cli `
  --preset codex-luna `
  --session-id "session-..."
```

引用策略是有意收紧的：图片/PDF 在 repair 完成前不会进入问答证据；检索和引用只读取 `clean/` 下的 Markdown/TXT。clean 文件可用 `--source-ref "clean:clean/剧本/第一章/document.md"` 指定；这样不会把半成品 OCR 当成事实。

## 7. Prompt、人工修改和恢复

可编辑位置：

- `prompts/text_repair_system.md`：项目默认提示词
- `vault/prompts/repair.md`：Vault 级修复规则
- `vault/prompts/profile.md`：角色别名、专名和上下文事实
- `generated/<batch_id>/normalization.json`：确定性清洗结果
- `generated/<batch_id>/repair/<page_id>.json`：每页完整修复结果
- `generated/.../current.md`：人工可编辑的当前正文

每次 build 都生成新的 `revisions/000N/document.md`，并更新 `clean/<name>/document.md`；不会覆盖原图、raw、旧 revision 或 repair checkpoint。人工推荐直接编辑 `clean` 中的文件，文件浏览会显示并允许保存；需要回溯时仍可查看 generated 的 `current.md` 和旧 revision。

删除也采用“先预览、再确认”：

```powershell
# 只显示将删除的 raw/source/generated 和受影响的 merge 计划
cargo run --quiet -p readtrace-cli -- delete-batch $vault <batch_id>

# 确认后才删除；runtime/calls.jsonl 和 events/events.jsonl 保留
cargo run --quiet -p readtrace-cli -- delete-batch $vault <batch_id> --confirm

# 先从 sources 复制完整 unit_id，再预览/确认删除一个 source 或 clean 文件
cargo run --quiet -p readtrace-cli -- sources $vault
cargo run --quiet -p readtrace-cli -- delete-unit $vault <unit_id>
cargo run --quiet -p readtrace-cli -- delete-unit $vault <unit_id> --confirm
```

删除复制的 source 会同时失效该 batch 的 generated 输出；`--no-copy` 的外部原文件不会被删除。删除 batch 的完整示例、影响范围和 JSON 计划见 [`CLI_END_TO_END_EXAMPLE.md`](CLI_END_TO_END_EXAMPLE.md) 第 13 节。

## 8. 运行时 Token 与费用

每个 repair page、answer 和 `ai-check` 调用都会写入一个 JSONL ledger；Vault 默认是 `runtime/calls.jsonl`，探针可以用 `--ledger tmp\codex-check.jsonl` 指定其它文件名。每条记录保存 input、cached-input、output、reasoning、total Token 和调用时的单价快照。`usage --scan-root` 会读取扫描根下所有 `.jsonl` 并按 `call_id` 去重，因此临时测试调用不会被漏掉。Token 缺失保持 `null`，价格未配置或只配置了一部分时费用保持 `null`。

费用按“未缓存 input + 缓存 input + output”计算：

```text
cached = min(cached_input_tokens, input_tokens)
uncached = input_tokens - cached
USD = uncached/1,000,000×input_price
    + cached/1,000,000×cached_input_price
    + output_tokens/1,000,000×output_price
CNY = USD×READTRACE_USD_TO_CNY
```

`usage` 的人类摘要会额外显示 `cached_input_tokens`；需要完整的每次调用明细时使用 `--format json`，再查看 `calls` 数组（或直接打开扫描输出文件）。

`answer` 的默认输出也会在答案后显示 input/cached/output/total Token，以及 `cost usd/cny` 和 `pricing` 版本；费用为 `null` 时表示 Provider usage 或单价尚未完整提供。

如果用 `--model` 临时切换到与 `.env` 不同的模型，ReadTrace 不会沿用 `.env` 原模型的单价；已知模型套用自己的价格，未知模型保持 `null`，避免把 GLM 价格误算给其它模型。

```powershell
cargo run --quiet -p readtrace-cli -- usage $vault
cargo run --quiet -p readtrace-cli -- usage $vault --batch-id <batch_id>
cargo run --quiet -p readtrace-cli -- usage --scan-root E:\AI_diary\summer_project --out .\deliverables\runtime-usage-all.json
```

扫描模式会按 `call_id` 去重，可同时覆盖 Workspace、Vault 和 `tmp` 测试输出，不会重复计费。

## 9. Web API

```powershell
cargo run --quiet -p readtrace-cli -- serve $workspace --bind 127.0.0.1:8787
```

CLI 与 Web 共用核心协议：

- `POST /api/import`、`POST /api/ocr`、`POST /api/normalize`、`POST /api/repair`（`provider`、`preset`、`model`、`thinking`、`speed`）
- `POST /api/merge`：同一 batch 的确认式合并
- `GET /api/sources`：列出 source/clean unit
- `POST /api/merge-units`：跨 batch unit 合并；预览传 `units`，确认传 `merge_id` 和 `confirm:true`
- `POST /api/answer`：支持 `source_refs`、`quotes`、`session_id`、`provider`、`preset`、`model`、`thinking`、`speed`
- `GET /api/search`、`GET /api/usage`、`GET /api/events`
- `GET /api/vaults`、`POST /api/vaults/select`、`GET /api/vault`
- `GET /api/tasks`、`GET /api/tasks/{task_id}`、`POST /api/tasks/{task_id}/cancel`；旧的 `POST /api/cancel` 仍按 batch 兼容
- `GET/POST /api/merge-plan`：读取或提交人工排序，提交内容会由 core 校验
- `GET /api/artifact?batch_id=...`：查看当前 revision 内容

打开 `http://127.0.0.1:8787/` 就是工作台 GUI；它不自带独立业务逻辑，只调用上述 API。以 `$workspace` 启动可以创建/切换多个 Vault；以单个 `$vault` 启动仍可运行，但不能创建同级 Vault。界面包含工作台、文件浏览、导入队列、批次处理、后台、来源与 API、检索和阅读问答八个工作区：文件浏览可点开图片/PDF/Markdown/TXT/JSON 预览，Markdown/TXT 可以直接编辑保存，勾选 source/clean 后可以跨 batch 合并或删除；检索页只做本地全文查询，阅读与问答页通过弹出式“添加引用”选择证据；导入队列可以连续添加多个路径并为每项设置复制策略，处理偏好在队列底部一次配置。

任务接口的最小调用顺序（PowerShell）如下；`task_id` 不要用 batch id 代替：

```powershell
$start = Invoke-RestMethod http://127.0.0.1:8787/api/ocr `
  -Method Post -ContentType 'application/json' `
  -Body (@{ batch_id = '<batch_id>'; provider = 'real' } | ConvertTo-Json)
Invoke-RestMethod "http://127.0.0.1:8787/api/tasks/$($start.task_id)"
Invoke-RestMethod "http://127.0.0.1:8787/api/tasks/$($start.task_id)/cancel" -Method Post
```

合并计划先 `POST /api/merge`（`confirm:false`）预览，再用 `GET /api/merge-plan?batch_id=...` 读取；人工调整页序后 `POST /api/merge-plan` 提交，最后 `POST /api/merge`（`confirm:true`）。

## 10. 常见问题

- `ocr-check` 显示不可用：检查 `.env` 中的绝对路径、Tesseract 中文语言包和 `TESSDATA_PREFIX`；PDF 还要检查 Poppler。
- 图片引用为空：先对对应 batch 执行 `repair`；raw OCR 和 normalization 不会被当成最终引用。
- `merge` 要求确认：这是多来源安全边界，先查看/编辑 `merge_plan.json`，再执行 `--confirm`。
- 找不到合并 unit：先运行 `sources <VAULT>`，复制完整的 `unit_id`；批次选择也可以直接使用 batch id。
- Codex 超时：降低 `--speed` 或提高 `.env` 中的 `READTRACE_TIMEOUT_SECONDS`；已完成页会从 checkpoint 恢复。
- Codex 报 `拒绝访问 (os error 5)` 或 `readonly database`：先单独运行 `codex --version`，确认 `READTRACE_CODEX_BIN` 指向真实入口。若错误来自 Codex 内置受限终端，当前进程无法写入 `CODEX_HOME`；请在普通 PowerShell/Windows Terminal 重新启动 ReadTrace，或改用 `--provider http`，不要把 `auth.json` 复制到项目。若报 `UnknownIssuer`/`invalid peer certificate`，则是受限环境的 CA 信任问题，也应切换到普通终端。ReadTrace 会照常记录失败调用，但因没有 usage 不计算费用。
