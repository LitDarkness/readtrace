# ReadTrace CLI：从零完成一次完整流程

这是一份可以直接照着执行的示例。假设你第一次打开项目，不知道 Workspace、Vault、batch 或 `source_ref` 是什么，也不想手工猜目录。

本教程在 PowerShell 中执行。所有命令都从项目根目录开始；如果你的项目不在 `E:\AI_diary\summer_project`，只需要把下面的 `$project` 改成实际路径。

## 1. 先把四种路径分清楚

```powershell
Set-Location E:\AI_diary\summer_project
$project = (Get-Location).Path
$workspace = Join-Path $project "workspace"
$vault = Join-Path $workspace "vaults\cli-demo"
$sample = "E:\AI_diary\tests\1.png"
```

这里有四个不同概念：

| 名称 | 本教程中的值 | 作用 |
| --- | --- | --- |
| 项目根目录 | `$project` | Rust workspace；在这里运行 `cargo` |
| Workspace | `$workspace` | 管理多个 Vault 的目录，根下有 `workspace.json` |
| Vault | `$vault` | 一个独立资料库，保存 source、raw、generated、clean、notes、sessions 和 runtime |
| 外部输入 | `$sample` 或一个输入文件夹 | 还没有进入 Vault 的原始文件 |

`raw`、`generated`、`clean` 等目录都是 Vault 的子目录，不是项目根目录下的公共目录。项目根目录里的 `tmp` 只用于测试和临时产物；运行费用扫描时可以把它一起纳入。

如果你从别的目录启动命令，CLI 不会自动找到项目 `.env`。最稳妥的做法是每次先执行 `Set-Location E:\AI_diary\summer_project`。

## 2. 第一次检查环境

先编译整个 workspace：

```powershell
cargo build --workspace
```

### 2.1 检查 OCR

```powershell
cargo run --quiet -p readtrace-cli -- ocr-check
```

默认输出是人类摘要，应该能看到类似结果：

```text
ocr_languages: chi_sim+eng
pdftoppm_available: true
tesseract_available: true
```

如果你需要保存完整路径和配置诊断，保留原始 JSON：

```powershell
cargo run --quiet -p readtrace-cli -- --format json ocr-check |
  Tee-Object -FilePath .\tmp\ocr-check.json
```

以后可以这样查看它的字段：

```powershell
$ocr = Get-Content .\tmp\ocr-check.json -Raw | ConvertFrom-Json
$ocr | Select-Object tesseract_available, tesseract_bin,
  pdftoppm_available, pdftoppm_bin, ocr_languages, tessdata_prefix |
  Format-List
```

`.env` 中的 `READTRACE_TESSERACT_BIN`、`READTRACE_PDFTOPPM_BIN`、可选的 `READTRACE_PDFINFO_BIN`、`READTRACE_OCR_DPI`、`READTRACE_OCR_CONCURRENCY` 和 `TESSDATA_PREFIX` 会在每次 CLI 启动时自动加载。当前配置使用项目内的可复现路径，因此不要求系统 PATH 里另有一个 `tesseract` 命令。TXT/Markdown 不依赖 OCR；PDF 需要 `pdfinfo` 读取页数并由 `pdftoppm` 栅格化。单个 25 页 PDF 会显示 `0/25` 渲染、逐页 `n/25` 和最终 `25/25`；默认 4 路 Tesseract 并行，结果仍按原页序落盘。默认 DPI 为 200，若需要速度优先可在确认字小仍可识别后调到 150。

### 2.2 检查 LLM Provider

先用本地 Mock 检查 CLI 本身，不产生网络请求：

```powershell
cargo run --quiet -p readtrace-cli -- ai-check --provider mock
```

使用 `.env` 里的 OpenAI-compatible Endpoint（例如学校网关和 GLM）：

```powershell
cargo run --quiet -p readtrace-cli -- ai-check --provider http --speed low
```

使用本机 Codex CLI 和 Luna：

```powershell
cargo run --quiet -p readtrace-cli -- ai-check `
  --provider codex-cli --preset codex-luna --speed high
```

`ai-check` 会把探测调用也写入 `runtime/calls.jsonl`，所以不要把它当作“零费用”调用。探测结果里的 Token 如果是 `null`，表示 Provider 没有返回 usage，而不是 CLI 猜测失败。

查看当前 Endpoint、模型、鉴权字段和挡位映射：

```powershell
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- provider-check --preset codex-luna --speed high
```

完整 JSON 仍然可以显式要求：

```powershell
cargo run --quiet -p readtrace-cli -- --format json provider-check |
  ConvertFrom-Json | Format-List
