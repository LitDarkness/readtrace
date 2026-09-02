# Web / GUI 协议

服务端命令：

```console
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
```

打开 `http://127.0.0.1:8787/`。页面只调用下面的 JSON API；Vault 内的 Markdown/JSON 仍是最终数据源。启动参数中的路径决定初始上下文：传入 Workspace 目录可切换和创建多个 Vault；传入单个 Vault 仍可使用处理功能，但不能创建同级 Vault。

## 资源

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| `GET` | `/api/vault` | 当前 Vault 路径和 Workspace 信息 |
| `GET` | `/api/vaults` | Workspace 中的 Vault 列表和当前选择 |
| `POST` | `/api/vaults/select` | `{ "name_or_id": "..." }` 切换 Vault |
| `POST` | `/api/workspace/init` | `{ "path", "vault_name" }` 创建 Workspace、首个 Vault 并立即切换 |
| `POST` | `/api/vaults/create` | `{ "name", "select" }` 在当前 Workspace 创建 Vault |
| `GET` | `/api/batches` | 当前 Vault 的 batch 列表 |
| `POST` | `/api/import` | `{ "path", "mode", "no_copy" }` 导入文件或文件夹 |
| `POST` | `/api/import-upload` | `multipart/form-data`（`files` 可重复，另带 `mode`/`order`/`target`）从浏览器文件/文件夹选择器导入；上传内容始终复制到当前 Vault 的 `sources/` |
| `POST` | `/api/direct-clean` | `{ "batch_id", "clean_name" }` 将单个 TXT/MD batch 直接发布到 `clean/`，不调用 LLM；PDF、图片或多文件 batch 会返回错误 |
| `GET` | `/api/sources` | 列出 source/clean 最小合并单元 |
| `GET` | `/api/files?view=essential|all` | 列出当前 Vault 中可浏览的文件；默认只返回内容目录，`all` 显示 raw/runtime/events 等审计目录 |
| `GET` | `/api/file?path=...` | 返回文本/JSON 预览（最多 120,000 字符） |
| `POST` | `/api/file` | `{ "path": "clean/.../document.md", "content": "..." }` 保存 Markdown/TXT；clean 是推荐的人类编辑入口，raw、sources 和审计目录只读，并在成功后重建本地索引 |
| `GET` | `/api/file/raw?path=...` | 返回图片、PDF 或其它文件的安全原始流 |
| `POST` | `/api/delete-batch` | `{ "batch_id", "confirm" }` 预览或删除 batch |
| `POST` | `/api/delete-unit` | `{ "unit", "confirm" }` 预览或删除 source/clean unit |
| `GET` | `/api/providers` | 列出内置和自定义来源；只返回 `key_present`，绝不返回密钥 |
| `POST` | `/api/providers` | 创建或更新 OpenAI-compatible/Codex/Mock 来源；`api_key` 只写入本机配置 |
| `POST` | `/api/providers/delete` | 删除自定义来源，内置来源不可删除 |
| `POST` | `/api/providers/check` | 对指定来源发送最小探针，并把这次调用写入 runtime ledger |
| `GET` | `/api/prompts/repair` | 读取当前 Vault 的修复提示词；没有自定义文件时返回项目默认模板 |
| `POST` | `/api/prompts/repair` | `{ "content": "..." }` 保存到 `prompts/repair.md`；`{ "reset": true }` 恢复项目默认模板 |
| `GET` | `/api/sessions` | 列出当前 Vault 的对话摘要，按最近更新时间排序 |
| `GET` | `/api/sessions/{session_id}` | 恢复某个对话的消息、引用和会话状态 |
| `POST` | `/api/answer` | `{ "query", "source_refs", "quotes", "profile_id", "thinking", "session_id" }` 进行带证据的问答 |

`/api/import-upload` 的请求体上限为 512 MiB，单文件上限为 256 MiB、单次最多 20,000 个文件；文件名会在服务端规范化并拒绝绝对路径和 `..` 穿越。上传临时文件位于当前 Vault 的 `tmp/web-import/`，导入成功或失败后都会清理。

## 阶段任务

`POST /api/ocr`、`POST /api/repair` 返回：

```json
{ "ok": true, "task_id": "task-...", "batch_id": "batch-...", "status": "started" }
```

任务状态：

```json
{
  "task_id": "task-...",
  "kind": "repair",
  "batch_id": "batch-...",
  "status": "running",
  "current": 2,
  "total": 5,
  "message": "completed page 2",
  "error": null,
  "result": null
}
```

使用 `GET /api/tasks` 列表、`GET /api/tasks/{task_id}` 查询单项，使用 `POST /api/tasks/{task_id}/cancel` 取消。状态为 `running`、`completed`、`completed_with_errors`、`failed` 或 `cancelled`；其中 `completed_with_errors` 表示部分页面可用、部分页面失败，`failed` 表示没有可交付的修复页。取消不会删除已完成 checkpoint。旧的 `POST /api/cancel` 接受 batch id，保留作兼容入口。`POST /api/normalize` 可单独执行确定性空白清洗。

