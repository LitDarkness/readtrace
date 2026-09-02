# ReadTrace 实现审计（2026-09-02）

## 结论

CLI 和 `readtrace-core` 的核心链路已经可以完整演示：导入、按格式路由、OCR、规范化、整页修复、revision、同 batch/跨 batch 合并、搜索、带来源问答、session、删除和运行时台账都已落地。模型调用与 PDF 页级 OCR 现在默认有界并行，账本保存 input/cached-input/output/reasoning/total Token，并按调用时的模型价格快照计算费用。

因此，数据模型、CLI 和 Web/GUI 使用的是同一套核心协议。Web 已经提供工作台式界面：Workspace/Vault、文件浏览与预览、导入队列、批次处理、跨 batch 单元选择、后台命令记录和阅读问答均在同一个页面完成，GUI 只负责调用这些接口，不复制业务规则。

## 已验收项目

| 优先级 | 项目 | 结果 |
| --- | --- | --- |
| P0 | TXT/Markdown 直接读取；PDF/PNG/JPG/WEBP/BMP 走 Tesseract/Poppler；其它格式记录 `skipped_files` | 通过 |
| P0 | 原素材复制或 `--no-copy` 外部引用；raw OCR、规范化、repair checkpoint、revision 分层保存 | 通过 |
| P0 | LLM 按页返回完整 `repaired_text`；prompt 可用 `--prompt-file` 或 Vault 文件编辑 | 通过 |
| P0 | HTTP OpenAI-compatible、学校/GLM 自定义 Base URL、Mock、Codex CLI 共用同一 Provider 协议 | 通过 |
| P0 | 同 batch 合并和跨 batch source/clean unit 合并；计划可编辑，确认后构建 | 通过 |
| P0 | 图片/PDF 未完成整页 repair 时默认不能 build、merge 或作为问答证据；`--allow-unrepaired` 仅生成带 warning 的应急稿，raw OCR 不进入引用 | 通过 |
| P0 | SQLite 普通文本搜索；显式 source/quote/session 问答不混入无关搜索结果 | 通过 |
| P0 | batch/unit 删除先预览，`--confirm` 才执行；运行台账和事件保留 | 通过 |
| P0 | 每页 checkpoint 可续跑；多页 repair 和 PDF OCR 默认最多 4 路并行，结果仍按原始顺序落盘 | 通过 |
| P0 | runtime ledger 按 `call_id` 合并去重，包含 `tmp` 下自定义 `.jsonl` 测试账本 | 通过 |
| P0 | OpenAI、GLM‑5.2 和 GLM‑5.3 Flash 自动套用已记录的 input/cached/output 价格；其它自定义模型保持显式配置 | 通过 |

## 计费与并发审计

每次 `repair_page`、`reading_answer` 和 `ai_check` 都会追加一条 `CallRecord`。保存字段包括：

```text
input_tokens, cached_input_tokens, output_tokens,
reasoning_tokens, total_tokens,
input_price_per_million, cached_input_price_per_million,
output_price_per_million, pricing_version,
cost_usd, cost_cny, usd_to_cny
```

计算规则为：

```text
cached = min(cached_input_tokens, input_tokens)
uncached = input_tokens - cached
USD = uncached/1,000,000×input_price
    + cached/1,000,000×cached_input_price
    + output/1,000,000×output_price
CNY = USD×usd_to_cny
```

如果 input/output Token 或对应单价不完整，费用保持 `null`，并计入 `unknown_cost_calls`；不会以字符数估算，也不会把缺失价格当成 0。读取旧账本时，只要真实记录已经包含 input/output，就会按 model 的官方单价回填并写回；Mock 调用则明确记为非计费 `$0`。重复账本合并时，含 cached usage、request id 或费用的完整记录优先保留。

