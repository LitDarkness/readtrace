# ADR 0006: Codex CLI as an optional local Provider

## Context

部分环境可以通过学校 OpenAI-compatible 网关调用 GLM，但该网关不一定暴露 Codex 模型；本机 Codex CLI 已经具备登录态，并支持 `gpt-5.6-luna` 与 `model_reasoning_effort`。

## Decision

新增 `CodexCliProvider`，实现与 HTTP/Mock 相同的 `LlmProvider` 接口。它通过 `codex exec --ephemeral --sandbox read-only --json` 发送提示词：`--output-last-message` 提供最终回答，JSONL 的 `turn.completed.usage` 提供 Token，`thread.started.thread_id` 作为本地 request id。默认模型和推理强度由 `codex-luna` 预设提供，也可以用 `--model`、`--thinking` 覆盖。AI 探针通过 `ai-check --provider codex-cli` 调用同一实现。

可执行文件由 `READTRACE_CODEX_BIN` 控制，默认值为 `codex`。适配器会显式检查 Windows 的 `.exe`、`.cmd`、`.bat`、`.ps1` 后缀，并在 PATH 不完整时尝试 Codex Desktop 的本地安装目录；`.ps1` 通过 PowerShell 启动。这样 IDE、旧 PowerShell 会话和 npm shim 都不会被误判为“模型不可用”。

## Safety and trade-offs

- Codex CLI 使用已有登录态，不读取或转发 ReadTrace 的 API Key。
- 每次调用都在随机临时空目录中运行；只读沙箱、临时会话和禁止工具提示词避免模型读取 `.env` 或修改项目文件。
- Codex 的系统上下文使单次调用 Token 开销更高；旧版本适配器没有打开 `--json`，因此历史 ledger 中的 Token 仍是 `null`，无法从旧的最终文本倒推。当前版本会读取 JSONL usage；如果本机 CLI 不产生该事件，才回退为 unknown。开发阶段 Excel 仍按外部账单人工登记。
- 该 Provider 依赖本机 Codex CLI 可执行文件和登录状态，不适合无 Codex CLI 的部署环境；Codex Desktop GUI 本身不是可供 ReadTrace 直接调用的 HTTP 服务。此时使用 HTTP 或 Mock Provider，或在 `.env` 设置 `READTRACE_CODEX_BIN` 的绝对路径。
- 在 Codex Desktop/受限代理终端中，子进程可能无法写入用户的 `CODEX_HOME`，导致 app-server 初始化报 `拒绝访问 (os error 5)` 或 `readonly database`；若随后出现 `UnknownIssuer`/`invalid peer certificate`，则是该宿主的 CA 信任边界。ReadTrace 会保留原始错误并给出切换普通 PowerShell/Windows Terminal 的建议，但不会复制 `auth.json`、改变登录态或绕过 Codex 的安全限制。相同命令在普通外部 PowerShell 成功后，说明 Provider 协议和网络请求本身正常。
