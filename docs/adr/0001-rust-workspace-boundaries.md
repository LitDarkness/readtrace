# Rust workspace boundaries

ReadTrace 使用 Rust workspace 拆分 `readtrace-core`、`readtrace-cli` 和 `readtrace-server`。核心领域、Provider trait、Agent 状态和持久化只放在 core，CLI 与 Web 只是适配器；这样既满足 R1，又让两种界面共享同一套安全边界和可恢复任务模型。
