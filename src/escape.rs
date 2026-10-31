//! Escape the characters that have a meaning in HTML and XML

/// Replace `&`, `<`, `>`, and `"` with their character references
pub fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
