use crate::{AppConfig, CodexCliProvider, LlmProvider, MockLlmProvider, OpenAiCompatibleProvider};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Provider names shared by the CLI, Web adapter and future GUI. The
/// provider implementation stays behind this small interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LlmBackend {
    #[default]
    Http,
    Mock,
    CodexCli,
}

impl FromStr for LlmBackend {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "http" | "openai-compatible" => Ok(Self::Http),
            "mock" => Ok(Self::Mock),
            "codex" | "codex-cli" => Ok(Self::CodexCli),
            other => Err(anyhow!(
                "unknown LLM provider `{other}`; use http, mock or codex-cli"
            )),
        }
    }
}

/// A user-facing speed shortcut. It is deliberately separate from a model:
/// changing low/mid/high changes reasoning effort, not the selected model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSpeed {
    Low,
    Mid,
    High,
}

impl ReasoningSpeed {
    pub fn thinking(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "medium",
            Self::High => "high",
        }
    }
}

impl FromStr for ReasoningSpeed {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" | "fast" => Ok(Self::Low),
            "mid" | "medium" | "balanced" => Ok(Self::Mid),
            "high" | "quality" => Ok(Self::High),
            other => Err(anyhow!("unknown speed `{other}`; use low, mid or high")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmOptions {
    #[serde(default)]
    pub backend: LlmBackend,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub speed: Option<ReasoningSpeed>,
}

#[derive(Debug, Clone)]
pub struct ResolvedLlm {
    pub backend: LlmBackend,
    pub config: AppConfig,
}

impl LlmOptions {
    /// Resolve one complete provider selection. Environment loading and
    /// published model prices happen here so every adapter follows the same
    /// rules instead of copying them in CLI and Web handlers.
    pub fn resolve(&self) -> Result<ResolvedLlm> {
        if self.backend == LlmBackend::CodexCli
            && self
                .model
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("glm"))
        {
            return Err(anyhow!(
                "Codex CLI 不能使用 GLM 模型；请使用 HTTP provider，或选择 codex-luna"
            ));
        }
        let effective_preset = if self.backend == LlmBackend::CodexCli
            && self.preset.is_none()
            && self.model.is_none()
        {
            Some("codex-luna".to_owned())
        } else {
            self.preset.clone()
        };
        let mut config = effective_preset
            .as_deref()
            .map(AppConfig::for_preset)
            .unwrap_or_else(AppConfig::from_env);
        let env_model = config.model.clone();
        let codex_preset = effective_preset.as_deref().is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "codex" | "codex-luna" | "codex-5.6-luna" | "gpt-5.6-luna"
            )
        });
        if codex_preset {
            config.model = "gpt-5.6-luna".into();
            config.thinking_mode = "high".into();
            config.max_tokens_field = "max_completion_tokens".into();
        }
        if let Some(model) = self
            .model
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            config.model = model.trim().to_owned();
        }
        let model_overridden = self
            .model
            .as_deref()
            .is_some_and(|model| !model.trim().eq_ignore_ascii_case(&env_model));
        if let Some(thinking) = self
            .thinking
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            config.thinking_mode = thinking.trim().to_owned();
        }
        if let Some(speed) = self.speed {
            config.thinking_mode = speed.thinking().into();
        }

        // GLM-5.3 and GLM-5.3-Flash always think.  Normalize the user-facing
        // `none`/`medium` aliases to the actual minimum effort so ledgers,
        // task cards and payloads all report the same effective setting.
        if config
            .model
            .trim()
            .to_ascii_lowercase()
            .starts_with("glm-5.3")
            && matches!(
                config.thinking_mode.trim().to_ascii_lowercase().as_str(),
                "" | "default" | "none" | "disabled" | "off" | "false" | "medium"
            )
        {
            config.thinking_mode = "low".into();
        }

        // A named preset is an explicit model selection, so its published
        // price wins over unrelated prices in the ambient `.env` (for
        // example, a GLM `.env` must not price a Codex Luna run). Without a
        // preset, non-zero prices remain a deliberate local override.
        let has_explicit_prices = config.input_price_per_million > 0.0
            || config.cached_input_price_per_million > 0.0
            || config.output_price_per_million > 0.0;
        if model_overridden && effective_preset.is_none() {
            config.input_price_per_million = 0.0;
            config.cached_input_price_per_million = 0.0;
            config.output_price_per_million = 0.0;
            config.pricing_version = "unset".into();
        }
        if effective_preset.is_some() || model_overridden || !has_explicit_prices {
            config.apply_official_model_pricing();
        }
        Ok(ResolvedLlm {
            backend: self.backend,
            config,
        })
    }

    pub fn provider(&self, config: &AppConfig) -> Box<dyn LlmProvider> {
        match self.backend {
            LlmBackend::Http => Box::new(OpenAiCompatibleProvider::new(config.clone())),
            LlmBackend::Mock => Box::new(MockLlmProvider),
            LlmBackend::CodexCli => Box::new(CodexCliProvider::new(config)),
        }
    }
}

impl ResolvedLlm {
    pub fn provider(&self) -> Box<dyn LlmProvider> {
        LlmOptions {
            backend: self.backend,
            ..Default::default()
        }
        .provider(&self.config)
    }
}
