# ADR 0005：确定性规范化与多 Vault

## 状态

已接受。

## 决策

- 中文/日文/韩文字符间 OCR 空格、标点邻接空格、行尾空白和多余空行由 `text_cleanup` 确定性处理，不猜测词义。
- 结果写入 `generated/<batch_id>/normalization.json`，raw 不变；人可以直接编辑规范化文本。
- LLM repair 只接收规范化文本，按页保存完整修复文本和 prompt hash；不再依赖 byte/Unicode patch 偏移。
- Workspace 管理多个 Vault，Vault 之间不共享 sources、generated 或 runtime ledger；搜索只在当前 Vault 重建 SQLite 索引。

## 后果

机械噪声与语义修复分层，模型输入稳定且可审计；人工修改规范化层后可直接重跑 repair。不同资料集合互不污染，外部大文件可以 `--no-copy` 引用。