```

HTTP Provider 从 `.env` 读取这些非秘密配置：

```dotenv
READTRACE_BASE_URL=https://school.example/v1
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.3-flash
READTRACE_API_KEY_ENV=READTRACE_API_KEY
READTRACE_AUTH_HEADER=Authorization
READTRACE_AUTH_SCHEME=Bearer
READTRACE_RESPONSE_FORMAT=json_object
READTRACE_MAX_TOKENS_FIELD=max_tokens
```

密钥只放在 `.env` 的 `READTRACE_API_KEY` 或指定的环境变量中，不要写进文档、Vault 或 Git。`--provider http` 使用这个兼容接口；`--provider codex-cli` 使用本机 Codex CLI，二者共用同一套修复和问答协议。

注意：这里的 Codex 是 shell 可执行的 CLI，不是桌面 GUI。ReadTrace 会先在当前 `PATH` 查找 `codex.exe`/`codex.cmd`/`codex.bat`/`codex.ps1`，Windows 上还会尝试 Codex Desktop 的本地安装目录。若自动发现失败，在项目根目录 `.env` 中指定真实路径：

```dotenv
READTRACE_CODEX_BIN=C:/Users/<用户名>/AppData/Local/OpenAI/Codex/bin/<版本目录>/codex.exe
```

然后重新打开 PowerShell，再运行 `Get-Command codex`、`where.exe codex` 和 `ai-check`。只有 GUI 登录或在 GUI 中选中 Luna，并不能让 Rust 自动调用它；没有 CLI 时改用 `--provider http`（例如 GLM）。

速度挡位统一为：

| 命令参数 | HTTP/GLM | Codex/Luna | 适合场景 |
| --- | --- | --- | --- |
| `--speed low` | `thinking=low` | `low` | 先跑通、批量速度优先 |
| `--speed mid` | `thinking=medium` | `medium` | 日常平衡 |
| `--speed high` | `thinking=high` | `high` | 复杂剧情和最终质量 |

也可以直接传 `--model` 或 `--thinking`。`--preset codex-luna` 会选择 `gpt-5.6-luna`；如果想明确写出模型，可以使用 `--provider codex-cli --model gpt-5.6-luna --speed high`。

一次 repair 会把多个页面并行送入模型，默认上限是 4。可在项目根目录 `.env` 中调整：

```dotenv
READTRACE_LLM_CONCURRENCY=4   # 允许 1..64；设为 1 即串行
```

并行只影响在途请求数量，不改变 page/source 的原始顺序；每页仍独立写入 checkpoint，中断后可以续跑。

## 3. 创建或选择 Vault

初始化 Workspace（已经存在时不会破坏已有 Vault）：

```powershell
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
```

创建一个新的 Vault：

```powershell
cargo run --quiet -p readtrace-cli -- vault-create $workspace cli-demo
```

如果提示同名 Vault 已存在，不要删除它；直接查看并继续使用：

```powershell
cargo run --quiet -p readtrace-cli -- vault-path $workspace cli-demo
$vault = (cargo run --quiet -p readtrace-cli -- vault-path $workspace cli-demo).Trim()
```

查看 Workspace 的 Vault：

```powershell
cargo run --quiet -p readtrace-cli -- ls $workspace
```

`ls` 默认是结构化摘要，例如：

```text
workspace: E:\AI_diary\summer_project\workspace
vaults:
  - cli-demo  (id: ..., path: vaults/cli-demo)
```

需要机器读取时才加 `--format json`：

```powershell
cargo run --quiet -p readtrace-cli -- ls $workspace --format json |
  Out-File -Encoding utf8 .\tmp\workspace.json
