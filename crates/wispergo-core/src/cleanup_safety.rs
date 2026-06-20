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

        if is_preserved_standalone_symbol(ch) || is_cjk_ideograph(ch) {
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
        '$' | '+' | '=' | '<' | '>' | '@' | '#' | '%' | '&' | '*' | '￥' | '€' | '£' | '¥'
    )
}

fn is_cjk_ideograph(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{30000}'..='\u{3134F}'
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
            vec!["hello".to_string(), "小".to_string(), "王".to_string()]
        );
    }

    #[test]
    fn normalized_content_preserves_symbols_as_tokens() {
        assert_eq!(
            normalized_content_tokens("a+b = c < d > $5"),
            vec![
                "a".to_string(),
                "+".to_string(),
                "b".to_string(),
                "=".to_string(),
                "c".to_string(),
                "<".to_string(),
                "d".to_string(),
                ">".to_string(),
                "$".to_string(),
                "5".to_string(),
            ]
        );
    }
}
