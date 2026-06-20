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

    let raw_content = normalized_content_tokens(raw);
    let candidate_content = normalized_content_tokens(candidate);

    !raw_content.is_empty() && raw_content == candidate_content
}

fn normalized_content_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() || is_ignored_punctuation(ch) {
            flush_current(&mut tokens, &mut current);
            continue;
        }

        if is_preserved_standalone_symbol(ch) {
            flush_current(&mut tokens, &mut current);
            tokens.push(ch.to_string());
            continue;
        }

        current.push(ch.to_ascii_lowercase());
    }

    flush_current(&mut tokens, &mut current);
    tokens
}

fn flush_current(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn is_preserved_standalone_symbol(ch: char) -> bool {
    matches!(
        ch,
        '$' | '+' | '=' | '@' | '#' | '%' | '&' | '*' | '￥' | '€' | '£' | '¥'
    )
}

fn is_ignored_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | '\''
            | '"'
            | '`'
            | '-'
            | '_'
            | '/'
            | '\\'
            | '|'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '。'
            | '？'
            | '！'
            | '，'
            | '、'
            | '；'
            | '：'
            | '「'
            | '」'
            | '『'
            | '』'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '（'
            | '）'
            | '《'
            | '》'
            | '〈'
            | '〉'
            | '…'
            | '—'
            | '～'
            | '·'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_content_removes_common_punctuation_but_preserves_word_boundaries() {
        assert_eq!(
            normalized_content_tokens(" Hello, 小王！ "),
            vec!["hello".to_string(), "小王".to_string()]
        );
    }

    #[test]
    fn normalized_content_preserves_symbols_as_tokens() {
        assert_eq!(
            normalized_content_tokens("a+b = $5"),
            vec![
                "a".to_string(),
                "+".to_string(),
                "b".to_string(),
                "=".to_string(),
                "$".to_string(),
                "5".to_string(),
            ]
        );
    }
}
