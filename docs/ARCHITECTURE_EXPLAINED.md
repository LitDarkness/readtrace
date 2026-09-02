# ReadTrace 当前架构说明

本文描述仓库中已经实现的架构，不是未来 GUI 的愿望清单。课程设计 PDF、完整 AI 对话记录和 Excel 开销表会在最终验收时统一导出；代码仓库只保留能复现系统的源码、配置模板和操作文档。

## 1. 目标、边界与核心取舍

ReadTrace 服务于“图片/PDF 中的文字质量差，但原始证据必须可追溯”的阅读场景。它不是通用文档转换器，也不试图替用户决定故事内容。

当前输入边界：

- TXT、Markdown：直接读取；可以通过 direct-clean 立即发布，或者走同样的 normalize/repair/build 流程。
- PDF、PNG、JPG、JPEG、WEBP、BMP：用 Poppler 栅格化，再交给 Tesseract；PDF 按页处理并报告页级进度。
- 文件夹：递归处理上述格式；其它格式进入 skipped_files，不会被静默改名或移动。

最重要的设计取舍是“整页修复”而不是逐条 patch。模型返回一页完整的 repaired_text，用户可以对照原图、raw OCR、normalization、repair checkpoint 和 revision 人工编辑或恢复。这样既允许剧情文本的大范围修复，也不会丢失原始证据。

## 2. 数据流

~~~mermaid
flowchart LR
  A[文件/文件夹] --> B[ImportBatch]
  B --> C{格式}
  C -->|txt/md| D[直接读取]
  C -->|pdf/图片| E[Poppler 栅格化 + Tesseract 页级并行]
  D --> F[OcrPage raw]
  E --> F
  F --> G[确定性规范化]
  G --> H[repair/page-*.json checkpoint]
  H --> I[HTTP GLM/自定义网关 或 Codex CLI]
  I --> J[repair.json + runtime/calls.jsonl]
  J --> K[build]
  K --> L[generated revision Markdown]
  L --> M[clean/<name>/document.md]
  M --> N[SQLite 本地检索]
  N --> O[引用问答 Provider]
  B --> P[MergeUnit 清单]
  M --> P
  P --> Q[merge_plan.json + 人工确认]
  Q --> R[跨 batch revision + clean 投影]
~~~

一次典型生命周期是：

1. 导入创建 ImportBatch，并按默认策略复制原素材或记录 external_path。
2. OCR 为每个 source 生成一个或多个 OcrPage；PDF 的一个文件会展开为 N 页。
3. normalize 只做可解释的空白、行尾和标点邻接整理，不猜词义。
4. repair 对每页调用统一的 LlmProvider；结果以页为单位 checkpoint，失败页单独记录。
5. build 生成不可变 revision；确认 merge 后再把最终 Markdown 发布到 clean。
6. 索引只读 clean；问答只接受用户选定的 clean 证据，视觉页面没有完整 repair 时不会进入引用。

## 3. 模块划分

| 模块 | 责任 |
| --- | --- |
| readtrace-core | 领域结构、路径安全、导入、OCR、normalize、整页 repair、build/merge、Provider、引用、SQLite 索引、运行台账 |
| readtrace-cli | clap 子命令和人类摘要/JSON 输出；适合脚本、批处理和故障恢复 |
| readtrace-server | Axum API、SSE 事件和静态前端；只调用 core，不复制业务逻辑 |
| workspace | 多 Vault 清单、创建/切换和目录约束 |
| text_cleanup | 确定性文本整理；不做上下文猜词 |
| prompt_templates | 内置系统提示词和 Vault 级 prompts/repair.md 覆盖 |
| OCR adapter | Tesseract、Poppler、pdfinfo 路径解析、页级并发与进度 |
| provider adapter | HTTP OpenAI-compatible、Codex CLI、Mock 统一到 LlmProvider |
| runtime ledger | 每次调用的 usage、费用、耗时、状态和去重汇总 |

Web 前端保存的只是当前 Vault、筛选条件、会话和待导入队列；会改变资料的操作都走 Rust API。文件选择接口接收 multi-part 上传，服务器将上传文件写入用户选定 Vault 的导入队列，不把浏览器的临时路径当作来源。

