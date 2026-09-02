# ReadTrace 完整流程教程

这份教程是 CLI 的完整复现流程，从空 Workspace 开始，完成三种输入：一个 PDF、一个 Markdown 文件和两张图片。第一次使用或希望直接操作 GUI 时，请先看 [`QUICK_START.md`](QUICK_START.md)；Windows 的 Tesseract/Poppler 安装也在那里按步骤说明。正文保留已在 Windows 验证的 PowerShell 记录。macOS 先按 [`GITHUB_AND_DEVICE_SETUP.md`](GITHUB_AND_DEVICE_SETUP.md) 安装依赖，再将 Windows 路径换成正斜杠路径、PowerShell 反引号换成 shell 反斜杠；所有 ReadTrace CLI 参数保持不变。命令都从项目根目录运行，也就是：

```powershell
Set-Location E:\AI_diary\summer_project
```

文中的 `$workspace` 是 Workspace 根目录，`$vault` 是其中一个 Vault；两者不是同一个目录：

```text
E:\AI_diary\summer_project\workspace\tutorial-demo                 # Workspace
E:\AI_diary\summer_project\workspace\tutorial-demo\vaults\教程示例  # Vault
```

## 1. 准备环境

项目根目录的 `.env` 已配置 Tesseract、Poppler 和清华 OpenAI-compatible 网关时，先检查 OCR：

```powershell
cargo run --quiet -p readtrace-cli -- ocr-check
```

需要真实 LLM 时，确认 `.env` 至少包含 `READTRACE_BASE_URL`、`READTRACE_ENDPOINT_PATH`、`READTRACE_MODEL` 和 `READTRACE_API_KEY_ENV`。不要把 Key 写进命令或提交到 Git。

创建一个独立 Workspace 和 Vault：

```powershell
$workspace = ".\workspace\tutorial-demo"
cargo run --quiet -p readtrace-cli -- workspace-init $workspace
cargo run --quiet -p readtrace-cli -- vault-create $workspace "教程示例"
$vault = cargo run --quiet -p readtrace-cli -- vault-path $workspace "教程示例"
```

查看结构时，默认是人类可读摘要；需要脚本处理完整 JSON 时才加 `--format json`：

```powershell
cargo run --quiet -p readtrace-cli -- ls $workspace
cargo run --quiet -p readtrace-cli -- ls $vault
cargo run --quiet -p readtrace-cli -- sources $vault
```

## 2. 导入三个批次

本教程用到的输入如下。两张图片必须放在只包含图片的文件夹中，否则 `import-folder` 会把其中的 PDF/Markdown 也一起导入。

```text
tmp/tutorial-input/assignment.pdf
tmp/tutorial-input/story.md
tmp/tutorial-images/scene-01.png
tmp/tutorial-images/scene-02.png
```

导入 PDF：

```powershell
$pdf = cargo run --quiet -p readtrace-cli -- --format json `
  import-file $vault ".\tmp\tutorial-input\assignment.pdf" --mode book | ConvertFrom-Json
$pdfBatch = $pdf.batch_id
```

导入 Markdown：

```powershell
$md = cargo run --quiet -p readtrace-cli -- --format json `
  import-file $vault ".\tmp\tutorial-input\story.md" --mode game_dialogue | ConvertFrom-Json
$mdBatch = $md.batch_id
```

导入两张图片：

```powershell
$images = cargo run --quiet -p readtrace-cli -- --format json `
  import-folder $vault ".\tmp\tutorial-images" --mode game_dialogue --order filename | ConvertFrom-Json
$imagesBatch = $images.batch_id
```

导入只负责保存 batch 和原素材。复制是默认行为；不想复制时，在 `import-file` / `import-folder` 后加 `--no-copy`，此时 manifest 会保存外部路径。

## 3. Markdown/TXT：直接进入 clean 或走 LLM

### 3.1 不调用 LLM，直接发布

单个 `.md` 或 `.txt` 已经是可读文本，不必浪费一次模型调用。使用 `direct-clean`：

```powershell
cargo run --quiet -p readtrace-cli -- --format json `
  direct-clean $vault $mdBatch --clean-name "教程/markdown-直达"
```

