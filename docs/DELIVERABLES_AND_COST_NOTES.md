# 课程交付与 AI 开发开销记录

## 一、交付清单

1. 完整设计文档 PDF：痛点分析、至少两项场景定制、系统架构图/数据流/关键结构、crate 技术选型。
2. 源代码：提交清华 Git，包含 README、编译/配置（Endpoint/Key）/运行说明和演示用例。
3. AI 对话历史：开发期间的完整原始 Markdown/JSON/PDF 记录。
4. AI 开发开销明细表 Excel：按阶段记录人时、API 调用次数、Token、费用、模型和工具。

除源代码 Git 提交外，其余交付物按网络学堂作业要求整理；本项目不负责自动上传或 push。

开发期间的原始记录属于需要保留的审计材料。`runtime/`、`tmp/`、`workspace/` 和 `deliverables/` 虽然被 `.gitignore` 排除，清理源码时不要删除；最终提交作业前，再从这些目录导出 AI 对话记录、运行时汇总和 Excel，不要把本机 Key 或整个 Vault 直接加入 Git。

## 二、两种成本必须分开

### 1. 项目运行时成本（自动记录）

`repair` 的每个页面调用、`answer` 的每次问答都会追加到 Vault 的 `runtime/calls.jsonl`；`ai-check` 等没有 Vault 的探针默认追加到当前目录的 `runtime/calls.jsonl`，也可用 `--ledger` 指定路径。每条记录包含：

`call_id`、`batch_id`、`phase`、`provider`、`endpoint_host`、`model`、`thinking_mode`、`request_id`、输入/输出/缓存/推理/总 Token、`cost_usd`、`cost_cny`、`pricing_version`、`usage_source`、`estimated`、耗时、成功状态和错误类型。

规则：

- 调用次数无论成功失败都计数。
- Provider 返回的 usage 原样解析；字段缺失保持 `null`，不写成 0。读取旧账本时，若已有 input/output 和可识别模型，会按官方价格补齐历史费用；Mock 调用明确记为非计费 `$0`。
- 只有 input/output Token 和对应单价都已知时才计算费用；cached input 是 input 的子集，若未提供单独 cached 单价则按普通 input 单价计费；未知费用计入 `unknown_cost_calls`。
- USD 价格单位是每百万 Token；CNY 使用运行时 `READTRACE_USD_TO_CNY`（默认 6.8）换算，并把汇率写入记录。
- 修改汇率不会回写历史调用；已完成的历史记录保留当时写入的 `usd_to_cny`，新调用从当前 `.env` 读取 6.8。旧记录的缺失费用只在读取时按其模型和 Token 回填，不从文本反推。
- 当前 Codex 适配器通过 `codex exec --json` 读取 `turn.completed.usage`，并记录 `thread.started.thread_id`。GLM/自定义 HTTP 若提供标准 usage 则精确记录；若某次真实调用没有 usage 事件，才回退为 unknown。

公式：

```text
cached = min(cached_input_tokens, input_tokens)
uncached = input_tokens - cached
USD = uncached / 1,000,000 × input_price
    + cached / 1,000,000 × cached_input_price
    + output_tokens / 1,000,000 × output_price
CNY = USD × USD_TO_CNY
```

