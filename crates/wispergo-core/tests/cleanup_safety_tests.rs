use wispergo_core::cleanup_safety::is_safe_punctuation_cleanup;

#[test]
fn accepts_english_punctuation_and_capitalization_only() {
    assert!(is_safe_punctuation_cleanup(
        "can you send the updated notes before the meeting starts",
        "Can you send the updated notes before the meeting starts?",
    ));
}

#[test]
fn accepts_chinese_punctuation_only() {
    assert!(is_safe_punctuation_cleanup(
        "你明天可以帮我检查这个离线版本吗",
        "你明天可以帮我检查这个离线版本吗？",
    ));
}

#[test]
fn accepts_mixed_language_when_content_is_preserved() {
    assert!(is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind 小王 to review the offline build tonight.",
    ));
}

#[test]
fn rejects_chinese_translation_to_english() {
    assert!(!is_safe_punctuation_cleanup(
        "你明天可以帮我检查这个离线版本吗",
        "Can you check this offline version for me tomorrow?",
    ));
}

#[test]
fn rejects_removed_cjk_character_in_mixed_text() {
    assert!(!is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind 王 to review the offline build tonight.",
    ));
}

#[test]
fn rejects_romanized_cjk_name() {
    assert!(!is_safe_punctuation_cleanup(
        "please remind 小王 to review the offline build tonight",
        "Please remind Xiao Wang to review the offline build tonight.",
    ));
}

#[test]
fn rejects_added_latin_word() {
    assert!(!is_safe_punctuation_cleanup(
        "today we reviewed the release checklist",
        "Today, we reviewed our release checklist.",
    ));
}

#[test]
fn rejects_removed_latin_word() {
    assert!(!is_safe_punctuation_cleanup(
        "today we reviewed the release checklist",
        "Today, we reviewed the checklist.",
    ));
}

#[test]
fn rejects_empty_raw() {
    assert!(!is_safe_punctuation_cleanup("", "Hello, world.",));
}

#[test]
fn rejects_empty_candidate() {
    assert!(!is_safe_punctuation_cleanup("hello world", ""));
}

#[test]
fn rejects_latin_word_merge() {
    assert!(!is_safe_punctuation_cleanup("hello world", "helloworld"));
}

#[test]
fn rejects_latin_word_split() {
    assert!(!is_safe_punctuation_cleanup("helloworld", "hello world"));
}

#[test]
fn rejects_dropped_currency_symbol() {
    assert!(!is_safe_punctuation_cleanup("$5 is due", "5 is due"));
}

#[test]
fn rejects_dropped_math_symbol() {
    assert!(!is_safe_punctuation_cleanup("a+b equals c", "ab equals c",));
}
