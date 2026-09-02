use crate::InputMode;
use std::fs;

const BUILTIN_TEXT_REPAIR_PROMPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/text_repair_system.md"
));

/// Return the raw editable OCR repair prompt template. A runtime file can
/// override the built-in template without changing Rust code; `{mode}` is
/// intentionally kept for callers that need to reuse one prompt across input
/// profiles.
pub fn repair_prompt_template() -> String {
    std::env::var("READTRACE_CORRECTION_PROMPT_FILE")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| BUILTIN_TEXT_REPAIR_PROMPT.to_owned())
}

/// Return the editable OCR repair prompt with the current input profile
/// substituted into `{mode}`.
pub fn text_repair_system_prompt(mode: &InputMode) -> String {
    repair_prompt_template().replace("{mode}", &mode.to_string())
}

/// Resolve a prompt for a vault/run. Explicit CLI path wins, then the vault's
/// editable prompt files, then the environment override and builtin prompt.
/// The profile file is appended so users can keep character aliases and other
/// domain facts in a small human-editable document.
pub fn repair_prompt_for(
    mode: &InputMode,
    project_root: &std::path::Path,
    explicit_path: Option<&std::path::Path>,
) -> (String, Option<String>) {
    let mut candidates = explicit_path
        .map(|p| p.to_path_buf())
        .into_iter()
        .chain([project_root.join("prompts/repair.md")]);
    let (mut text, path) = candidates
        .find_map(|path| {
            fs::read_to_string(&path)
                .ok()
                .map(|text| (text, Some(path)))
        })
        .unwrap_or_else(|| (text_repair_system_prompt(mode), None));
    let profile = project_root.join("prompts/profile.md");
    if let Ok(profile_text) = fs::read_to_string(&profile) {
        text.push_str("\n\n# Profile context\n");
        text.push_str(&profile_text);
    }
    (
        text.replace("{mode}", &mode.to_string()),
        path.map(|p| p.to_string_lossy().to_string()),
    )
}
