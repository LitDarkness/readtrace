# Provider traits with OpenAI-compatible HTTP

OCR 与 LLM 均通过 Rust trait 接入，真实实现分别调用 Tesseract/Poppler 和 OpenAI-compatible Chat Completions，Mock 实现只用于离线测试与演示。Provider 预设包含 GLM、DeepSeek、OpenAI、SiliconFlow、OpenRouter、Ollama，Endpoint/模型/Key 仍可完全自定义。HTTP Adapter 同时支持完整 Endpoint 与 Base URL + path，并可调整认证头、response format 和 token 字段，避免把 Agent Loop 锁定到单一服务。
