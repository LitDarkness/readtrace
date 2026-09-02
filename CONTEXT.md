# ReadTrace 项目语境

## Product boundary

ReadTrace 是按 Workspace 管理多个独立 Vault 的 OCR 整理工具。支持 `.txt`/`.md` 直接读取，图片/PDF 走 Tesseract/Poppler，文件夹递归解析；其它格式明确跳过并记录。搜索是本地 SQLite 查询，不把 LLM 接入搜索。

## Domain terms

**Vault**：一个资料集合及其 raw、generated、source 和 runtime 台账目录。

**Profile**：批次携带的字符串上下文标识。Profile 不产生业务分支；额外角色别名、专名和规则放在 Vault 的 `prompts/profile.md`。

**Source snapshot**：默认复制到 `sources/<batch_id>` 的原素材；`--no-copy` 时改为 `SourceFile.external_path` 外部引用。

**OcrPage**：不可覆盖的原始 OCR 页，含 `source_ref`、块和 raw_text。

**PreparedPage**：确定性空白/行尾清洗后的页，带 `NormalizationChange` 审计。

**Repair result**：Provider 针对一页返回的完整 `repaired_text`。它可以大幅重写 OCR 错误，但不能覆盖 raw。

**Repair run**：一个批次的成功页、失败页、Provider、模型和 prompt hash；每页 JSON 是断点 checkpoint。

**Revision**：`build` 从 repair 结果生成的不可变 Markdown 版本。current.md 只是方便人工编辑的副本。

视觉页也可以通过 `allow_unrepaired` 显式使用规范化 OCR 生成 revision；这是带警告的应急路径，默认关闭，且不会让这类文本进入引用上下文。

**Merge unit**：跨批次选择的最小资料单元；可以是一份导入来源文件，也可以是 `clean/` 或 `generated/.../current.md` 中的一份已经整理好的 Markdown 文件。它不等同于 batch，多个 unit 可以来自不同 batch。

**Cross-batch merge plan**：跨 batch 合并前的可编辑清单，记录 unit、页序和来源锚点。确认前不生成合并 revision，确认后按清单生成。

**Runtime call record**：每次 LLM repair/answer/ai-check 调用的次数、input/cached/output/reasoning/total Token、价格快照、费用、耗时和错误。未知值保持 null。

**Citation text**：问答允许使用的最终证据。图片/PDF 只引用完整 repair 结果；原始 txt/md 和 clean Markdown 可直接引用，因为它们本身就是可读文本。raw OCR 和仅规范化的视觉 OCR 不进入引用上下文。

**Speed profile**：用户对 reasoning effort 的统一挡位。Low 追求速度，Mid（medium）平衡，High 追求复杂上下文质量；它不改变模型名称。

**Deletion plan**：`delete-batch` 或 `delete-unit` 生成的破坏性操作预览。只有显式 `--confirm` 才执行；运行台账和事件流保留，索引随后重建。

## Human edit point

人工不需要逐条确认模型意见。可以编辑 `prompts/repair.md`、`prompts/profile.md`、`normalization.json` 或 `current.md`，也可以复制旧 revision 恢复。下一次 repair 默认复用未变化 checkpoint；明确 `--refresh` 才重跑。

## Provider contract

`LlmProvider::repair_page(page, mode, prompt) -> RepairResponse` 是整页修复协议；`answer` 是带本地检索证据的问答协议。HTTP OpenAI-compatible 和本机 Codex CLI 实现共用这两个接口，Mock 用于离线测试。

Codex CLI 适配器的入口由 `READTRACE_CODEX_BIN` 决定，默认查找 `codex` 及 Windows 常见 shim；Codex Desktop GUI 登录态本身不是 shell 命令，也不是本项目可直接访问的 HTTP API。无法发现 CLI 时应切换 HTTP/Mock，而不是把模型名当作可执行文件。

## Safety invariants

- raw OCR 与 source snapshot 不被模型写入。
- repair 输出必须是 JSON 的完整 `repaired_text`；解析失败只影响当前页。
- build 总是生成新 revision，不静默覆盖历史版本。
- 搜索不调用 LLM。
- 显式 source_ref、quote 或 session 会限定证据边界，不混入无关的全库搜索结果。
- 多来源合并必须先确认 merge plan；人工调整页序会反映到最终 revision。
- LLM 修复任务的 `completed_with_errors` 与 `failed` 不得混同：只有至少一页成功时才是前者；零页成功必须显示失败。未修复 OCR 合并必须显式选择并在结果中留下 warning。
- 删除必须先查看 DeletionPlan；复制的 source、raw、repair 和 revision 可以删除，但 `--no-copy` 的外部文件、runtime 台账和 events 不会被误删。
- API Key 不写入 session、事件或 runtime ledger。
- Token/费用未知时不伪造精确数字。
