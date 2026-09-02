# 图片端到端样例

`E:\AI_diary\tests\1.png` 和 `2.png` 是真实 OCR 样例；脚本默认跑 `1.png`，需要一次导入全部图片时使用 `-UseFolder`。推荐运行：

```powershell
Set-Location E:\AI_diary\summer_project
.\scripts\run-image-flow.ps1 -RealOcr -LlmProvider codex-cli -Preset codex-luna -Thinking high -TimeoutSeconds 300
```

如果提示 `Codex CLI could not be started`，先在同一个 PowerShell 运行 `Get-Command codex` 和 `where.exe codex`。项目 `.env` 已设置 `READTRACE_CODEX_BIN=codex`；适配器会寻找 Windows 的 `.exe/.cmd/.bat/.ps1`，也可以把 `.env` 中这一项改成 `codex.exe` 的绝对路径。Codex Desktop 的 GUI 登录并不等同于 shell CLI；没有 CLI 时把 `-LlmProvider` 改成 `http` 或 `mock`。

测速时保留同一个 OCR 批次，分别执行 `repair --refresh --speed low`、`mid`、`high`；每次结果的 `duration_ms` 在 `runtime/calls.jsonl`，不会因为 checkpoint 而漏记调用。最近一次对这张图的实测（单页、Codex CLI、`gpt-5.6-luna`）repair 耗时为 Low 21.138 s、Mid 18.097 s、High 52.614 s；完整 process 墙钟分别为 23.908 s、20.738 s、55.340 s。这是单样本结果，应以自己的重复运行数据为准。

没有 Tesseract 时去掉 `-RealOcr`，脚本使用 Mock OCR 验证相同的 import、repair、revision、索引和问答流程。`-ReviewOnly` 会停在 build 之前；`-NoCopy` 只保存外部素材路径。

产物结构：

- `sources/`：默认复制的原始 PNG；
- `raw/`：页级 OCR JSON；
- `generated/`：`normalization.json`、每页 `repair` checkpoint、`repair.json` 和 revisioned Markdown；
- `runtime/calls.jsonl`：Provider 调用次数、input/cached-input/output/reasoning/total Token、调用时单价快照、费用和耗时；单独用 `ai-check --ledger tmp\check.jsonl` 的测试账本也会被 `usage --scan-root` 纳入并按 `call_id` 去重；
- `events/`、`sessions/`：流程事件和会话。

`repair` 默认输出摘要；需要完整的 `result_files`、短预览和 `repaired_text` 时，使用 `repair ... --format json` 或直接打开 build 后的 `generated/<batch_id>/<document_id>/current.md`。

输入分流：`.txt`/`.md` 直接读取，`.pdf` 和图片走 OCR，其它格式跳过；文件夹递归解析并记录 skipped。规范化 JSON 仅供人工审计/调整，视觉来源仍必须完成整页 repair 才能 build、merge 或引用；repair 默认复用未变化页，`--refresh` 才重跑。

多页 repair 默认最多并行 4 个请求，可通过 `.env` 的 `READTRACE_LLM_CONCURRENCY=1..64` 调整；页序和 checkpoint 不受并行完成顺序影响。已知 OpenAI/Codex 模型、GLM‑5.2 和 GLM‑5.3 Flash 按记录的 input/cached/output 价格计算；学校网关采用独立价格时需要手工覆盖。

测试图片的 Codex Luna High 运行样例保存在 `tmp/luna-high-full-20260829-225721/`；本次 Low/Mid/High 独立基准保存在 `tmp/png-speed-20260829-234232/`（运行产物，不是固定数据集）。旧运行产物中的 Codex usage 仍是 unknown；修复适配器后重新运行会记录 JSONL usage 和 request id。
