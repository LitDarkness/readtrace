# File-first project storage

Markdown、JSON、JSONL 和原始来源是 Project 的事实来源，SQLite 只作为可删除重建的检索投影。这个选择保留了用户用普通编辑器维护资料的能力，同时提供可接受的搜索、账单和事件查询性能；`reindex`/`rebuild` 负责从文件恢复投影。
