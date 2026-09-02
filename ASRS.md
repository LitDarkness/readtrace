# ASRS：当前需求与验收标准

本文是课程项目的验收摘要，详细实现以 [`docs/ARCHITECTURE_EXPLAINED.md`](docs/ARCHITECTURE_EXPLAINED.md) 为准。

## 用户价值

图片/PDF OCR 的错字、断行和角色标签会破坏阅读；现有 OCR 工具只给原始文字，通用 LLM 又可能无来源地改写。ReadTrace 保留原素材和 raw OCR，同时让 LLM 在整页上下文中修复，再生成可编辑 revision。

## 功能验收

- TXT/Markdown 直接读取；图片/PDF 走真实 Tesseract/Poppler；文件夹递归解析支持格式并列出 skipped。
- 默认复制 source；`--no-copy` 保存 external_path。
- `import → ocr → normalize → repair → build` 可分别执行，repair 按页 checkpoint、错误可重试。
- LLM 返回完整 `repaired_text`，无 confidence、review gate 或逐条 patch 要求。
- `prompts/repair.md` 与 `prompts/profile.md` 可由人直接编辑。
- 每次 build 生成新 revision，source anchor、raw、原图和旧 revision 可恢复。
- 同 batch 与跨 batch 合并均先生成可编辑计划；跨 batch 的最小单位是单个 source 文件或 clean Markdown，人工只能调整页序，确认后才落地 revision。
- 图片/PDF 没有完整 page repair 时，不能 build、merge 或作为问答证据；TXT/Markdown 可直接作为可读文本。
- HTTP OpenAI-compatible、GLM/学校网关、Codex CLI、Mock 共用 Provider 协议。
- 搜索为本地 SQLite，不调用 LLM；问答保留来源引用；TXT/Markdown 和 clean Markdown 可直接引用，视觉 OCR 必须先有完整 repair。
- `runtime/calls.jsonl` 记录每次调用、input/cached/output/reasoning/total Token、模型单价快照、费用和耗时；unknown 保持 null。
- `delete-batch` 和 `delete-unit` 默认只预览，确认后删除对应素材与派生结果，保留 runtime/events 审计并重建索引。

## 交付验收

设计 PDF、README、CLI 教程、源代码、完整 AI 对话记录和开发开销 Excel 依照 [`docs/DELIVERABLES_AND_COST_NOTES.md`](docs/DELIVERABLES_AND_COST_NOTES.md) 整理。源代码只需 Git 管理，不在本项目自动提交清华或网络学堂。
