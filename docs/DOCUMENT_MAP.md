# ReadTrace 文档导航

这里按阅读目的列出产品仓库中的文档。命令都从项目根目录执行；`Workspace`、`Vault`、运行台账和导入素材是运行时数据，不是仓库内容。

## 使用者

- [`README.md`](../README.md)：安装方式、Release 快速开始、源码构建、Provider 和命令总览。
- [`QUICK_START.md`](QUICK_START.md)：新机器从依赖安装到首次打开 Web 工作台。
- [`CLI_TUTORIAL.md`](CLI_TUTORIAL.md)：命令参数、断点恢复、合并、删除、搜索和费用查看。
- [`CLI_END_TO_END_EXAMPLE.md`](CLI_END_TO_END_EXAMPLE.md)：从导入到 clean、检索和引用问答的可复制流程。
- [`COMPLETE_FLOW_TUTORIAL.md`](COMPLETE_FLOW_TUTORIAL.md)：PDF、Markdown、TXT 和图片混合输入的完整示例。

## 开发者与维护者

- [`CONTEXT.md`](../CONTEXT.md)：领域术语、不变量和 Provider 合约。
- [`ARCHITECTURE.md`](../ARCHITECTURE.md)：架构入口。
- [`ARCHITECTURE_EXPLAINED.md`](ARCHITECTURE_EXPLAINED.md)：当前实现的权威架构、数据流和边界。
- [`WEB_GUI_PROTOCOL.md`](WEB_GUI_PROTOCOL.md)：Web API、任务状态和 SSE 契约。
- [`IMPLEMENTATION_AUDIT.md`](IMPLEMENTATION_AUDIT.md)：测试、验收结果和已知限制。
- [`adr/`](adr/)：解释关键设计取舍。

## 发布者

- [`RELEASE_GUIDE.md`](RELEASE_GUIDE.md)：Windows x86_64 和 macOS arm64 自包含压缩包、第三方许可证、GitHub Actions 和 Release 发布流程。
- [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md)：随发布包分发的依赖许可证说明。

## 仓库边界

课程排版稿、AI 原始记录、费用表、个人书籍/图片、Vault、Workspace、`.env` 和运行台账可能仍保留在开发机上，但不会进入产品仓库。`.gitignore` 已覆盖这些本机内容；README 和这里的运行手册不依赖它们。

README 负责“怎么安装和运行”，架构说明负责“代码怎样组织”，ADR 负责“为什么这样取舍”，教程负责“怎样复现一条流程”。它们不把未来想法写成已实现功能，也不要求使用者先阅读课程材料。
