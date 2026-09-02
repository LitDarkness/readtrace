# 核心逻辑与设计（冻结版）

## 1. 核心原则

ReadTrace 把“证据保存”和“文本交付”分开：source snapshot/raw OCR 永远不改，LLM 可以对整页做大范围修复，build 每次产生一个新 Markdown revision。人工审查通过对照和直接编辑完成，不引入 confidence 或逐条批准状态。

## 2. 流程

```text
导入（可复制/外部引用）
  → OCR（txt/md 直接读取，pdf/图片 Tesseract）
  → 确定性空白规范化
  → LLM repair_page（完整 repaired_text）
  → 按页 checkpoint + runtime call ledger
  → build revision（source anchors）
  → SQLite 搜索 / 可选引用问答
```

OCR 与 repair 是两个可独立重跑的阶段，避免长请求超时。repair 只重试缺失或过期页，失败页不阻塞其它页；多个页面默认最多并行 4 个请求，可用 `READTRACE_LLM_CONCURRENCY=1..64` 调整，checkpoint 和最终页序不变。多来源导入默认只完成各页 OCR/repair，并生成可编辑的 `merge_plan.json`；用户确认后才合并。跨 batch 操作以单个 `source` 文件或已整理好的 `clean` Markdown 为最小 unit，而不是以 batch 为边界。图片/PDF 没有完整 repair 时，`build`、旧 `apply` 和跨 batch merge 都拒绝落地最终文本；只有 TXT/Markdown 可以直接使用规范化内容。

## 3. 目录

```text
sources/       原素材快照（或 batch 中的 external_path）
raw/           ImportBatch 与 OcrPage
generated/     normalization、repair checkpoint、revision
generated/merges/ 跨 batch unit merge plan 与 revision
prompts/       repair.md 与 profile.md
runtime/       calls.jsonl
events/        JSONL 进度
.readtrace/    可重建 SQLite（state.db）
```

## 4. Provider 与提示词

`LlmProvider::repair_page` 是核心接口。HTTP Adapter 使用 OpenAI-compatible JSON，Codex Adapter 包装本机 `codex exec`，GLM/清华网关仅需配置 Base URL/Key/模型。提示词要求严格 JSON `{"repaired_text":"完整文本"}`，允许 profile 提供角色别名，但禁止分析、patch、confidence 和无依据续写。

问答的 `SourceExcerpt` 有证据门槛：TXT/Markdown 和 clean Markdown 可直接引用；图片/PDF 只引用完整 `RepairedPage.repaired_text`，raw OCR 和仅规范化文本不会送入 Provider。显式 `source_ref`、`quote` 或 session 会关闭无关的全库搜索混入。

## 5. 可恢复性与成本

每页结果含 ocr_text、normalized_text、repaired_text、source_ref、prompt hash、model 和 call_id。revision 目录按 `0001` 递增。每次 LLM 调用写入 `runtime/calls.jsonl`，保存 input/output/cached/reasoning/total Token 和调用时的价格快照；费用按缓存子集拆分后计算 USD，再按 `READTRACE_USD_TO_CNY` 换算 CNY，价格或 usage 不完整时保持 null。开发阶段 AI 开销 Excel 独立人工填写。

## 6. 验收命令

```powershell
cargo test --workspace
cargo run -p readtrace-cli -- process .\vault .\input --ocr mock --llm mock
cargo run -p readtrace-cli -- usage .\vault
cargo run -p readtrace-cli -- ocr-check
cargo run -p readtrace-cli -- sources .\vault
cargo run -p readtrace-cli -- merge-units .\vault <unit-id> <unit-id>
```

完整命令（包括确认式 `delete-batch`/`delete-unit`）和课程交付字段见 `docs/CLI_TUTORIAL.md`、`docs/CLI_END_TO_END_EXAMPLE.md` 与 `docs/DELIVERABLES_AND_COST_NOTES.md`。

## 7. Web/GUI 协议

Web 只做适配，不复制核心逻辑。以 Workspace 启动服务时，`GET /api/vaults` 返回 Vault 列表，`POST /api/vaults/select` 切换当前 Vault；单 Vault 启动仍兼容。导入和阶段任务返回 `task_id`，前端轮询 `GET /api/tasks/{task_id}` 获取 `running/completed/failed/cancelled`、进度、结果和错误，取消使用 `POST /api/tasks/{task_id}/cancel`。合并计划通过 `GET/POST /api/merge-plan` 读写，提交前由 core 校验页集合和不可变 `source_ref`；`GET /api/artifact?batch_id=...` 提供当前 revision 预览。第一版无构建 GUI 位于 `crates/readtrace-server/static/`。