OCR 任务的 `current/total` 以页为单位。单个 PDF 会先显示 `0/25 · rendering PDF (25 pages)`，随后随着页级 Poppler 栅格化和 Tesseract 识别显示 `n/25 · rendered PDF page p/25`、`n/25 · OCR page p/25`，最终为 `25/25 · OCR complete (25 pages)`；多个文件的总数仍会在任务结束时收敛为实际页数。PDF 页数由 Poppler 的 `pdfinfo` 读取，读取失败时退回单进程栅格化并仍显示可用的实际页数。页面默认有界并行 4 路，可在 `.env` 用 `READTRACE_OCR_CONCURRENCY=1..16` 调整；完成顺序可以不同，但写入的 page number 和最终文档顺序稳定。

repair 请求的 `provider` 可选 `http`、`codex-cli`、`mock`，另可传 `profile_id`（来源与 API 页面保存的来源）、`preset`、`model`、`thinking` 或 `speed=low|mid|high`。并发上限由 `.env` 的 `READTRACE_LLM_CONCURRENCY` 控制。`POST /api/answer` 使用同一套 `profile_id`、`provider` 和 `thinking` 字段，因此处理和对话不会出现两套配置语义；为兼容旧网页，若 `provider` 本身是 profile id（如 `tsinghua-glm-5.2`），服务端会先按 profile 查找再解析 backend。

### 来源与密钥

Web 的“来源与 API”页提供四个内置来源：清华 GLM-5.3 Flash、清华 GLM-5.2、Codex Luna High 和 Mock。新增来源时填写 Base URL（或完整 Endpoint）、模型、认证头/方案、Token 字段、响应格式和三档价格；API Key 可以直接写入密码框，也可以只填写环境变量名。服务端默认把自定义来源保存到 Windows 的 `%LOCALAPPDATA%/ReadTrace/providers.json` 或 macOS 的 `~/Library/Application Support/ReadTrace/providers.json`（可用 `READTRACE_PROVIDER_STORE` 改位置）。若当前进程没有权限写用户配置目录，会自动回退到当前 Vault 的 `.readtrace/providers.json`；`.readtrace/` 与 `providers.json` 均被 `.gitignore` 忽略。响应只给出 `key_present`，浏览器刷新后也不会拿到明文 Key；保存时密码框留空表示保留原值，勾选“清除已保存的 Key”才会删除。

连接测试不是假的 UI 检查：它会使用当前来源的最小请求，显示 HTTP 状态、耗时、响应预览和 usage，并以 `purpose=provider_check` 追加到当前 Vault 的 `runtime/calls.jsonl`。如果服务没有返回 usage，Token/费用仍显示未知，而调用次数和失败状态照样统计。

## 合并和结果

- `POST /api/merge`：同 batch 预览或确认（`confirm:false/true`）。请求可带 `allow_unrepaired:true`，显式允许视觉页在没有修复 checkpoint 时使用规范化 OCR；响应和 revision manifest 会带警告。默认仍拒绝这种合并。
- `GET /api/merge-plan?batch_id=...`：读取可编辑计划。
- `POST /api/merge-plan`：提交 `{ "batch_id", "plan" }`；core 只允许重排现有页，拒绝伪造 `source_ref`、增删页或修改 source 元数据。
- `POST /api/merge-units`：跨 batch source/clean unit 预览或确认，也支持同样的 `allow_unrepaired` 显式选项；确认时可带 `clean_name`。
- `POST /api/build`：生成不可变 revision；可带 `allow_unrepaired:true` 和 `clean_name`；成功响应包含 `clean_path`。每次 build/确认 merge 都会自动发布 `clean/<name>/document.md`，`GET /api/artifact?batch_id=...` 返回当前 revision 元数据和 Markdown 内容。

图片/PDF 的 raw OCR 不会进入检索或引用；必须先完成整页 repair 并 build 到 `clean/`。TXT/Markdown 也应先 build 到 clean 后再从网页检索或引用。

跨 batch 的 GUI 操作以 source/clean unit 为最小单位：在“文件浏览”中勾选单元，先预览 `POST /api/merge-units`，确认后再生成 revision。文件列表和预览只负责展示，选择、排序、来源锚点校验仍由 core 完成。

## 界面工作流

页面左侧固定显示 Workspace、Vault 和八个工作区；品牌栏右侧的箭头可以像 VS Code 一样收窄/恢复侧边栏，状态保存在浏览器本地，收窄后仍可通过按钮标题识别入口：