## 4. 关键数据结构

| 结构 | 用途 |
| --- | --- |
| Workspace | 根目录和多个 Vault 的发现/创建关系 |
| Vault | sources、raw、generated、clean、prompts、runtime、events 和索引的根 |
| ImportBatch | batch_id、来源清单、导入模式、复制策略、排序和 skipped_files |
| SourceEntry | source_id、相对路径、原始扩展名、复制状态、external_path |
| OcrPage | page_id、source_ref、页序、raw_text、OCR 元数据 |
| NormalizationRecord | 每个确定性变更的前后位置和规则 |
| RepairPage | page_id、prompt_hash、model、thinking_mode、repaired_text 或错误 |
| Revision | immutable revision id、页面顺序、warning、来源锚点和 Markdown 文件 |
| MergeUnit | 一个 source 文件或 clean Markdown；可跨 batch，是合并的最小单元 |
| MergePlan | 选择的 unit 顺序、来源、警告、冲突和确认状态 |
| SourceExcerpt | 只允许来自 clean 或合规的文本来源，带文件名和段落定位 |
| Session | 会话消息、引用快照、Provider/model/thinking 设置 |
| CallRecord | call_id、phase、provider、model、request_id、Token、价格快照、耗时和状态 |

所有 ID 使用稳定 UUID 或带前缀的字符串。page_id、source_ref、unit_id 在生成 revision 和调用台账时都作为可追溯关联键保存。

## 5. Vault 布局与一致性

~~~text
vault/
├─ sources/<batch_id>/             # 复制的原素材；--no-copy 时不创建副本
├─ raw/<batch_id>/                 # batch.json、OcrPage JSON
├─ generated/<batch_id>/
│  ├─ normalization.json           # 确定性规范化记录
│  ├─ repair/<page>.json           # 每页修复 checkpoint
│  ├─ repair.json                  # repair run 汇总
│  └─ <document>/revisions/000N/   # 不可变 Markdown revision
├─ generated/merges/<merge_id>/   # 跨 batch merge plan/revision
├─ clean/<name>/document.md        # 最终可读投影
├─ prompts/repair.md               # Vault 级提示词覆盖（可选）
├─ prompts/profile.md              # 角色和专名上下文（可选）
├─ runtime/calls.jsonl             # Provider 调用和费用
├─ events/events.jsonl             # 进度、完成、失败
└─ .readtrace/state.db             # 可重建 SQLite 索引
~~~

一致性规则：

- 原素材不被覆盖；build/merge 只写新的 revision 和 clean 投影。
- clean 同名发布只替换投影，不删除 generated 历史。
- visual source 的 raw OCR 和只做 normalize 的文本不是问答证据。
- repair 失败默认阻止 build/merge；allow_unrepaired 只生成带 warning 的临时校对稿。
- 多来源 merge 先写可编辑的 merge_plan.json，确认后才落盘。
- delete-batch/delete-unit 没有 confirm 时只展示计划；确认后清理目标产物，但保留 runtime 和 events 以便审计。

## 6. OCR 与并发

程序先加载项目 .env 中的 READTRACE_TESSERACT_BIN、READTRACE_PDFTOPPM_BIN、READTRACE_PDFINFO_BIN、TESSDATA_PREFIX、READTRACE_OCR_LANGUAGES 和 READTRACE_OCR_DPI，再查找平台默认路径和 PATH。ocr-check 会报告最终解析到的可执行文件、语言包、DPI 和并发度。

PDF 先用 pdfinfo 得到页数，再由 Poppler 逐页栅格化，并以 READTRACE_OCR_CONCURRENCY（默认 4，范围 1–16）有界并行调用 Tesseract。事件会包含 0/N 栅格化、n/N OCR 和最终完成状态。repair 使用 READTRACE_LLM_CONCURRENCY（默认 4，范围 1–64）；结果按原始页序写入，单页 checkpoint 使失败后可以重跑而不用重复成功页。

## 7. Prompt 与 Provider 协议

上层只依赖两个操作：repair_page 和 answer。三种实现使用同一套 trait：