当前内置模型价格来自模型价格表：[GPT-5.6 Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)、[GPT-5.6 Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra)、[GPT-5.6 Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol)、[GPT-5.4](https://developers.openai.com/api/docs/models/gpt-5.4)、[GPT-5.5](https://developers.openai.com/api/docs/models/gpt-5.5) 和 [Z.ai 官方价格页](https://docs.z.ai/guides/overview/pricing)。目前记录的价格（USD/百万 Token）为：GPT-5.6 Luna `$0.20/$0.02/$1.20`，GLM‑5.3 Flash `$0.15/$0.03/$0.50`，GLM‑5.2 `$1.40/$0.26/$4.40`（均为 input/cached input/output）。学校平台有独立结算价时仍需在 `.env` 手工填入实际价格。

`READTRACE_LLM_CONCURRENCY` 取值范围是 `1..64`，默认 `4`；`READTRACE_OCR_CONCURRENCY` 取值范围是 `1..16`，默认 `4`。`READTRACE_OCR_DPI` 默认 200，可在识别小字不敏感时降到 150。并行只作用于 Provider 请求或独立 Tesseract 子进程；文件写入、账本追加和最终页序仍在一个任务中顺序完成，因此不会产生损坏的 JSONL 或乱序 revision。PDF 先用 `pdfinfo` 获取页数，任务会产生 `0/N` 栅格化和 `n/N` 页级 OCR 事件。

模型选择也有隔离规则：命名 preset 的价格优先于环境变量；临时 `--model` 与 `.env` 模型不同时，不会继承旧模型的单价，未知模型费用保持 `null`。

## 本轮可复现检查

以下检查已在项目根目录执行：

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --quiet -p readtrace-cli -- ocr-check
cargo run --quiet -p readtrace-cli -- provider-check --preset codex-luna --speed high
```

结果：格式检查、Clippy 通过；Workspace 测试共 77 项（core 68 项、server 9 项）全部通过；`ocr-check` 报告 Tesseract、Poppler/pdfinfo 和 OCR 并行度可用；Codex preset 配置显示 `gpt-5.6-luna`、`high`、官方价格和并发上限 4。Web 的推理强度统一显示 `None/Low/Mid/High`，首次加载优先选择已配置 Key 的自定义 `GLM-5.2`；用户仍可切换 `Codex Luna`、其它内置来源或自定义来源。显式把 `glm-*` 交给 Codex CLI 会被拒绝，避免 provider/model 错配。HTTP payload 回归测试覆盖 GLM-5.3 的 `reasoning_effort` 映射、有效强度归一化和 GLM-5.2 的 disabled thinking；新增回归测试覆盖 GLM‑5.2 官方价格、跨平台 Windows 盘符上传路径拒绝、旧网页 profile id、多页 OCR 页级进度、clean 发布和 clean-only 搜索。

本轮在受限 Codex 宿主中用同一条 `ai-check --provider codex-cli` 做了多组最小复现：移除会话环境变量、显式指定 `.cmd`/`.exe`、以及直接运行 `codex exec`，均在本机 app-server 初始化处收到 `拒绝访问 (os error 5)`；开启可写临时 `CODEX_HOME` 后又得到 `readonly database` 或 `UnknownIssuer`。这说明失败发生在受限宿主的文件权限/证书边界，而不是 ReadTrace 的模型、Token 或计费解析错误，也不是普通网络抖动。相同的 ReadTrace 命令在普通外部 PowerShell 环境成功返回 `gpt-5.6-luna` 的 `input_tokens=13822`、`output_tokens=5`、`total_tokens=13827` 和 request id；因此使用 Codex 时应从普通 PowerShell/Windows Terminal 启动，或在当前宿主改用 HTTP/GLM/Mock。适配器现在会保留原始尾部并给出上述行动建议，且不会复制 `auth.json` 或绕过 Codex 安全边界。学校 HTTP/GLM 探针验证了两种模型：GLM-5.3-Flash `none→reasoning_effort=low` 返回 200（input 28、cached 0、output 72、reasoning 69、total 100），GLM-5.2 `thinking.type=disabled` 返回 200（input 22、cached 0、output 1、reasoning 0、total 23）；两者均按模型价格记录。

当前 `first_run` Vault 的 ledger（含本轮 Chrome 流程）为 43 次调用、123,631 total Token（input 105,883、output 17,748、cached input 27,136），已计费 `$0.02511802`；已知 `gpt-5.6-luna` 按官方价计算，Mock 为 `$0`，15 条没有 provider usage 的 Codex 失败调用仍计入 `unknown_cost_calls`。这个运行时汇总不替代课程要求的开发阶段 Excel。

为避免把探针和临时测试漏掉，最新 `usage --scan-root .` 已扫描项目内全部 JSONL 并按 `call_id` 去重：81 次调用，input 260,990、cached input 62,976、output 20,230、total 281,220 Token；已知调用费用合计 `$0.05201652`（约 `¥0.356417936`），29 次失败，37 条因宿主或 Provider 未返回 usage/价格而保持 unknown。该快照保存在本机 `deliverables/runtime-usage-all.json`（`deliverables/` 被 `.gitignore` 刻意排除，需按课程要求单独提交），删除的临时 Vault 不再参与统计。此次新增的来源连接测试和 Mock 对话也已进入台账；Mock 明确按 `$0` 处理。

## GUI 协议与当前实现（P1）

- `/api/ocr`、`/api/repair` 返回 `task_id`；`GET /api/tasks`、`GET /api/tasks/{task_id}` 查询状态、进度、结果和错误；`POST /api/tasks/{task_id}/cancel` 取消任务。repair 的取消会传入核心并保留已完成 checkpoint。
- `GET/POST /api/merge-plan` 负责读取和提交人工排序，core 会拒绝伪造 page、source_ref 或新增页面。
- 以 Workspace 路径启动 Web 后，`GET /api/vaults` 列表并可用 `POST /api/vaults/select` 切换；`POST /api/workspace/init` 可创建 Workspace 与首个 Vault，`POST /api/vaults/create` 可继续添加 Vault；单 Vault 启动仍保持兼容。
- `GET /api/files` 返回当前 Vault 的安全文件清单，`GET /api/file` 和 `/api/file/raw` 分别用于文本和二进制预览；`.readtrace` 与隐藏文件不会暴露到浏览器。
- `GET /api/files?view=essential|all` 支持内容文件与完整审计文件两种视图；GUI 在前端将路径组织成可折叠的文件树。
- `GET /api/activity` 合并持久化事件、当前任务和用量；后台页以终端式命令行显示开始、进度、完成、失败和取消，并以卡片和明细同时显示 Token、USD/CNY 费用及未返回 usage 的调用数。
- `POST /api/delete-batch` 与 `POST /api/delete-unit` 保持“先计划、后确认”的删除语义；文件浏览中的删除按钮只针对已选的 source/clean unit。
- `GET /api/artifact?batch_id=...` 返回当前 revision 元数据和 Markdown 内容。
- `GET /api/providers`、`POST /api/providers`、`POST /api/providers/check` 管理内置/自定义来源，Key 只写入本机配置且连接测试进入 ledger；`GET /api/sessions` 和 `GET /api/sessions/{id}` 支持阅读室历史侧栏恢复会话。
- `POST /api/answer` 的问答引用分为 Vault `source_refs` 与用户选择的可读文件 `quotes`；GUI 的引用弹窗将 clean 全文搜索置于顶部，下面是可折叠的 clean 文件树，支持文件名/路径本地即时筛选和多选；raw OCR、图片、PDF、generated 历史和审计文件不会作为 quote 发送。
- `POST /api/build`、确认 `POST /api/merge` 和确认 `POST /api/merge-units` 会自动发布 `clean/<name>/document.md`；`clean_name` 可自定义名称，响应返回 `clean_path`。generated revision 继续保留，便于人工对照和回溯。
- `crates/readtrace-server/static/` 提供无构建步骤的工作台 GUI：左侧 Workspace/Vault 导航，工作台统计，可折叠文件树与“内容文件/显示全部”切换，Markdown/TXT 编辑保存，文件预览/删除，导入队列，OCR/repair/merge 分步处理，后台命令与事件、任务取消，跨 batch 合并选择，来源与 API 管理，独立检索页、搜索上下文、聊天页的弹出式引用选择器（搜索结果上方、可读文件多选下方）、会话侧栏、大对话区、revision 和费用查看均已接入。模型选择显示来源名称、`内置/自定义` 和 Key 状态，推理强度统一为 `None/Low/Mid/High`。

剩余事项主要是增强：统一 HTTP 错误状态码、为跨 batch merge 增加可视化排序编辑、增加预算停止条件和桌面打包；这些不阻塞当前 GUI 使用。

## GUI 可复用的稳定边界

GUI 不应重新实现 OCR、修复、计费或合并规则，只调用以下协议：

- 资源：`import-file`/`import-folder`、`ls`、`sources`、`vault-list`；
- 阶段：`ocr`、`normalize`、`repair`、`build`、`merge`、`merge-units`；
- 阅读：`search`、`answer`、`session-*`；
- 审计：`usage`、`ocr-check`、`provider-check`、`ai-check`；
- 安全：`delete-batch`、`delete-unit`、`--confirm`；
- 事件：`tasks` 轮询和现有 `/api/events` SSE；任务状态以 task API 为准。

工作台 GUI 已覆盖 Workspace/Vault 创建与切换、导入队列、文件浏览预览、阶段进度、单元合并预览、revision 查看和带来源问答；所有业务结果仍以 Vault 内 Markdown/JSON 为准。
