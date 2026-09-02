# ReadTrace（读迹）

ReadTrace 是一个面向视觉文本的本地整理工作台：把图片、PDF、TXT 和 Markdown 放进独立 Vault，逐页 OCR，做确定性的文本清洗，再用可替换的 LLM 修复整页内容，最后生成可以人工编辑、检索和引用的 Markdown。原始素材始终保留，修复结果不会覆盖证据。

ReadTrace 的边界很明确：

- .txt、.md 直接读取；.pdf、.png、.jpg、.jpeg、.webp、.bmp 走 Poppler + Tesseract。
- 文件夹递归导入支持的格式，其余格式只记录到 skipped_files。
- 搜索和引用只面向 clean/ 中的最终文档。视觉页面必须先有完整修复；显式使用 --allow-unrepaired 时可以生成带警告的临时稿，但该稿不能成为问答证据。
- 同一个 Rust 协议同时服务 CLI 和 Web；Web 只是工作台界面，不另写一套业务逻辑。

## 1. 先理解三个路径

以下命令均假设当前目录是项目根目录：

~~~text
E:\AI_diary\summer_project\       # Project root：源码、Cargo.toml、.env
E:\AI_diary\summer_project\workspace\  # Workspace：可以包含多个 Vault
E:\AI_diary\summer_project\workspace\vaults\first_run\  # Vault：一个资料库
~~~

把这些路径替换为 macOS 上的对应路径即可。Workspace 和 Vault 是运行数据，不应提交到 Git；真实 Key 也不应提交。

## 2. 构建和系统依赖

需要 Rust stable 和 Git。只有处理图片/PDF 时才需要 Tesseract；PDF 另外需要 Poppler 的 pdfinfo 和 pdftoppm。TXT/Markdown 不依赖外部 OCR 程序。

~~~console
cargo build --workspace
cargo test --workspace
~~~

### macOS

~~~bash
brew install tesseract tesseract-lang poppler
~~~

Apple Silicon 可在项目根目录的 .env 写：

~~~dotenv
READTRACE_TESSERACT_BIN=/opt/homebrew/bin/tesseract
READTRACE_PDFTOPPM_BIN=/opt/homebrew/bin/pdftoppm
READTRACE_PDFINFO_BIN=/opt/homebrew/bin/pdfinfo
~~~

Intel Mac 将 /opt/homebrew 换成 /usr/local。若 ocr-check 找不到 chi_sim，再设置实际的 TESSDATA_PREFIX。

### Windows

在 .env 填写机器上的绝对路径：

~~~dotenv
READTRACE_TESSERACT_BIN=C:/Program Files/Tesseract-OCR/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/Program Files/Tesseract-OCR/tessdata
~~~

两套系统都可以设置：

~~~dotenv
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
~~~

程序读取项目根目录的 .env；未指定路径时，会依次尝试项目内工具目录、Homebrew 常见路径、Windows 常见安装位置和当前 PATH。启动前检查：

~~~console
cargo run --quiet -p readtrace-cli -- ocr-check
~~~

PDF 会先显示栅格化 0/N、n/N 进度，再显示逐页 Tesseract 进度。OCR 并发范围是 1–16，默认 4；降低 READTRACE_OCR_DPI（例如 150）通常更快，但小字识别可能变差。

## 3. 配置 HTTP、Codex 和 Mock

复制配置模板：

~~~console
# Windows PowerShell
Copy-Item .env.example .env

# macOS
cp .env.example .env
~~~

HTTP Provider 使用 OpenAI-compatible Chat Completions，因此可以接学校网关、GLM、DeepSeek、Ollama 或其它自定义 Base URL：

~~~dotenv
READTRACE_BASE_URL=https://lab.cs.tsinghua.edu.cn/ai-platform/api/v1
READTRACE_ENDPOINT_PATH=chat/completions
READTRACE_MODEL=glm-5.2
READTRACE_API_KEY_ENV=THU_AI_PLATFORM_API_KEY
READTRACE_AUTH_HEADER=Authorization
READTRACE_AUTH_SCHEME=Bearer
READTRACE_RESPONSE_FORMAT=json_object
READTRACE_MAX_TOKENS_FIELD=max_tokens
READTRACE_THINKING_MODE=none
READTRACE_TIMEOUT_SECONDS=300
~~~

