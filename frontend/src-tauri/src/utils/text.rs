pub(crate) fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
