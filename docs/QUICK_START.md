# ReadTrace Quick Start

这份文档只解决一件事：在一台新机器上，从克隆仓库到打开 Web 工作台，并完成第一次导入。先走 Web，不需要先记住全部 CLI 参数；CLI 和更细的恢复/合并操作见 [CLI_TUTORIAL.md](CLI_TUTORIAL.md)。

## 0. 先分清三个位置

命令都从项目根目录执行。项目根目录是包含 Cargo.toml 的目录：

~~~text
<项目根目录>/                    # 源码、Cargo.toml、.env
<项目根目录>/workspace/           # 一个 Workspace，可放多个 Vault
<项目根目录>/workspace/vaults/demo/ # 一个 Vault，保存一组资料
~~~

外部输入文件可以在任意位置。导入后，默认会复制一份到 Vault 的 sources；不想复制时，导入选项中关闭“复制素材”，或在 CLI 使用 --no-copy。

## 1. 安装共同依赖

需要 Rust stable 和 Git。Rust 推荐通过 [rustup](https://rustup.rs/) 安装。

~~~console
git --version
cargo --version
~~~

如果只想先熟悉界面，可以暂时不安装 OCR 和 LLM：使用 Mock Provider，Workspace、文件树、任务状态、搜索和会话都可以先测试。处理真实图片/PDF 时，再完成下一节。

## 2. Windows：安装 Tesseract 和 Poppler

Windows 没有一个叫 tesseract-lang 的通用命令。tesseract-lang 是 Homebrew 下的语言数据包名称；Windows 要安装 Tesseract 主程序，并确认 tessdata 中有简体中文训练数据 chi_sim。Tesseract 官方安装说明列出了 Windows 安装器和语言数据的放置位置：[tessdoc Installation](https://tesseract-ocr.github.io/tessdoc/Installation.html)。

### 2.1 安装 Tesseract

1. 打开官方安装说明中的 Windows 入口，下载可信的 64 位 Tesseract 安装器。
2. 安装到默认目录或你自己的目录，例如 C:\Program Files\Tesseract-OCR。
3. 安装时选择 Chinese / Simplified Chinese；如果安装器没有语言选项，就从官方 tessdata 下载 chi_sim.traineddata，并把它放进 Tesseract-OCR\tessdata。
4. 记下两个路径：
   - 可执行文件：C:\Program Files\Tesseract-OCR\tesseract.exe
   - 语言目录：C:\Program Files\Tesseract-OCR\tessdata

在 PowerShell 验证：

~~~powershell
& "C:\Program Files\Tesseract-OCR\tesseract.exe" --version
& "C:\Program Files\Tesseract-OCR\tesseract.exe" --list-langs
~~~

输出中应有 chi_sim 和 eng。Tesseract 不在 PATH 也没有关系，ReadTrace 可以从 .env 使用绝对路径。

### 2.2 安装 Poppler

PDF 需要 Poppler 的两个命令：

- pdfinfo：读取 PDF 页数；
- pdftoppm：把 PDF 页面栅格化为图片。

Windows 没有统一的官方安装器。可以使用可信的预编译包，例如 [poppler-windows releases](https://github.com/oschwartz10612/poppler-windows/releases)。解压后找到包含 bin 的目录，例如 C:\tools\poppler\Library\bin，并确认其中有：

~~~text
C:\tools\poppler\Library\bin\pdftoppm.exe
C:\tools\poppler\Library\bin\pdfinfo.exe
~~~

在 PowerShell 验证：

~~~powershell
& "C:\tools\poppler\Library\bin\pdftoppm.exe" -h
& "C:\tools\poppler\Library\bin\pdfinfo.exe" -h
~~~

### 2.3 配置项目 .env

在项目根目录复制模板：

~~~powershell
Copy-Item .env.example .env
~~~

然后打开 .env，按实际安装路径填写：

~~~dotenv
READTRACE_TESSERACT_BIN=C:/Program Files/Tesseract-OCR/tesseract.exe
READTRACE_PDFTOPPM_BIN=C:/tools/poppler/Library/bin/pdftoppm.exe
READTRACE_PDFINFO_BIN=C:/tools/poppler/Library/bin/pdfinfo.exe
TESSDATA_PREFIX=C:/Program Files/Tesseract-OCR/tessdata
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
~~~

建议使用正斜杠。检查 ReadTrace 实际解析到的程序：

~~~powershell
cargo run --quiet -p readtrace-cli -- ocr-check
~~~

图片/PDF 的三个可执行文件和语言包可用后，输出中的 tesseract_available、pdftoppm_available、pdfinfo_available 应为 true。PDF 任务会显示 0/N、n/N 和最终完成状态。

## 3. macOS：安装 Tesseract 和 Poppler

~~~bash
brew install tesseract tesseract-lang poppler
command -v tesseract
command -v pdftoppm
command -v pdfinfo
tesseract --list-langs
~~~

Apple Silicon 的常见路径是 /opt/homebrew/bin；Intel Mac 通常是 /usr/local/bin。必要时在 .env 指定：

~~~dotenv
READTRACE_TESSERACT_BIN=/opt/homebrew/bin/tesseract
READTRACE_PDFTOPPM_BIN=/opt/homebrew/bin/pdftoppm
READTRACE_PDFINFO_BIN=/opt/homebrew/bin/pdfinfo
READTRACE_OCR_LANGUAGES=chi_sim+eng
READTRACE_OCR_DPI=200
READTRACE_OCR_CONCURRENCY=4
~~~

macOS 通常不需要 TESSDATA_PREFIX；只有 ocr-check 找不到 chi_sim 时才填写实际语言目录。

## 4. 构建项目

~~~console
cargo build --workspace
cargo test --workspace
~~~

## 5. 第一次启动 Web 工作台

这是最短的可用路径。API Key 和真实 LLM 都是可选的，首次可以直接用 Mock：

~~~console
cargo run --quiet -p readtrace-cli -- workspace-init ./workspace
cargo run --quiet -p readtrace-cli -- vault-create ./workspace default
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8787
~~~

浏览器打开 <http://127.0.0.1:8787/>。

如果 workspace 已经存在，只需要运行最后一条 serve。若端口被占用，可换端口：

~~~console
cargo run --quiet -p readtrace-cli -- serve ./workspace --bind 127.0.0.1:8788
~~~

### 5.1 在 GUI 中完成第一轮

1. 左侧选择 Workspace 和 Vault。
2. 打开“来源与 API”。不配置 Key 时，选择 Mock；需要真实修复时，再添加 HTTP profile，填 Base URL、模型和 Key 环境变量。
3. 打开“导入”，选择一个 PDF、Markdown、TXT 或图片文件；也可以选择文件夹。
4. 设置是否复制原素材、clean 发布名称、OCR Provider、LLM Provider 和推理挡位。
5. 启动任务后，在“处理批次”查看 OCR、normalize、repair、build 的完成、失败、页数和 warning。后台页会显示任务事件、Token 和美元费用。
6. 到“文件浏览”打开 clean 下的 Markdown。它可以直接编辑和保存；保存后索引会刷新。
7. 到“检索”搜索 clean 内容。检索不调用 LLM。
8. 到“阅读与问答”新建会话，点击“添加引用”，在 clean 文件树中多选文件，再提问。问答只使用 clean 证据。

## 6. 什么时候需要配置 Provider

Provider 配置不是启动 Web 的前置条件：

- 只测试界面、导入队列和任务状态：使用 Mock，不需要 Key。
- 真实 OCR 只需要 Tesseract/Poppler；OCR 与 LLM 是两个阶段。
- 需要模型修复或问答时，配置 HTTP 或 Codex CLI。
- 可以先启动 GUI，再从“来源与 API”添加、编辑和测试 profile；Key 会保存在用户配置目录，不会回显到网页，也不会进入 Git。

HTTP profile 的最小 .env 示例：

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
~~~

也可以在 CLI 发送探针：

~~~console
cargo run --quiet -p readtrace-cli -- provider-check
cargo run --quiet -p readtrace-cli -- ai-check --provider http --model glm-5.2 --thinking none
~~~

Codex CLI 只有在本机安装并登录命令行版 Codex 时才需要。Codex Desktop 的登录状态不等于 shell 中可执行的 codex 命令。

## 7. 用 CLI 做一次最小流程（可选）

GUI 能完成完整流程；需要脚本或复现时，在项目根目录运行：

~~~console
cargo run --quiet -p readtrace-cli -- import-file ./workspace/vaults/default /path/to/story.md --target story
cargo run --quiet -p readtrace-cli -- ocr ./workspace/vaults/default <batch_id> --provider real
cargo run --quiet -p readtrace-cli -- normalize ./workspace/vaults/default <batch_id>
cargo run --quiet -p readtrace-cli -- repair ./workspace/vaults/default <batch_id> --provider http --model glm-5.2 --thinking none
cargo run --quiet -p readtrace-cli -- build ./workspace/vaults/default <batch_id> --clean-name story/chapter-01
~~~

跨 batch 合并先用 sources 查看 unit，再生成 merge plan；确认后才会写 clean。详细参数见 [CLI_TUTORIAL.md](CLI_TUTORIAL.md) 和 [CLI_END_TO_END_EXAMPLE.md](CLI_END_TO_END_EXAMPLE.md)。

## 8. 常见问题

| 现象 | 处理 |
| --- | --- |
| tesseract not found | 在 .env 填 READTRACE_TESSERACT_BIN，重跑 ocr-check |
| chi_sim 缺失 | Windows 把 chi_sim.traineddata 放进 tessdata；macOS 安装 tesseract-lang |
| PDF 无法读取页数 | 同时检查 pdfinfo 和 pdftoppm，不能只安装 Tesseract |
| 只能使用 Mock | 先在 GUI 添加 HTTP profile，或检查 API Key 环境变量 |
| Codex CLI 拒绝访问/找不到命令 | 在普通终端验证 codex；不具备 CLI 时改用 HTTP 或 Mock |
| 看不到搜索结果 | 确认文件已经发布到 clean；必要时在文件浏览保存后执行 reindex |

更完整的双平台安装和设备迁移见 [GITHUB_AND_DEVICE_SETUP.md](GITHUB_AND_DEVICE_SETUP.md)；架构和数据边界见 [ARCHITECTURE_EXPLAINED.md](ARCHITECTURE_EXPLAINED.md)。