最终文件是：

```text
$vault\clean\教程\markdown-直达\document.md
```

它仍然包含 `rt:block` 来源锚点，因此检索和引用可以回到原始 `story.md`。`direct-clean` 只接受一个 TXT/MD batch；多文件请先用 `sources` 查看单元，再走合并计划。

TXT 使用完全相同的命令：先 `import-file`，再把返回的 `batch_id` 交给 `direct-clean`。

### 3.2 需要 LLM 改写

如果文本虽可读取，但希望统一说话人、修正错字或按自定义提示词重写，仍可走标准流程：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault $mdBatch --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $mdBatch
cargo run --quiet -p readtrace-cli -- repair $vault $mdBatch `
  --provider http --model glm-5.2 --thinking none --speed low
cargo run --quiet -p readtrace-cli -- build $vault $mdBatch `
  --clean-name "教程/markdown-llm"
```

提示词优先读取 `$vault\prompts\repair.md`，也可以用 `repair --prompt-file <文件>` 临时替换。`prompts/profile.md` 适合放角色别名等上下文。每页完整结果写在 `generated/<batch_id>/repair/`，原始素材和 raw OCR 不会被覆盖。

## 4. PDF：按页 OCR，之后修复与合并

先执行 OCR 和确定性规范化：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault $pdfBatch --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $pdfBatch
```

PDF 会显示两段进度：先是栅格化页数，再是 Tesseract 页数。例如 26 页文件会出现 `rendering PDF (26 pages)` 和 `OCR page n/26`。PDF 的一个文件也可能包含很多页，但仍然只算一个 source；页级锚点会写成 `assignment.pdf:page:1`、`assignment.pdf:page:2` 等。

用真实 HTTP 模型修复 PDF：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $pdfBatch `
  --provider http --model glm-5.2 --thinking none --speed low
```

如果网关暂时不可用，可以先用 Mock 验证本地流程，之后用 `--refresh` 切回 HTTP 重跑；Mock 不计费：

```powershell
cargo run --quiet -p readtrace-cli -- repair $vault $pdfBatch `
  --provider mock --refresh
```

## 5. 两张图片：人工确认合并，再发布 clean

图片批次必须先 OCR 和规范化：

```powershell
cargo run --quiet -p readtrace-cli -- ocr $vault $imagesBatch --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $imagesBatch
```

LLM 修复可以选择清华接口、Codex CLI 或 Mock：

```powershell
# 清华 GLM-5.2，关闭思考，适合速度优先
cargo run --quiet -p readtrace-cli -- repair $vault $imagesBatch `
  --provider http --model glm-5.2 --thinking none --speed low

# Codex CLI，使用本机登录态；必须安装并登录 codex
cargo run --quiet -p readtrace-cli -- repair $vault $imagesBatch `
  --provider codex-cli --preset codex-luna --speed high

# 没有网络或 Codex 时，仅验证修复写盘和后续合并
cargo run --quiet -p readtrace-cli -- repair $vault $imagesBatch `
  --provider mock --refresh
```

多来源合并不会悄悄拼接文件。先预览计划：

```powershell
cargo run --quiet -p readtrace-cli -- --format json merge $vault $imagesBatch
```

确认页序没有问题后再写入 revision 和 clean：

```powershell
cargo run --quiet -p readtrace-cli -- --format json merge $vault $imagesBatch `
  --confirm --clean-name "教程/图片场景"