- HTTP：OpenAI-compatible Chat Completions，读取 usage、request id、认证头、max_tokens 字段和推理参数；清华网关、GLM、DeepSeek、Ollama 和其它自定义服务只需改 profile。
- Codex CLI：执行 codex exec --ephemeral --sandbox read-only --json，最终正文来自 output-last-message，Token 来自 turn.completed.usage；不读取或复制桌面登录态。
- Mock：用于无网络流程/界面测试，调用成功且费用明确为 USD 0。

Codex CLI 不是 Codex Desktop GUI。READTRACE_CODEX_BIN 可指定可执行文件；Windows 额外解析 exe、cmd、bat、ps1，macOS 依赖启动服务时的 PATH。Codex 只接受 Codex 模型 preset；GLM 必须走 HTTP，避免“Codex provider + GLM model”的无效组合。

repair prompt 强制要求 JSON 结构中的 repaired_text，只输出完整正文，不输出解释、confidence、patch 或引用框。若响应不是合法合约，或疑似删掉页面段落，当前页失败并保留原 OCR。

## 8. 搜索、引用与问答

搜索是本地 SQLite 查询，不调用 LLM。reindex 只扫描 clean/，因此编辑 clean Markdown/TXT 后可立即重新索引。搜索结果显示文件名、前后文和可复制的 source_ref。

阅读室把检索和问答分成两个页面。添加引用时可以先按内容搜索，再在 clean 文件树中多选文件；会话保存引用文件名和内容快照。answer 的证据集合只接受：

1. 原本就是 TXT/Markdown 的 clean 文本；
2. 图片/PDF 经完整 repair 后发布的 clean Markdown；
3. 用户明确输入的 quote 或 quote-file。

raw、normalization-only 视觉文本和 allow_unrepaired 临时稿都被拒绝为引用来源。这样可以保证问答不绕过修复门槛，也方便从回答回到 clean 文件和原始素材。

## 9. 运行台账与费用

repair、answer、ai-check 每次调用都会追加一条 CallRecord，即使失败也计入调用次数。input_tokens、cached_input_tokens、output_tokens、reasoning_tokens、total_tokens 和 Provider 返回的 request_id 原样记录；Provider 没有 usage 时保持 null，不从文本长度猜 Token。

扫描多个 Vault、tmp 和测试台账时按 call_id 去重；重复记录中 usage 更完整的一条优先。Mock 调用记为 0 美元，只有 Token 或价格确实缺失才计入 unknown_cost_calls。

公式：

~~~text
cached = min(cached_input_tokens, input_tokens)
uncached = input_tokens - cached
USD = uncached/1,000,000 × input_price
    + cached/1,000,000 × cached_input_price
    + output_tokens/1,000,000 × output_price
CNY = USD × usd_to_cny
~~~

每条记录保存调用时的价格、pricing_version 和 USD_TO_CNY（默认 6.8），修改环境变量不会改写旧汇率。当前内置美元价格（每百万 Token）：

| 模型 | input | cached input | output |
| --- | ---: | ---: | ---: |
| GPT-5.6 Luna | 0.20 | 0.02 | 1.20 |
| GPT-5.6 Terra | 2.00 | 0.20 | 12.00 |
| GPT-5.6 Sol | 4.00 | 0.40 | 20.00 |
| GPT-5.5 | 5.00 | 0.50 | 30.00 |
| GPT-5.4 Mini | 0.75 | 0.075 | 4.50 |
| GLM 5.3 Flash | 0.15 | 0.03 | 0.50 |
| GLM 5.2 | 1.40 | 0.26 | 4.40 |

未知自定义模型可以在 .env 提供 READTRACE_INPUT_PRICE、READTRACE_CACHED_INPUT_PRICE、READTRACE_OUTPUT_PRICE。课程要求的开发阶段人时和外部 AI 对话账单不是这份运行时台账的替代品，必须另行登记。

## 10. Web 工作台

Web API 由 readtrace-server 提供，前端静态文件位于 crates/readtrace-server/static。核心页面：

