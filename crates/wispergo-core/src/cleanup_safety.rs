/// Return true when a punctuation-cleanup candidate preserves all non-punctuation
/// transcript content.
///
/// The safety gate intentionally prefers false negatives over false positives:
/// rejecting useful punctuation falls back to raw ASR, while accepting a rewrite
/// silently corrupts user dictation.
pub fn is_safe_punctuation_cleanup(raw: &str, candidate: &str) -> bool {
    if raw.trim().is_empty() || candidate.trim().is_empty() {
        return false;
    }

    let raw_content = normalized_content(raw);
    let candidate_content = normalized_content(candidate);

    !raw_content.is_empty() && raw_content == candidate_content
}

fn normalized_content(text: &str) -> String {
    text.chars()
        .filter_map(normalized_content_char)
        .collect::<String>()
}

fn normalized_content_char(ch: char) -> Option<char> {
    if is_ignored_punctuation_or_spacing(ch) {
        return None;
    }

    Some(ch.to_ascii_lowercase())
}

fn is_ignored_punctuation_or_spacing(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '？' | '！' | '，' | '、' | '；' | '：' | '「' | '」' | '『' | '』'
                | '“' | '”' | '‘' | '’' | '（' | '）' | '《' | '》' | '〈' | '〉'
                | '…' | '—' | '～' | '·' | '￥'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_content_removes_common_punctuation_and_spaces() {
        assert_eq!(normalized_content(" Hello, 小王！ "), "hello小王".to_string());
    }
}