```

结果位于：

```text
$vault\clean\教程\图片场景\document.md
```

如果某个视觉页修复失败，默认的合并会拒绝生成；只有确认接受规范化 OCR 时才加 `--allow-unrepaired`。该结果会带 warning，且不应作为可靠问答证据。

## 6. 检索和引用问答

搜索只查 `clean/`，不会把 raw OCR 或 normalization 当作资料：

```powershell
cargo run --quiet -p readtrace-cli -- search $vault "舞台"
```

需要完整结构（包括上下文和可引用的来源锚点）时：

```powershell
cargo run --quiet -p readtrace-cli -- --format json search $vault "舞台"
```

注意：问答的 `--source-ref` 要填搜索结果里的来源锚点，而不是 `clean/.../document.md` 路径。例如图片批次的两个来源是：

```text
sources/<imagesBatch>/scene-01.png:page:1
sources/<imagesBatch>/scene-02.png:page:1
```

使用引用询问模型：

```powershell
cargo run --quiet -p readtrace-cli -- --format json answer $vault `
  "这段剧情讲了什么？" --provider http --model glm-5.2 --thinking none `
  --source-ref "sources/$imagesBatch/scene-01.png:page:1" `
  --source-ref "sources/$imagesBatch/scene-02.png:page:1"
```

没有网络时，可以用 Mock 验证引用确实被送进回答上下文：

```powershell
cargo run --quiet -p readtrace-cli -- --format json answer $vault `
  "这段剧情讲了什么？" --provider mock `
  --source-ref "sources/$imagesBatch/scene-01.png:page:1" `
  --source-ref "sources/$imagesBatch/scene-02.png:page:1"
