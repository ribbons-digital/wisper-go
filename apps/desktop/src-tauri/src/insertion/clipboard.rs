#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertionResult {
    Inserted,
    CopiedOnly,
}

pub trait Clipboard {
    fn set_text(&self, text: &str) -> Result<(), String>;
}

pub fn insert_via_clipboard<C: Clipboard>(
    clipboard: &C,
    text: &str,
) -> Result<InsertionResult, String> {
    clipboard.set_text(text)?;
    Ok(InsertionResult::CopiedOnly)
}

#[cfg(test)]
mod tests {
    use super::{insert_via_clipboard, Clipboard, InsertionResult};

    struct FakeClipboard;

    impl Clipboard for FakeClipboard {
        fn set_text(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn failed_native_paste_still_copies_text() {
        let result = insert_via_clipboard(&FakeClipboard, "hello").expect("insert");

        assert_eq!(result, InsertionResult::CopiedOnly);
    }
}
