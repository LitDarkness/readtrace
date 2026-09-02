# ADR 0003：原素材快照与整页修复 revision

## 状态

已接受。

## 背景

OCR 和 LLM 都可能大幅改变文字。逐条 patch 既无法表达角色标签、断行和上下文修复，也容易让人工陷入逐条确认。因此必须让模型输出完整页，同时保留可恢复的证据链。

## 决策

1. 导入默认把输入复制到 `sources/<batch_id>`；`--no-copy` 时保存 `external_path`，不复制大文件。
2. `raw/<batch_id>` 只保存 OCR 原文；规范化、repair checkpoint 和 revision 都写入 `generated/<batch_id>`。
3. 每个 LLM 调用返回完整 `repaired_text`，按页立即写入 `repair/<page>.json`。失败页记录错误并可重试。
4. `build` 每次创建 `revisions/000N/document.md`，`current.md` 仅为便利副本；历史 revision 永不覆盖。
5. 正文保留 HTML source anchor，原图和 raw OCR 可随时对照。人工可以直接编辑或复制旧 revision，不存在 confidence/status gate。

## 后果

整页上下文修复能力更强，批量任务可断点续跑，源文件和历史版本可恢复；代价是输出可能比 patch 大，需通过 prompt/profile 约束无依据改写。`CorrectionPatch` 仅保留用于读取旧项目，不参与新流程。