已知 OpenAI 模型、GLM‑5.2 和 GLM‑5.3 Flash 会按价格表自动填入三档单价，并把 `pricing_version` 与单价快照写入每条记录。Codex Luna（`gpt-5.6-luna`）当前为 `$0.20/$0.02/$1.20`，GLM‑5.3 Flash 为 `$0.15/$0.03/$0.50`，GLM‑5.2 根据 [Z.ai 官方价格页](https://docs.z.ai/guides/overview/pricing)在 2026-09-02 的标价为 `$1.40/$0.26/$4.40`（均为 input/cached input/output，每百万 Token）。学校网关若有不同结算价，应在 `.env` 设置实际价格覆盖：

```dotenv
READTRACE_INPUT_PRICE=0
READTRACE_CACHED_INPUT_PRICE=0
READTRACE_OUTPUT_PRICE=0
READTRACE_PRICING_VERSION=school-unknown
```

repair 默认最多并行 4 个页面；`READTRACE_LLM_CONCURRENCY=1..64` 可调整上限。并行不会改变调用记录去重规则，也不会改变最终页序。

查询：

```powershell
cargo run -p readtrace-cli -- usage .\vault
cargo run -p readtrace-cli -- usage .\vault --batch-id <batch_id>
```

Web 对应 `GET /api/usage?batch_id=<batch_id>`。这份机器台账用于批量调用的预算和复盘，不等同于开发者的订阅账单。

### 2. 开发阶段 AI 开销（按作业 Excel 人工维护）

本地 Codex/ChatGPT 对话、设计讨论和代码生成的费用可能没有逐次 API 账单，因此不从运行时 ledger 猜测。最终 Excel 建议字段固定为：

| 阶段 | 人时（小时） | API 调用次数 | 输入 Token | Cached 输入 Token | 输出 Token | 总 Token | 总花费（USD） | 总花费（CNY） | 模型名称 | AI 开发工具 | 备注 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| 需求分析 |  |  |  |  |  |  |  |  |  |  |
| 架构设计 |  |  |  |  |  |  |  |  |  |  |
| Rust 核心逻辑 |  |  |  |  |  |  |  |  |  |  |
| OCR/Provider |  |  |  |  |  |  |  |  |  |  |
| CLI/Web |  |  |  |  |  |  |  |  |  |  |
| 测试调试 |  |  |  |  |  |  |  |  |  |  |
| 文档与演示 |  |  |  |  |  |  |  |  |  |  |

未知字段留空并在备注写明 `unknown`，不要填估算的 0。订阅版若只能按套餐比例估算，必须标记“订阅估算”；本地/学校免费额度写明来源。

## 三、便捷整理命令

下面命令收集当前 Vault 的运行时调用汇总，输出可直接粘贴到阶段统计表；它不会上传任何数据：

```powershell
$vault = ".\workspace\vaults\first_run"
New-Item -ItemType Directory -Force .\deliverables | Out-Null
cargo run --quiet -p readtrace-cli -- --format json usage $vault | Out-File -Encoding utf8 .\deliverables\runtime-usage.json
```

若要把 Workspace、仓库 `tmp/` 以及其它测试 Vault 的调用一次合并，使用扫描模式。它会按 `call_id` 去重；同一调用被复制到多个目录时只计一次，若重复记录中有一份 usage 更完整则优先保留完整版本：

```powershell
cargo run --quiet -p readtrace-cli -- usage `
  --scan-root E:\AI_diary\summer_project `
  --out .\deliverables\runtime-usage-all.json
```

`--scan-root` 可以指向任意包含 Vault 或临时测试输出的目录，命令不要求该目录本身是 Vault；扫描器会读取根目录下所有 `.jsonl`，逐行筛选合法的 `CallRecord`，因此 `tmp\codex-check.jsonl` 这类自定义文件名也会被纳入。第一个位置参数只在未使用扫描模式时作为 Vault 路径。需要纳入系统临时目录时，可把 `--scan-root $env:TEMP` 单独执行，再将结果按 `call_id` 合并，不要手工把同一 JSON 行追加两次。

开发对话原始记录仍从 Codex/OpenCode/ChatGPT 导出后放入 `deliverables/ai-history/`，按阶段人工填写 Excel。建议最终目录：

```text
deliverables/
├─ design.pdf
├─ ai-history/
├─ ai-development-cost.xlsx
└─ runtime-usage.json
```

## 四、当前文档索引

- [`ARCHITECTURE_EXPLAINED.md`](ARCHITECTURE_EXPLAINED.md)：当前实现的模块、数据流、结构、Provider 和费用。
- [`QUICK_START.md`](QUICK_START.md)：Windows/macOS 依赖安装、可选 Provider 配置和第一次启动 Web 的最短路径。
- [`CLI_TUTORIAL.md`](CLI_TUTORIAL.md)：从 import 到 repair/build、删除、恢复和 Web API 的操作手册。
- [`CLI_END_TO_END_EXAMPLE.md`](CLI_END_TO_END_EXAMPLE.md)：假设完全不了解项目时，从路径、导入、OCR、推理强度到合并、引用问答和费用查看的可复制示例。
- [`GITHUB_AND_DEVICE_SETUP.md`](GITHUB_AND_DEVICE_SETUP.md)：Windows/macOS 安装、`.env`、迁移、Web 启动与排错。
- `ASRS.md`、`CORE_LOGIC_AND_DESIGN.md`：当前需求、设计决策和开发过程记录；课程交付文档在最终验收时统一整理。