$workspaceInfo = Get-Content .\tmp\workspace.json -Raw | ConvertFrom-Json
$workspaceInfo.vaults | Select-Object name, vault_id, relative_path | Format-Table
```

查看某个 Vault 的 batch 和可合并单元：

```powershell
cargo run --quiet -p readtrace-cli -- ls $vault
```

## 4. 导入：一个文件、一个文件夹，或不复制原素材

ReadTrace 当前明确支持：

- `.txt`、`.md`：直接作为文本来源，不走图像 OCR。
- `.png`、`.jpg`、`.jpeg`、`.webp`、`.bmp`：交给 Tesseract OCR。
- `.pdf`：先用 Poppler 转页，再交给 Tesseract OCR。
- 其它扩展名：文件夹导入时跳过，并记录在 `skipped_files`；不会默默当成文本。

### 4.1 导入示例 PNG

手工查看时直接运行即可：

```powershell
cargo run --quiet -p readtrace-cli -- import-file $vault $sample
```

命令返回的 `batch_id` 是后续 OCR、repair 和 build 的批次编号。脚本要可靠提取它时，显式使用 JSON 格式：

```powershell
$importText = (& cargo run --quiet -p readtrace-cli -- --format json `
  import-file $vault $sample) -join "`n"
$batch = $importText | ConvertFrom-Json
$batchId = $batch.batch_id
"batch_id = $batchId"
$batch.source_files | Select-Object source_id, relative_path, kind, copied |
  Format-Table
```

可以把完整响应保存下来，之后不必重新导入：

```powershell
$importText | Set-Content -Encoding utf8 .\tmp\import-$batchId.json
```

默认会把原图复制到 `$vault\sources\<batch_id>\`。如果素材很大、只想保留引用：

```powershell
cargo run --quiet -p readtrace-cli -- import-file $vault $sample --no-copy
```

此时 batch manifest 保存外部绝对路径；原文件被移动或删除后，后续 OCR 会失败，这是 `--no-copy` 的明确代价。

### 4.2 导入文件夹

先把同一组页面放在一个文件夹中，再运行：

```powershell
$inputFolder = "D:\captures\chapter-03"
cargo run --quiet -p readtrace-cli -- import-folder $vault $inputFolder --order filename
```

文件夹导入产生一个 batch，但每个文件仍然是独立的 source unit。排序规则默认是文件名；不要依赖 Windows 资源管理器当前排序。查看导入清单：

```powershell
cargo run --quiet -p readtrace-cli -- ls $vault
cargo run --quiet -p readtrace-cli -- sources $vault --kind source
```

如果要让脚本得到 `skipped_files`，使用：

```powershell
$folderText = (& cargo run --quiet -p readtrace-cli -- --format json `
  import-folder $vault $inputFolder --order filename) -join "`n"
$folderBatch = $folderText | ConvertFrom-Json
$folderBatch | Select-Object batch_id, copy_sources, status | Format-List
$folderBatch.skipped_files | ForEach-Object { "skipped: $_" }
```

## 5. OCR、规范化和 LLM 修复

### 5.1 分阶段执行（推荐第一次使用）

先用刚才得到的 `$batchId`：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault $batchId --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $batchId
```

`ocr` 只写入 `raw/<batch_id>/`；`normalize` 只做确定性的空白、标点和换行清理。它不会替代 LLM 的语义修复。

接着选择一个模型。开发时建议先用 Mock 验证文件链路：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider mock --speed low
```

真实学校 GLM：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider http --model glm-5.3-flash --speed mid
```

真实 Codex Luna：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider codex-cli --preset codex-luna --speed high
```

如果只是比较速度，可以分别用同一个 batch 的 checkpoint 做三次测试：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider codex-cli --preset codex-luna --speed low
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider codex-cli --preset codex-luna --speed mid --refresh
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider codex-cli --preset codex-luna --speed high --refresh
```

`--refresh` 会重跑已有页，因而会产生新的 API 调用；不要在大批量资料上无意使用它。正常重复执行 `repair` 会复用已完成的 page checkpoint。

修复提示词可以由用户直接编辑：

```powershell
$prompt = Join-Path $vault "prompts\repair.md"
notepad $prompt
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider http --prompt-file $prompt --speed high
```

若要把规则分成全局默认和 Vault 专用规则，参考 `prompts\text_repair_system.md`、`$vault\prompts\repair.md` 和 `$vault\prompts\profile.md`。LLM 默认直接生成完整修复文本，不把每个字符拆成提议；原始图片和 raw OCR 始终保留，方便人工对照。

### 5.2 一键执行

单个文件不需要合并，可以一条命令完成导入 → OCR → 规范化 → repair → build：

```powershell
cargo run --quiet -p readtrace-cli -- process $vault $sample `
  --ocr real --llm codex-cli --preset codex-luna --speed high
```

学校 GLM 的一键版本：

```powershell
cargo run --quiet -p readtrace-cli -- process $vault $sample `
  --ocr real --llm http --model glm-5.3-flash --speed mid
```

如果想跑到 repair 就停下来人工检查：

```powershell
$processText = (& cargo run --quiet -p readtrace-cli -- --format json `
  process $vault $sample --ocr real --llm http --speed mid --no-apply) -join "`n"
$process = $processText | ConvertFrom-Json
$batchId = $process.batch_id
$process | Select-Object batch_id, repaired_pages, review_only, repair_directory |
  Format-List
```

`--no-apply` 只是不生成最终 revision，不会丢弃 OCR 或 repair 结果。检查无误后手工执行：

```powershell
cargo run --quiet -p readtrace-cli -- build $vault $batchId
```

## 6. 多文件：不合并、人工合并、自动确认

### 6.1 不合并

文件夹导入后，如果每张图应该独立成为文档，不要调用 `merge`。对单 source batch，直接 build；对多 source batch，可以分别处理并分别构建。`merge` 不是必须步骤。

如果视觉页没有成功的 LLM 修复，但你仍要先交付一份可人工校对的文件，可以显式走应急路径：

```powershell
cargo run --quiet -p readtrace-cli -- build $vault $batchId --allow-unrepaired
```

生成的 revision 和 `manifest.json` 会带 warning；该文本不会被当作可引用的最终证据。默认不加此选项时，CLI 仍会拒绝这种 build。

### 6.2 人工确认同一 batch 的合并

对文件夹导入的多 source batch，先运行 OCR 和 repair。然后只预览计划，不写最终文档：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault $batchId
```

打开计划文件：

```powershell
$planPath = Join-Path $vault "generated\$batchId\merge_plan.json"
notepad $planPath
```

计划中的 `pages` 是最终页序。可以人工调整数组顺序；不要增删页面、修改 `unit_id`、`page_id` 或 `source_ref`。保存后确认：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault $batchId --confirm
```

确认时也可以选择上述应急路径（例如某一页的 Codex/GLM 调用失败）：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault $batchId --confirm --allow-unrepaired
```

如果计划被改坏，CLI 会拒绝构建，而不是静默产生不完整文档。确认成功后，revision 位于 `$vault\generated\<batch_id>\...\revisions\000N\document.md`。

### 6.3 自动确认同一 batch

如果已经接受文件名排序，不需要打开计划，可以在 process 时加：

```powershell
cargo run --quiet -p readtrace-cli -- process $vault $inputFolder `
  --ocr real --llm http --speed mid --confirm-merge
```

这仍然是确定性的文件名顺序自动确认，不是让 LLM 猜顺序。当前设计没有开启“LLM 自动跨文档拼接”，因为那会改变原素材边界。

### 6.4 跨 batch 合并（最小单元是文件或 clean Markdown）

先列出当前 Vault 的所有可选单元：

```powershell
cargo run --quiet -p readtrace-cli -- sources $vault
```

输出中的 `unit_id` 才是后续命令要复制的值。`source:<batch>/<source>` 代表一个导入文件；`clean:clean/<name>/document.md` 代表一个人工整理或自动发布的 clean 文件；`clean:generated/.../current.md` 仍代表 generated 侧的合并单元，但它不进入检索和引用。

假设 `sources` 输出了下面两行：

```text
source:batch-a/src-aaa...  source  batch-a  1  sources/batch-a/page-01.png
source:batch-b/src-bbb...  source  batch-b  1  sources/batch-b/page-02.png
```

把两个完整 `unit_id` 按需要的顺序交给 CLI。第一次只预览：

```powershell
$unitA = "source:batch-a/src-aaa..."
$unitB = "source:batch-b/src-bbb..."
$planText = (& cargo run --quiet -p readtrace-cli -- --format json `
  merge-units $vault $unitA $unitB) -join "`n"
$plan = $planText | ConvertFrom-Json
$plan.plan.pages | Select-Object ordinal, unit_id, page_id, source_ref | Format-Table
$mergeId = $plan.plan.merge_id
```

如果顺序正确，确认；如果不正确，打开计划文件，只调整 `pages` 顺序后再确认：

```powershell
$crossPlanPath = Join-Path $vault "generated\merges\$mergeId\merge_plan.json"
notepad $crossPlanPath
cargo run --quiet -p readtrace-cli -- merge-units $vault $mergeId --confirm
```

跨 batch 的视觉 source 同样可以在确认时加 `--allow-unrepaired`，但结果会标记为“含未修复 OCR”。

把人工 Markdown 放到 `$vault\clean\manual.md` 后，可以和图片来源一起合并：

```powershell
cargo run --quiet -p readtrace-cli -- sources $vault --kind clean
cargo run --quiet -p readtrace-cli -- merge-units $vault `
  $unitA "clean:clean/manual.md"
```

跨 batch 的计划永远要求人工确认；这样既支持跨批组织，也不会让一次错误的自动排序覆盖多个来源。

## 7. 查看、管理和恢复

### 7.1 找文件

```powershell
cargo run --quiet -p readtrace-cli -- ls $vault
cargo run --quiet -p readtrace-cli -- sources $vault
```

当前正文通常在 `generated` 下的最新 revision。用 PowerShell 搜索：

```powershell
Get-ChildItem $vault\generated -Recurse -Filter document.md |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 5 FullName, LastWriteTime
```

查看正文或 repair 结果：

```powershell
Get-Content -LiteralPath "E:\path\to\document.md" -Encoding utf8
Get-Content -LiteralPath (Join-Path $vault "generated\$batchId\repair.json") -Raw |
  ConvertFrom-Json | Select-Object batch_id, pages, errors | Format-List
```

单页 repair JSON 中有 `source_ref` 和完整 `repaired_text`，可用来人工对照原图：

```powershell
Get-ChildItem (Join-Path $vault "generated\$batchId\repair") -Filter *.json |
  Select-Object -First 1 | Get-Content -Raw | ConvertFrom-Json |
  Select-Object page_id, source_ref, repaired_text | Format-List
```

### 7.2 搜索和索引

只有 `clean/` 下的最终可读 Markdown/TXT 会进入索引；已修复的图片/PDF、原始 TXT/Markdown 和 notes 必须先经 build 发布到 clean。raw OCR、generated 历史或没有 repair 的视觉页面不会被当作检索事实。

```powershell
cargo run --quiet -p readtrace-cli -- reindex $vault
cargo run --quiet -p readtrace-cli -- search $vault "沉默"
cargo run --quiet -p readtrace-cli -- search $vault "沉默" --scope generated
```

### 7.3 备份和恢复

`build` 每次生成新的 `revisions\000N\document.md`，不会覆盖原图、raw 或旧 revision。可以把 `current.md` 复制到人工编辑目录后再 build，也可以直接从旧 revision 恢复。

导出一个可迁移的 Vault（包括 sources、raw、generated、clean、notes、sessions、prompts 和 events）：

```powershell
cargo run --quiet -p readtrace-cli -- export-vault $vault .\tmp\cli-demo-export
```

`.readtrace\state.db` 是可重建索引和状态，不是提交物；导出命令不会把它当作业务正文。

## 8. 引用资料并和模型对话

没有显式证据时，`answer` 只做本地 SQLite 搜索；搜索本身不调用 LLM。要让模型只围绕指定内容回答，传入 `--source-ref`、`--quote` 或 `--quote-file`。

先从 `sources` 或 repair JSON 复制完整的 `source_ref`，不要手写一个相似路径：

```powershell
cargo run --quiet -p readtrace-cli -- sources $vault --kind source
```

然后提问：

```powershell
$ref = "sources/batch-a/page-01.png:page:1"
cargo run --quiet -p readtrace-cli -- answer $vault `
  "这一页发生了什么？" `
  --provider http --model glm-5.3-flash --speed mid `
  --source-ref $ref `
  --quote "这是剧情对话，英文字母说话人应理解为 Banished。"
```

也可以引用 clean Markdown：

```powershell
cargo run --quiet -p readtrace-cli -- answer $vault `
  "请总结这份人工整理内容的冲突。" `
  --provider codex-cli --preset codex-luna --speed high `
  --source-ref "clean:clean/manual.md"
```

图片/PDF 在完整 repair 之前会被拒绝作为证据；raw OCR 和单纯 normalization 也不能绕过这个限制。TXT/Markdown 原本就是可读文本，clean Markdown 也可以直接引用。

命令末尾会返回 `session_id`。继续追问时传回它：

```powershell
cargo run --quiet -p readtrace-cli -- answer $vault `
  "结合上一轮，指出仍然不确定的专名。" `
  --provider codex-cli --preset codex-luna --speed high `
  --session-id "session-..."
```

查看完整 session JSON：

```powershell
cargo run --quiet -p readtrace-cli -- --format json `
session-export $vault "session-..." |
  Out-File -Encoding utf8 .\tmp\session.json
```

## 9. 查看 Token、费用和调用明细

每个 `repair_page`、`answer` 和 `ai-check` 调用都会写入一个 JSONL ledger。Vault 默认是 `runtime/calls.jsonl`，`ai-check --ledger` 可以指定 `tmp\codex-check.jsonl` 之类的测试文件；`usage --scan-root` 会读取扫描根下所有 `.jsonl`，按 `call_id` 合并，避免临时测试调用被漏计或重复计费。记录包含 provider、model、阶段、耗时、request id、input/cached-input/output/reasoning/total Token、价格版本、三档单价、USD/CNY 和错误。

查看当前 Vault：

```powershell
cargo run --quiet -p readtrace-cli -- usage $vault
```

只看一个 batch：

```powershell
cargo run --quiet -p readtrace-cli -- usage $vault --batch-id $batchId
```

把 Vault、Workspace 和 `tmp` 测试调用合并扫描，并按 `call_id` 去重：

```powershell
$usagePath = Join-Path $project "deliverables\runtime-usage-all.json"
cargo run --quiet -p readtrace-cli -- usage `
  --scan-root $project --out $usagePath
```

需要人类可读的汇总：

```powershell
$usage = Get-Content -LiteralPath $usagePath -Raw | ConvertFrom-Json
$usage.summary | Format-List
$usage.calls |
  Select-Object call_id, phase, provider, model, elapsed_ms,
    input_tokens, cached_input_tokens, output_tokens, total_tokens, cost_usd, cost_cny |
  Format-Table
```

当前 `.env` 的 `READTRACE_USD_TO_CNY` 为 `6.8`。已知 OpenAI 模型（包括 GPT-5.6 Luna/Terra/Sol、GPT-5.5、GPT-5.4 系列和 GPT-4o Mini）以及 GLM 5.3 Flash 会根据价格表自动填入单价；Codex Luna 为 `$0.20/$0.02/$1.20`，GLM 5.3 Flash 为 `$0.15/$0.03/$0.50`（均为 input/cached/output，每百万 Token）。学校网关上的其它模型和自定义来源必须手工设置 `READTRACE_INPUT_PRICE`、`READTRACE_CACHED_INPUT_PRICE`、`READTRACE_OUTPUT_PRICE`，否则费用保持 `null`。如果 Provider 不返回 usage，Token 和费用同样保持 `null`；CLI 不会根据字符数伪造计费。修改汇率不会回写历史调用，旧记录保留当时的汇率。开发阶段和学校作业要求的 AI 对话历史/人时 Excel 仍然是独立人工整理的交付物。

## 10. 所有 JSON 命令的统一规则

以下命令默认给人类摘要：`vault-create`、`vault-list`、`ls`、`sources`、`provider-check`、`ocr-check`、`ai-check`、`import-file`、`import-folder`、`repair`、`process`、`build`、`delete-batch`、`delete-unit`、`merge`、`merge-units`、`usage`、`search`、`answer`、`session-export` 和 `progress`。

需要完整 JSON 时，在命令前或后加：

```powershell
--format json
```

例如：

```powershell
cargo run --quiet -p readtrace-cli -- --format json process $vault $sample `
  --ocr mock --llm mock > .\tmp\process.json
$process = Get-Content .\tmp\process.json -Raw | ConvertFrom-Json
$process | Select-Object batch_id, pages, repaired_pages, auto_built, artifact_absolute_path |
  Format-List
```

这是同一条命令的两个视图：默认摘要方便人工检查，`--format json` 保留脚本、审计和后续工具需要的完整结构。

## 11. 常见错误怎么处理

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| `tesseract not found` | `.env` 没有有效路径，且 PATH 也找不到 | 先检查 `ocr-check`；修正 `READTRACE_TESSERACT_BIN` 和 `TESSDATA_PREFIX` |
| 找不到 batch | 没有复制导入输出中的 `batch_id` | `ls $vault`，或重新用 `--format json import-file` 提取 |
| `build` 提示需要 repair | 图片/PDF 没有完整逐页修复 | 对同一 batch 执行 `repair`，再 build |
| `merge` 要求确认 | 一个 batch 含多个 source | 查看并编辑 `generated\<batch_id>\merge_plan.json`，再 `merge --confirm`；或 process 加 `--confirm-merge` |
| `merge-units` 找不到 unit | 手写了不完整的 source id | 先 `sources $vault`，复制整行的 `unit_id` |
| 引用为空或被拒绝 | 引用了 raw OCR 或未修复视觉来源 | 改用 repair 输出的 `source_ref`，或先完成 repair |
| Token 是 `null` | Provider 没有在响应中返回 usage | 保留记录，不能据此反推精确费用；检查 Provider 的 usage 支持 |
| Codex 运行太慢 | reasoning effort 或网络延迟高 | 先试 `--speed low`/`mid`，必要时提高 `.env` 的 `READTRACE_TIMEOUT_SECONDS` |
| Codex 报 `拒绝访问 (os error 5)`/`readonly database` | 受限宿主禁止 CLI 写入 `CODEX_HOME` | 在普通 PowerShell/Windows Terminal 重启 ReadTrace，或改用 HTTP/GLM；不要复制 `auth.json` |
| Codex 报 `UnknownIssuer`/`invalid peer certificate` | 当前宿主的 CA 信任链不完整 | 在普通终端重试，或修复 Windows 信任库/代理配置 |

到这里，一次完整的 CLI 流程就结束了：输入文件仍然可追溯，最终正文可人工编辑，合并顺序可审查，引用证据有边界，模型对话有 session，运行调用也有去重后的统计记录。

## 12. 实例：把 `E:\AI_diary\tests` 中的所有图片导入已有 Vault

这一节不创建新 Vault，直接使用已有的 `first_run`。如果你的 Vault 名称不同，只改 `$vault` 一行。

```powershell
Set-Location E:\AI_diary\summer_project
$project = (Get-Location).Path
$workspace = Join-Path $project "workspace"
$vault = Join-Path $workspace "vaults\first_run"
$imageFolder = "E:\AI_diary\tests"

cargo run --quiet -p readtrace-cli -- ls $workspace
cargo run --quiet -p readtrace-cli -- ls $vault
```

先确认输入目录里有哪些图片，不要假定文件名：

```powershell
Get-ChildItem -LiteralPath $imageFolder -Recurse -File |
  Where-Object { $_.Extension -in ".png", ".jpg", ".jpeg", ".webp", ".bmp" } |
  Select-Object FullName, Length, Extension | Format-Table
```

### 12.1 导入到已有 Vault

下面命令会递归导入这个目录中的所有支持图片，并按文件名排序。默认复制原图到 Vault 的 `sources/<batch_id>/`：

```powershell
$importText = (& cargo run --quiet -p readtrace-cli -- --format json `
  import-folder $vault $imageFolder --order filename) -join "`n"
$batch = $importText | ConvertFrom-Json
$batchId = $batch.batch_id
$batch | Select-Object batch_id, status, copy_sources | Format-List
$batch.source_files | Select-Object ordinal, source_id, kind, relative_path, copied |
  Format-Table -Wrap
$batch.skipped_files | ForEach-Object { "skipped: $_" }
```

此时只完成了“写入 Vault”，还没有 OCR。以后可以用下面的命令重新查看这次导入的文件和最小 unit：

```powershell
cargo run --quiet -p readtrace-cli -- ls $vault
cargo run --quiet -p readtrace-cli -- sources $vault --batch-id $batchId --kind source
```

如果不想复制图片，把导入命令改成：

```powershell
cargo run --quiet -p readtrace-cli -- import-folder $vault $imageFolder `
  --order filename --no-copy
```

`--no-copy` 只保存外部路径，删除 unit 时不会删除外部原图；但原图被移动后，OCR 将无法继续。

### 12.2 先 OCR，再选择修复模型

对这个 batch 执行真实 OCR：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault $batchId --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $batchId
```

修复可以选择不同 Provider。下面三条是三选一；输入 batch 相同，结果都写入同一 Vault 的逐页 checkpoint：

```powershell
# 离线验证文件链路，不访问网络
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider mock --speed low

# 清华/GLM 或其它 OpenAI-compatible 网关
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider http --model glm-5.3-flash --speed mid

# 本机 Codex CLI 的 Luna；可以把 high 改成 low 或 mid 比较速度
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider codex-cli --preset codex-luna --speed high
```

重复运行默认复用已经完成的页。若要把同一页交给另一个模型比较，必须明确加 `--refresh`，否则已有 checkpoint 会被复用：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $batchId `
  --provider codex-cli --preset codex-luna --speed low --refresh
```

查看每页修复结果：

```powershell
$repairText = (& cargo run --quiet -p readtrace-cli -- --format json `
  repair $vault $batchId --provider mock) -join "`n"
$repair = $repairText | ConvertFrom-Json
$repair.result_previews | Select-Object page_id, source_ref, repaired_text_preview |
  Format-Table -Wrap
```

真实模型的结果也位于 `$vault\generated\$batchId\repair\*.json`，完整正文不要只看 500 字预览。

### 12.3 三种合并选择

视觉文件不能在 repair 之前合并。下面三种方式都建立在 OCR 和逐页 repair 已经完成的前提上；如果直接对 raw OCR 执行 `merge`，CLI 会拒绝，这是为了避免把半成品写入最终文档。

#### 选择 A：不合并

如果两张图应当成为两个独立文档，可以分别选择 source unit，各自生成一个 revision。先列出 unit：

```powershell
$units = (& cargo run --quiet -p readtrace-cli -- --format json `
  sources $vault --batch-id $batchId --kind source) -join "`n" | ConvertFrom-Json
$units | Select-Object unit_id, path, page_ids | Format-Table -Wrap
```

对每一个 `unit_id` 单独确认一个只包含该文件的计划：

```powershell
$firstUnit = $units[0].unit_id
cargo run --quiet -p readtrace-cli -- merge-units $vault $firstUnit --confirm
```

这不是跨文件合并，而是把一个文件单独落成可搜索、可引用的 revision。也可以在导入时对每个文件分别使用 `import-file`，从源头得到多个单 source batch。

#### 选择 B：人工合并

先预览按文件名排序的计划：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault $batchId
$planPath = Join-Path $vault "generated\$batchId\merge_plan.json"
notepad $planPath
```

人工只调整 `pages` 的顺序，不增删页、不改 `page_id` 或 `source_ref`，然后确认：

```powershell
cargo run --quiet -p readtrace-cli -- merge $vault $batchId --confirm
```

#### 选择 C：自动确认合并

如果接受文件名顺序，可以让 `process` 对一个新的文件夹 batch 自动确认：

```powershell
cargo run --quiet -p readtrace-cli -- process $vault $imageFolder `
  --ocr real --llm http --model glm-5.3-flash --speed mid --confirm-merge
```

这会新建一个 batch。对本节已经导入并修复的 `$batchId`，直接执行 `merge $vault $batchId --confirm` 就是同样的确定性自动确认，不会再次调用 LLM。

### 12.4 查看合并后的文件

先让 CLI 告诉你最新 revision 在哪里：

```powershell
cargo run --quiet -p readtrace-cli -- ls $vault
Get-ChildItem $vault\generated -Recurse -Filter document.md |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 3 FullName, LastWriteTime
```

打开文件：

```powershell
Get-Content -LiteralPath "E:\path\shown\by\the\previous\command\document.md" -Encoding utf8
```

正文中的 source block 保留每一页的来源锚点；原图在 `$vault\sources\<batch_id>\`，raw OCR 在 `$vault\raw\<batch_id>\`，人工可直接对照或恢复旧 revision。

### 12.5 引用图片内容并提问“这段剧情讲了什么？”

不要猜 `source_ref`。从 repair 结果或 `sources` 的 JSON 中取出完整值：

```powershell
$sourceJson = (& cargo run --quiet -p readtrace-cli -- --format json `
  sources $vault --batch-id $batchId --kind source) -join "`n"
$sources = $sourceJson | ConvertFrom-Json
$refs = @($sources | ForEach-Object { $_.source_refs } | Where-Object { $_ })
$refs | ForEach-Object { "source_ref: $_" }
```

让另一个模型回答也可以，Provider 不必和 repair 相同。下面示例用 GLM 回答；把 `http` 换成 `codex-cli` 和 `--preset codex-luna` 即可改用 Luna：

```powershell
$answerArgs = @(
  "answer", $vault, "这段剧情讲了什么？",
  "--provider", "http", "--model", "glm-5.3-flash", "--speed", "mid"
)
foreach ($ref in $refs) {
  $answerArgs += @("--source-ref", $ref)
}
& cargo run --quiet -p readtrace-cli -- @answerArgs
```

如果只想引用其中一张图，使用 `$refs[0]`：

```powershell
cargo run --quiet -p readtrace-cli -- answer $vault `
  "这段剧情讲了什么？" `
  --provider codex-cli --preset codex-luna --speed high `
  --source-ref $refs[0]
```

命令会输出回答和 `session_id`。继续追问时复用这个 session；所有问答调用仍会进入 `runtime/calls.jsonl`。

## 13. 删除 batch 或 unit

删除是破坏性操作，所以两个命令都遵循“先预览、再确认”：没有 `--confirm` 时不会删除任何文件。

### 13.1 删除整个 batch

先看将要删除的范围：

```powershell
cargo run --quiet -p readtrace-cli -- delete-batch $vault $batchId
```

计划会列出 `raw/<batch_id>`、`sources/<batch_id>`、`generated/<batch_id>`，以及引用该 batch 的跨 batch merge 目录。确认无误后：

```powershell
cargo run --quiet -p readtrace-cli -- delete-batch $vault $batchId --confirm
```

会删除该 batch 的原图快照、OCR、repair、revision 和相关 merge 计划，并从 `metadata.json` 移除 batch。`runtime/calls.jsonl`、`events/events.jsonl` 会保留，SQLite 索引会自动重建；这样删除素材不会抹掉费用和过程审计。

### 13.2 删除一个 source/clean unit

先用 `sources` 找到完整的 `unit_id`：

```powershell
cargo run --quiet -p readtrace-cli -- sources $vault
$unitId = "source:<batch_id>/<source_id>"
cargo run --quiet -p readtrace-cli -- delete-unit $vault $unitId
```

确认计划后执行：

```powershell
cargo run --quiet -p readtrace-cli -- delete-unit $vault $unitId --confirm
```

行为取决于 unit 类型：

- 删除复制进 Vault 的 source：删除该文件和对应 raw page，整批 generated 输出会失效，剩余 source 保留但需要重新 OCR/repair。
- 删除 `--no-copy` 的 source：只删除 Vault 内的引用和派生结果，不删除外部原文件。
- 删除 `clean/` 下的 Markdown：删除该文件；引用它的跨 batch merge 计划也会删除。
- 如果 source 是该 batch 的最后一个文件，`delete-unit` 会升级为完整 batch 删除，避免留下空 batch。

需要脚本保留完整删除计划时：

```powershell
cargo run --quiet -p readtrace-cli -- --format json `
  delete-unit $vault $unitId > .\tmp\delete-unit-plan.json
```

删除前始终先检查计划；删除后可以用 `ls $vault`、`sources $vault` 和 `usage $vault` 验证结果。