先检查配置，再发送最小探针：

~~~console
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ai-check --provider http --model glm-5.2 --thinking none
~~~

可用 Provider：

| Provider | 用途 | 需要什么 |
| --- | --- | --- |
| http | 学校网关或任意 OpenAI-compatible 服务 | Base URL、模型和 Key 环境变量 |
| codex-cli | 当前机器上可执行的 Codex 命令 | 已安装并登录 codex |
| mock | 不联网的流程和 UI 测试 | 不需要 Key，费用固定为 USD 0 |

codex-cli 是命令行适配器，不是 Codex Desktop GUI。默认从 PATH 找 codex；Windows 也会识别 .exe、.cmd、.bat、.ps1 和本地安装目录。必要时在 .env 指定 READTRACE_CODEX_BIN。Codex 与 HTTP 必须配套：codex-cli 使用 gpt-5.6-luna 等 Codex 模型，GLM 应改用 --provider http --model glm-5.2。

推理挡位统一为 none|low|medium|high；--speed low|mid|high 是兼容旧脚本的别名。GLM-5.2 的 none 会发送 thinking.type=disabled；GLM-5.3/5.3 Flash 是强制思考模型，会将 none/medium 安全映射为最低 reasoning_effort=low。多页 repair 默认最多并行 4 页，范围 1–64，由 READTRACE_LLM_CONCURRENCY 控制。

Web 中的“来源与 API”页可以保存内置或自定义 profile。Key 只写入用户配置目录（Windows 默认 %LOCALAPPDATA%/ReadTrace/providers.json，macOS 默认 ~/Library/Application Support/ReadTrace/providers.json），不能由网页回显，也不会进入 Git；受限环境才回退到 Vault 的 .readtrace/providers.json。

## 4. 第一次启动 Web 工作台

在项目根目录执行：

~~~console
cargo run --quiet -p readtrace-cli -- workspace-init ./workspace
cargo run --quiet -p readtrace-cli -- vault-create ./workspace default
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
~~~

浏览器打开 <http://127.0.0.1:8787/>。以后只需要最后一条 serve。新增 Vault：

~~~console
cargo run --quiet -p readtrace-cli -- vault-create ./workspace <名称>
~~~

工作台支持 Workspace/Vault 切换、文件树、导入队列、批次处理、跨 batch 合并、clean 预览和编辑、来源配置、检索、引用问答以及后台 Token/费用查看。

## 5. CLI 的完整流程

### 5.1 创建或查看 Vault

~~~console
cargo run --quiet -p readtrace-cli -- ls ./workspace
cargo run --quiet -p readtrace-cli -- vault-list ./workspace
cargo run --quiet -p readtrace-cli -- vault-path ./workspace first_run
cargo run --quiet -p readtrace-cli -- sources ./workspace/vaults/first_run
~~~

默认输出是人类可读摘要；脚本需要完整 JSON 时，在全局位置加 --format json，例如：

~~~console
cargo run --quiet -p readtrace-cli --format json ls ./workspace
~~~

### 5.2 导入文件或文件夹

导入一个文件（请在 PowerShell 写成一行）：

~~~console
cargo run --quiet -p readtrace-cli -- import-file ./workspace/vaults/first_run E:/AI_diary/tests/sample.md --target story
~~~

导入整个目录（递归）：

~~~console
cargo run --quiet -p readtrace-cli -- import-folder ./workspace/vaults/first_run E:/AI_diary/tests --target test-images --order filename
~~~

PowerShell 的反引号只用于换行；上面的一行命令也可直接复制。macOS 请使用对应绝对路径。默认会把原素材复制到 sources/<batch_id>/。素材很大或不想复制时加 --no-copy，manifest 会保存 external_path，但源文件必须一直可访问。命令返回 batch_id，后续每一步都使用它。

TXT/Markdown 有两种路径：

- 想让模型重写：继续执行 ocr（文本页会直接读取）、normalize、repair、build。
- 想立即进入 clean：使用 direct-clean <vault> <batch_id> --clean-name <名称>，不会调用 LLM。

### 5.3 OCR、规范化和 LLM 修复