1. **工作台**显示文件数、批次数、可选单元和最近文件；
2. **文件浏览**按全部/来源/生成/清洗/审计筛选，点击即可预览图片、PDF、Markdown、TXT、JSON；勾选 source/clean 后可跨 batch 合并或删除；
3. **导入队列**允许通过 Windows/macOS 文件选择器选择多个文件或整个文件夹，也可以输入服务端可访问路径；浏览器上传项会先进入队列并复制到当前 Vault，路径导入仍可选择保留外部引用。每项都能选择内容类型，队列底部统一设置 OCR、LLM 来源、推理强度、模型、`clean` 发布名称和合并偏好；TXT/MD 可以选择直接发布到 clean（不调用 LLM），PDF/图片则选择后续处理或自动 OCR→修复→发布；
4. **处理批次**按 OCR → 规范化 → LLM 修复 → revision 的步骤运行，每页修复仍由服务端并行且可在任务页取消；页面可编辑当前 Vault 的 repair prompt，保存后后续 repair 自动使用；
5. **后台**集中显示最近事件、命令状态、完成/失败/取消结果、任务进度和 Token/费用；命令区采用终端式输出，进度事件会压缩成易读的最新状态；它位于侧边栏的工具区底部；
6. **来源与 API**集中管理内置/自定义 Provider、Key 状态、价格和默认推理强度，可直接测试连接；
7. **检索**是独立页面，只调用本地索引并展示命中行前后文；
8. **阅读与问答**使用显式引用的 clean 文件，不把 raw OCR 或 generated 历史自动送入模型；“添加引用”打开居中的搜索弹窗，左侧历史栏可以新建、切换和恢复已保存会话。推理强度统一显示 `None/Low/Mid/High`；首次加载优先选择已经配置 Key 的自定义 `GLM-5.2`，没有该项时再选择其它可用来源。

网页没有复制 CLI 的业务逻辑：所有文件写入、来源引用、合并校验、Token 统计和计费都在 Rust core 完成。网页刷新后，Vault 文件、批次状态和已落盘的运行记录仍然是权威状态；内存中的任务列表只用于显示本次服务进程的即时进度。批次页的“规范化”和“LLM 修复”是用户明确触发的重跑操作，会自动带 `refresh:true`，避免旧报告阻塞当前 OCR；多来源批次点击“生成文件”会先转为合并预览，必须确认后才写 revision。

## 阅读与审计

- `GET /api/search?q=...` 是只查询 `clean/` 的本地 SQLite 查询，不调用 LLM。每个 `SearchHit` 除路径、行号和命中行外，还返回前后各两行的 `context`；CLI 和网页优先展示这段可读上下文，而不是只显示元数据。网页可以把命中的 `source_refs` 加入下一轮问答引用。
- `clean_name` 是 build/merge 请求的可选相对名称（例如 `剧本/第一章`）；服务端始终写入 `clean/<名称>/document.md`，拒绝父目录穿越，同名发布会更新 clean 投影并保留 generated 历史。
- 文件浏览的 Markdown 预览提供编辑器和“保存”按钮；保存请求只接受 Vault 内的 `.md/.txt`，拒绝外部 source、raw 和审计目录，并自动刷新索引。原图、OCR 和来源快照始终保留。
- `POST /api/answer` 支持 `query`、`source_refs`、`quotes`、`session_id` 以及独立的 Provider 选择。阅读室把结果渲染成连续的用户/助手气泡，保留当前 `session_id` 以支持追问；每轮回答显示引用数量、Token 和 USD 费用。点击“添加引用”会打开一个居中的选择器：顶部是 clean 全文搜索，下面是可折叠的 clean 文件树，支持文件名/路径本地即时筛选和多选；选中的文件会读入为带文件名的 `quotes`，不会把 raw OCR、图片、PDF、generated 历史或审计 JSON 送给模型。
- 导入、批次处理、来源配置和阅读室都使用“推理强度”，并直接写入请求的 `thinking` 字段。界面统一显示 `None/Low/Mid/High`；GLM-5.3/5.3-Flash 会把 `None` 和 `Mid` 映射到实际允许的最低 `low`，GLM-5.2 等模型可用 `none` 真正发送 `thinking.type=disabled`。Codex/其它 OpenAI-compatible provider 仍使用各自的 reasoning effort 语义。
- `GET /api/usage` 返回 input/cached-input/output/reasoning/total Token、费用和未知费用调用数。
- `GET /api/activity` 返回最近事件、当前任务和用量摘要；事件类型包含 `task_started`、`progress`、`task_completed`、`warning`、`error`、`task_cancelled`。OCR/repair 服务任务在成功、部分失败或失败返回时都会追加终态事件；任务 API 使用 `completed_with_errors` 区分“部分成功”，不会把 `{errors:N,repaired_pages:0}` 显示成完成。终端视图把历史启动/进度行作为事件记录，不再把它们伪装成持续运行中的任务；实际运行状态以任务卡片为准。
- `GET /api/events` 是兼容性的全局 SSE；GUI 的进度以 task API 为准。
