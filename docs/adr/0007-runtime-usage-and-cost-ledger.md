# ADR 0007：运行时调用、Token 与费用台账

## 状态

已接受。

## 决策

每次 `repair_page`/`answer`/`ai-check` 调用完成或失败后立即追加一条 `runtime/calls.jsonl`。记录调用次数、Provider、模型、阶段、耗时、input/cached-input/output/reasoning/total usage、三档单价快照、价格版本、USD/CNY 和错误。Token 字段可为 null；费用只有 input/output Token 和对应单价都已知时计算，cached input 从 input 中拆出后按 cached 单价计费（没有单独 cached 单价时回退普通 input 单价）。CNY 使用配置汇率 `READTRACE_USD_TO_CNY`，默认 6.8。

CLI `usage` 和 Web `/api/usage` 提供批次/全 Vault 汇总，并显示 cached input Token。扫描模式会读取根目录下所有 `.jsonl` 并按 `call_id` 合并，所以 `--ledger tmp\...jsonl` 的测试调用也会进入汇总；Codex CLI 的 usage 不可见时保留 unknown。开发阶段 AI 对话开销不从该文件推断，按课程 Excel 独立人工登记。

repair 的页面调用默认以 4 为并发上限，由 `READTRACE_LLM_CONCURRENCY` 调整到 1..64；结果和台账写入保持顺序化。

## 原因

批量 OCR 修复可能产生大量调用，必须能回答“调用了多少次、用了多少 Token、花费多少”；同时不能把缺失 usage 或免费额度伪装成精确 0。