~~~console
$vault = "./workspace/vaults/first_run"
$batchId = "<import 返回的 batch_id>"
cargo run --quiet -p readtrace-cli -- ocr $vault $batchId --provider real
cargo run --quiet -p readtrace-cli -- normalize $vault $batchId
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider http --model glm-5.2 --thinking none
cargo run --quiet -p readtrace-cli -- build $vault $batchId --clean-name story/chapter-01
~~~

不用联网测试流程时，将 repair 的 Provider 换成 mock。使用 Codex CLI：

~~~console
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider codex-cli --preset codex-luna --thinking high
~~~

想测试不同挡位，可重复执行并加 --refresh：

~~~console
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider http --model glm-5.2 --thinking low --refresh
cargo run --quiet -p readtrace-cli -- repair $vault $batchId --provider http --model glm-5.2 --thinking high --refresh
~~~

每页结果保存为 generated/<batch_id>/repair/<page>.json，失败页不会阻塞其它页；重新执行会跳过已有 checkpoint。可通过 --prompt-file path/to/repair.md 使用自定义提示词，也可以在 Web 中编辑 Vault 的 prompts/repair.md。提示词应要求只返回完整修复正文，不能让模型输出解释、patch 或 confidence。

### 5.4 合并和人工确认

同一 batch 合并：

~~~console
cargo run --quiet -p readtrace-cli -- merge $vault $batchId
cargo run --quiet -p readtrace-cli -- merge $vault $batchId --confirm --clean-name story/chapter-01
~~~

多个来源默认先生成 merge_plan.json，没有 --confirm 不会写入最终 revision。跨 batch 选择最小单位（单个 source 或 clean 文档）：

~~~console
cargo run --quiet -p readtrace-cli -- sources $vault
cargo run --quiet -p readtrace-cli -- merge-units $vault <unit-a> <unit-b>
cargo run --quiet -p readtrace-cli -- merge-units $vault <unit-a> <unit-b> --confirm --clean-name story/complete
~~~

如果视觉页的 repair 失败，默认拒绝 build/merge；只有明确需要临时校对稿时才加 --allow-unrepaired。结果会标记 warning，且不会进入搜索和引用。

删除操作先显示计划，确认后才执行：

~~~console
cargo run --quiet -p readtrace-cli -- delete-batch $vault $batchId
cargo run --quiet -p readtrace-cli -- delete-batch $vault $batchId --confirm
cargo run --quiet -p readtrace-cli -- delete-unit $vault <unit-id> --confirm
~~~

删除会清理目标 batch/unit 的 source、raw、generated 和关联 merge 产物，但保留运行台账与事件流。

### 5.5 搜索、引用和问答

搜索是本地 SQLite 查询，不调用 LLM，而且只索引 clean/：

~~~console
cargo run --quiet -p readtrace-cli -- reindex $vault
cargo run --quiet -p readtrace-cli -- search $vault "舞台上的剧目"
~~~

问答必须明确提供 clean 来源（可重复 --source-ref）：

~~~console
cargo run --quiet -p readtrace-cli -- answer $vault "这段剧情讲了什么？" --provider http --model glm-5.2 --thinking none --source-ref clean/story/chapter-01/document.md
~~~

也可以用 --quote 或 --quote-file 添加人工摘录，并用 --session-id 继续会话。raw OCR、只做 normalization 的视觉文本和未修复页面不会被送进问答证据。

## 6. Vault 中保存什么

~~~text
vault/
├─ sources/<batch_id>/             # 原素材快照（--no-copy 时只保存外部路径）
├─ raw/<batch_id>/                 # batch.json、OcrPage JSON
├─ generated/<batch_id>/
│  ├─ normalization.json           # 可人工检查的确定性清洗
│  ├─ repair/<page>.json           # 每页 repaired_text checkpoint
│  ├─ repair.json                  # repair 汇总
│  └─ <document>/revisions/000N/   # 不可变 revision Markdown
├─ generated/merges/<merge_id>/   # 跨 batch merge plan/revision
├─ clean/<name>/document.md        # 最终可读投影；检索和引用的唯一来源
├─ prompts/repair.md               # Vault 级可编辑提示词（可选）
├─ prompts/profile.md              # 角色/专名上下文（可选）
├─ runtime/calls.jsonl             # Token、费用、耗时、成功状态
├─ events/events.jsonl             # 进度、完成和错误事件
└─ .readtrace/state.db             # 可重建的本地索引
~~~