```

输出会同时包含 `answer` 和 `source_refs`。同一会话可以把返回的 `session_id` 传给下一次 `answer --session-id ...`。

## 7. 网页端对应操作

启动服务：

```powershell
cargo run --quiet -p readtrace-cli -- serve .\workspace\tutorial-demo --bind 127.0.0.1:8787
```

打开 <http://127.0.0.1:8787/>，在“导入队列”中：

1. 选择文件或输入本机路径；
2. 填写可选的 clean 名称；
3. 在“导入后处理”选择“只导入，稍后处理”“TXT/MD 直接发布到 clean（不调用 LLM）”“导入后进入批次处理页”或“全部运行 OCR、修复并发布 clean”；
4. 处理页可以单独运行 OCR、规范化、LLM 修复、合并；
5. 文件浏览的 `clean/` 文件可预览，Markdown 可以直接编辑并保存；
6. 检索页只搜索 clean；对话页点击“添加引用”，在 clean 文件树中多选文件后提问。

网页里的来源选择、模型、None/Low/Mid/High 挡位和 API Key 管理与 CLI 使用同一套 Provider profile。Key 只保存在本机配置，不会进入项目 Git。

## 8. GUI 实际验收记录（2026-09-02）

这一次验收全程通过网页端完成，没有用 CLI 代替按钮操作。导入队列使用“本机路径”输入框（它和文件选择器写入同一队列），加入了三项：

| 队列项 | GUI 选项 | 观察到的结果 |
| --- | --- | --- |
| `t02-ai-agent.pdf` | `book`，只导入 | 新建 `batch-1975ea97-946f-4072-897b-3b881aa5dbad`；处理页显示 `OCR 处理中… 12/26 · rendered PDF page 14/26`，随后变为 `OCR 已完成 · 26` |
| `story.md` | `game_dialogue`，直达 `GUI/markdown` | 新建 `batch-c74b2a8a-f1f0-45f0-a2c0-1b9e9ab68bf1`；直接生成 `clean/GUI/markdown/document.md`，没有调用 LLM |
| `tutorial-images`（两张 PNG） | `game_dialogue`，真实 OCR，先用 Mock 自动合并，再切换真实 `GLM-5.2`、`Low` 重跑修复 | 新建 `batch-5b39cb09-6497-4b61-a2c7-1add0ec693de`；GUI 显示 `LLM 修复 已完成 · 2/2`，确认合并后更新 `clean/GUI/images/document.md` |

随后在 GUI 处理页手动完成了 PDF 的 OCR、规范化（114 处确定性修改）、Mock LLM 修复（26/26 页），先预览合并计划，再确认生成 `clean/GUI/pdf/document.md`。预览阶段在修复完成后只提示“请检查合并预览并确认”，不再把正常的确认步骤误报成未修复 OCR。

最后在“阅读与问答”页打开“添加引用”，从 clean 文件树多选 `clean/GUI/images/document.md` 和 `clean/GUI/markdown/document.md`，提问“这段剧情讲了什么？”。新建对话后页面显示“文件引用 2 个”，回答中带有两个引用块和来源锚点，统计显示 `Token 4,267 · $0.000000`；图片引用的内容来自真实 GLM 修复后的 `revision: 0002`。这验证了 GUI 的导入、清洗、修复、合并、clean 浏览、引用和问答链路是连通的；这次回答使用 Mock 只是为了让引用验证不受网络波动影响。

另外，随后在同一处理页把单页 `story.md` 切换到真实 `GLM-5.2`、`Low` 速度，点击“开始修复”，GUI 显示 `LLM 修复 已完成 · 1/1`。账本记录该次调用的 `input_tokens=472`、`output_tokens=78`、`total_tokens=550` 和 `request_id`。当次验证发生时尚未内置 GLM‑5.2 价格，所以原始记录费用未估算；当前版本已加入 2026-09-02 官方价格 `$1.40/$0.26/$4.40`，读取账本或执行 `usage` 时可依据已有 usage 回填。两张 PNG 也用同一控件重跑了真实修复，GUI 显示 `2/2`，账本分别记录 1,167 和 1,136 个总 Token。此前失败的 502 请求也会明确显示失败，不会覆盖已有产物。

## 9. CLI 与本地实际验证记录（2026-09-02）

本教程在当前项目根目录实际执行过以下批次：

| 输入 | batch | 结果 |
| --- | --- | --- |
| `assignment.pdf` | `batch-ac72cb31-0540-40b0-a2f5-37f1090534ca` | 真实 OCR 26 页，显示渲染和 OCR 两段 `0/26…26/26` 进度；规范化 114 处；Mock 修复 26 页并发布 `clean/教程/作业说明/document.md` |
| `story.md` | `batch-df878536-4ed9-4685-a871-5a39d9dc27da` | `direct-clean` 成功发布 `clean/教程/markdown-直达/document.md`，没有 LLM 调用 |
| `scene-01.png`、`scene-02.png` | `batch-5c481f49-2b15-45c7-a0a2-05c6ceba6e5a` | 真实 OCR 2 页，规范化 23 处；Mock 修复 2 页；预览合并计划后确认，发布 `clean/教程/图片场景/document.md` |

图片批次实际尝试了清华 HTTP 修复和 `ai-check`。当时网关返回 HTTP 502，因此没有把失败伪装成成功；本地修复、合并、clean 发布、搜索和带两个 `source_ref` 的引用问答均已用 Mock 完成验证。网关恢复后，直接把同一命令中的 `--provider mock --refresh` 换成 `--provider http --model glm-5.2 --thinking none --speed low` 即可重跑，历史失败调用会保留在 `$vault\runtime\calls.jsonl`。

重复执行 `usage` 不会重复计费；账本按 `call_id` 合并。HTTP 成功响应里的 input/cached/output/total Token 和模型价格会写入账本，Mock 明确记为 `$0`。

## 10. 常见错误

- `multiple sources require confirmation`：先运行 `merge` 预览，再加 `--confirm`；不要直接猜页序。
- `visual page ... has no full-page repair result`：先完成 repair；临时接受 OCR 才使用 `--allow-unrepaired`。
- `requested source_refs did not match this Vault`：`--source-ref` 必须来自 `--format json search` 的 `source_refs`，不能填 clean 文件路径。
- `tesseract not found`：运行 `ocr-check`，在 `.env` 填 `READTRACE_TESSERACT_BIN` 和 `TESSDATA_PREFIX`。
- HTTP 502/5xx：这是上游网关暂时不可用；用 `ai-check` 单独确认，再用 `--refresh` 重试，不要删除已有 raw/repair 文件。
