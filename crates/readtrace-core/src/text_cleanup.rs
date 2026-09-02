use serde::{Deserialize, Serialize};

/// Human-readable audit entry for a deterministic, non-semantic cleanup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizationChange {
    pub rule: String,
    pub line: Option<usize>,
    pub before: String,
    pub after: String,
    pub reason: String,
}

/// Text presented to the LLM as the deterministic base for full-page repair.
/// The OCR page under `raw/` remains the immutable source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedPage {
    pub page_id: String,
    pub source_id: String,
    pub page_number: usize,
    pub source_ref: String,
    pub raw_text: String,
    pub normalized_text: String,
    pub changes: Vec<NormalizationChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationReport {
    pub schema_version: u32,
    pub batch_id: String,
    pub pages: Vec<PreparedPage>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Remove OCR-only whitespace without guessing words or changing characters.
/// Latin word boundaries and Markdown blockquote spacing are preserved.
pub fn normalize_ocr_text(input: &str) -> (String, Vec<NormalizationChange>) {
    let canonical = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut changes = Vec::new();
    if canonical != input {
        changes.push(NormalizationChange {
            rule: "line_endings".into(),
            line: None,
            before: "mixed or CRLF line endings".into(),
            after: "LF line endings".into(),
            reason: "统一行尾，避免跨平台偏移不一致".into(),
        });
    }

    let mut output_lines = Vec::new();
    let mut previous_blank = true;
    let mut collapsed_blank_lines = 0usize;
    for (index, original_line) in canonical.split('\n').enumerate() {
        let trimmed = original_line.trim_end();
        let compacted = compact_inline_whitespace(trimmed);
        if compacted.is_empty() {
            if previous_blank {
                collapsed_blank_lines += 1;
                continue;
            }
            output_lines.push(String::new());
            previous_blank = true;
            continue;
        }
        previous_blank = false;
        if compacted != original_line {
            changes.push(NormalizationChange {
                rule: "ocr_whitespace".into(),
                line: Some(index + 1),
                before: original_line.into(),
                after: compacted.clone(),
                reason: "删除中日韩字符与标点之间的 OCR 分隔空格，并收拢其它连续空白".into(),
            });
        }
        output_lines.push(compacted);
    }
    while output_lines.last().is_some_and(|line| line.is_empty()) {
        output_lines.pop();
        collapsed_blank_lines += 1;
    }
    if collapsed_blank_lines > 0 {
        changes.push(NormalizationChange {
            rule: "blank_lines".into(),
            line: None,
            before: format!("{collapsed_blank_lines} redundant blank line(s)"),
            after: "at most one blank line between text blocks".into(),
            reason: "移除首尾空行并合并连续空行".into(),
        });
    }
    (output_lines.join("\n"), changes)
}

fn compact_inline_whitespace(line: &str) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(line.len());
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_whitespace() {
            out.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        let previous = out.chars().next_back();
        let next = chars.get(index).copied();
        let remove = matches!((previous, next), (Some(a), Some(b)) if removable_ocr_gap(a, b));
        if !remove && start > 0 && next.is_some() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out
}

fn removable_ocr_gap(left: char, right: char) -> bool {
    (is_cjk(left) && is_cjk(right))
        || (is_cjk(left) && is_punctuation(right))
        || (is_punctuation(left) && is_cjk(right))
        || (left == '…' && right == '|')
}

fn is_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x2E80..=0x2FDF
            | 0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

fn is_punctuation(c: char) -> bool {
    matches!(
        c,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '（'
            | '）'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
            | '【'
            | '】'
            | '“'
            | '”'
            | '‘'
            | '’'
            | ':'
            | ';'
            | ','
            | '.'
            | '!'
            | '?'
            | '('
            | ')'
            | '['
            | ']'
            | '…'
            | '|'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_cjk_ocr_gaps_but_keeps_latin_words() {
        let input =
            "\n冬日 静默 如 谜  \r\n\r\n> 沃 雅 妮 莎 : 莱 帕 娜 !\r\nBanished from home\r\n";
        let (text, changes) = normalize_ocr_text(input);
        assert_eq!(
            text,
            "冬日静默如谜\n\n> 沃雅妮莎:莱帕娜!\nBanished from home"
        );
        assert!(changes.iter().any(|change| change.rule == "ocr_whitespace"));
        assert!(changes.iter().any(|change| change.rule == "blank_lines"));
    }

    #[test]
    fn does_not_change_semantic_characters() {
        let input = "版本 2.0 alpha\n中文 English 123";
        assert_eq!(normalize_ocr_text(input).0, input);
    }

    #[test]
    fn closes_ocr_ellipsis_separator_gap() {
        assert_eq!(normalize_ocr_text("事件 … |").0, "事件…|");
    }
}