每次 build 或确认 merge 都会发布一个 clean 文档；同名发布只替换 clean 投影，不删除 generated 历史 revision。文件浏览可以预览并直接编辑保存 Markdown/TXT，保存后自动刷新索引。

## 7. Token 与费用

repair、answer、ai-check 每次调用都会追加到 runtime/calls.jsonl，无论成功失败都计数。HTTP/Codex 返回的 input、cached input、output、reasoning 和 total Token 原样保存；没有 usage 的失败调用保持 null，不从文本长度猜测。扫描整个项目时按 call_id 去重：

~~~console
cargo run --quiet -p readtrace-cli -- usage $vault
cargo run --quiet -p readtrace-cli -- usage --scan-root E:/AI_diary/summer_project --out ./deliverables/runtime-usage-all.json
~~~

费用公式：

~~~text
cached = min(cached_input_tokens, input_tokens)
uncached = input_tokens - cached
USD = uncached/1,000,000 × input_price
    + cached/1,000,000 × cached_input_price
    + output_tokens/1,000,000 × output_price
CNY = USD × usd_to_cny
~~~

每条记录保存调用时的价格快照和汇率（默认 USD_TO_CNY=6.8）。当前内置美元价格（每百万 Token）：

| 模型 | input | cached input | output |
| --- | ---: | ---: | ---: |
| GPT-5.6 Luna | 0.20 | 0.02 | 1.20 |
| GPT-5.6 Terra | 2.00 | 0.20 | 12.00 |
| GPT-5.6 Sol | 4.00 | 0.40 | 20.00 |
| GPT-5.5 | 5.00 | 0.50 | 30.00 |
| GPT-5.4 Mini | 0.75 | 0.075 | 4.50 |
| GLM 5.3 Flash | 0.15 | 0.03 | 0.50 |
| GLM 5.2 | 1.40 | 0.26 | 4.40 |

未知自定义模型可在 .env 填写 READTRACE_INPUT_PRICE、READTRACE_CACHED_INPUT_PRICE、READTRACE_OUTPUT_PRICE。Mock 明确记为 USD 0；只有 Token 或价格确实缺失时才会显示 unknown。课程要求的开发阶段 AI 开销（人时、外部对话、订阅费用）与本运行时台账分开，按 [docs/DELIVERABLES_AND_COST_NOTES.md](docs/DELIVERABLES_AND_COST_NOTES.md) 人工整理。

## 8. 文档索引和课程交付

- [docs/ARCHITECTURE_EXPLAINED.md](docs/ARCHITECTURE_EXPLAINED.md)：模块、数据流、关键结构、Provider、费用和当前完成度。
- [docs/CLI_TUTORIAL.md](docs/CLI_TUTORIAL.md)：命令参数、断点恢复、删除和 API。
- [docs/CLI_END_TO_END_EXAMPLE.md](docs/CLI_END_TO_END_EXAMPLE.md)：从导入到合并、引用问答的可复制流程。
- [docs/COMPLETE_FLOW_TUTORIAL.md](docs/COMPLETE_FLOW_TUTORIAL.md)：PDF、Markdown、TXT 和两张图片的完整示例。
- [docs/GITHUB_AND_DEVICE_SETUP.md](docs/GITHUB_AND_DEVICE_SETUP.md)：GitHub、Windows/macOS、Key 和 Vault 迁移。
- [docs/WEB_GUI_PROTOCOL.md](docs/WEB_GUI_PROTOCOL.md)：Web API 和 SSE 契约。
- [docs/IMPLEMENTATION_AUDIT.md](docs/IMPLEMENTATION_AUDIT.md)：逐项验收、已知边界和检查结果。
- [docs/DELIVERABLES_AND_COST_NOTES.md](docs/DELIVERABLES_AND_COST_NOTES.md)：课程交付清单与运行时/开发成本的分界。

课程的 PDF 设计文档、完整 AI 对话历史和 Excel 开发开销表在最终验收阶段统一导出到网络学堂；源代码仓库只提交源码、README、模板和可复现实例，不提交真实 Key、Workspace、运行记录或其它本地数据。