- 工作台：当前 Workspace/Vault、最近文件和任务入口。
- 文件浏览：可折叠 clean/source 树、Markdown/TXT 预览、编辑保存、批量选择、合并和删除。
- 导入队列：文件上传、复制策略、目标 clean 名称、OCR/LLM Provider、推理挡位和并发。
- 处理批次：OCR、normalize、repair、build 的完成/失败态、页级进度和 warning。
- 后台：最近命令、事件流、任务状态、Token 和美元费用。
- 来源与 API：内置/自定义 profile、Key 状态、模型和价格配置。
- 检索：仅 clean 的本地全文查询和可读上下文。
- 阅读与问答：会话侧栏、添加引用、连续追问和 Provider/thinking 选择。

长任务通过 task 状态和 SSE 事件更新；事件终态明确区分 completed、failed、cancelled，前端不会把“已创建”误显示为“已完成”。

## 11. 当前完成度与课程映射

截至 2026-09-02，核心后端和 Web 工作台已达到可演示状态：Workspace/Vault 管理、文件/文件夹导入、真实 OCR、PDF 页级进度、normalize、整页 repair、断点续跑、同 batch 与跨 batch merge、确认式删除、clean 发布、搜索、引用问答、session、Markdown 编辑保存、Provider profile 和运行费用账本均已落地。测试结果为 core 68 项、server 9 项，共 77 项；cargo fmt、cargo clippy 和前端语法检查通过。用户已在 macOS 上完成实际运行验证，仓库也包含跨平台路径与安装说明。

作业要求对应关系：

- 痛点分析：OCR 错字、断行和角色标签需要上下文，逐条 patch 不适合整页剧情。
- 场景定制：整页 repair + 可编辑 prompt；OCR/repair 分阶段 checkpoint；原素材快照、clean 投影和可回溯 revision；HTTP/Codex 双 Provider。
- 架构图、数据流、结构和 crate 选型：本文第 2、3、4 节。
- 源代码、README、配置和演示：仓库根目录及 docs/CLI_*。
- AI 对话历史和开发开销 Excel：最终验收时从保留的本地记录导出，详见 docs/DELIVERABLES_AND_COST_NOTES.md。
- 逐项验收和已知边界：docs/IMPLEMENTATION_AUDIT.md。

课程交付仍有三项“文档整理”工作，不属于核心运行链：

1. 将本文和痛点/场景定制内容排版为设计 PDF，并渲染检查。
2. 导出完整的 AI 原始对话记录。
3. 按阶段填写人时、API 调用、Token、费用、模型和工具的 Excel。

## 12. crate 选型

| 模块 | crate | 选择原因 |
| --- | --- | --- |
| CLI | clap | 子命令、枚举参数和帮助稳定、可脚本化 |
| 异步任务 | tokio、async-trait | 子进程、超时、取消和统一 Provider trait |
| HTTP | reqwest | TLS、JSON、超时和 OpenAI-compatible 请求 |
| 数据 | serde、serde_json、chrono、uuid | 人可读持久化、时间和稳定 ID |
| 搜索 | rusqlite | 无外部服务，索引可重建 |
| Web | axum、async-stream | 薄 API 层和 SSE 进度 |

相关操作手册：

- CLI：[docs/CLI_TUTORIAL.md](CLI_TUTORIAL.md) 和 [docs/CLI_END_TO_END_EXAMPLE.md](CLI_END_TO_END_EXAMPLE.md)
- 完整 PDF/Markdown/图片样例：[docs/COMPLETE_FLOW_TUTORIAL.md](COMPLETE_FLOW_TUTORIAL.md)
- 双平台安装与 GitHub：[docs/GITHUB_AND_DEVICE_SETUP.md](GITHUB_AND_DEVICE_SETUP.md)
- Web API：[docs/WEB_GUI_PROTOCOL.md](WEB_GUI_PROTOCOL.md)
- 验收：[docs/IMPLEMENTATION_AUDIT.md](IMPLEMENTATION_AUDIT.md)
- 课程清单与成本：[docs/DELIVERABLES_AND_COST_NOTES.md](DELIVERABLES_AND_COST_NOTES.md)
