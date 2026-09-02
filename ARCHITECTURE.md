# 架构入口

当前架构唯一权威文档是 [`docs/ARCHITECTURE_EXPLAINED.md`](docs/ARCHITECTURE_EXPLAINED.md)：

- `import → ocr → normalize → repair → build` 四阶段；
- LLM 按页返回完整 `repaired_text`，不使用 confidence 或逐条审批；
- source snapshot/`--no-copy`、raw OCR、prompt/profile、repair checkpoint 和 revision 的边界；
- 单个 source/clean 文件作为最小 unit 的同 batch 与跨 batch 合并，以及确认前可编辑的 merge plan；
- 图片/PDF 必须经过完整 page repair 才能 build、merge 或作为问答引用；TXT/Markdown 可直接引用；
- `.env` 自动发现 Tesseract/Poppler，`ls`/`sources` 查看 Workspace、Vault 和合并单元；
- `delete-batch`/`delete-unit` 先生成删除计划，`--confirm` 后才删除派生文件，运行台账和事件保留；
- HTTP GLM/自定义 OpenAI-compatible Provider 与本机 Codex CLI 的统一协议；
- `runtime/calls.jsonl` 的调用、input/cached/output Token、模型单价快照和 USD/CNY 规则；
- repair 默认 4 路有界并行（`READTRACE_LLM_CONCURRENCY=1..64`），结果仍按原始页序落盘。
- Web 以 Workspace 启动时提供 Vault 列表/切换、task 状态与取消、merge-plan 人工编辑和 revision 预览；`crates/readtrace-server/static/` 是无构建步骤的第一版 GUI。

第一次启动请看 [`docs/QUICK_START.md`](docs/QUICK_START.md)；命令操作见 [`docs/CLI_TUTORIAL.md`](docs/CLI_TUTORIAL.md)，从零示例见 [`docs/CLI_END_TO_END_EXAMPLE.md`](docs/CLI_END_TO_END_EXAMPLE.md)，Web/GUI 端点见 [`docs/WEB_GUI_PROTOCOL.md`](docs/WEB_GUI_PROTOCOL.md)，课程交付字段见 [`docs/DELIVERABLES_AND_COST_NOTES.md`](docs/DELIVERABLES_AND_COST_NOTES.md)。CLI 默认输出人类摘要，脚本可用 `--format json` 保留完整结构。
