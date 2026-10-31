//! Render inline Markdown (emphasis, links, code, and so on) as HTML.
//!
//! The output follows markdown-it with its default options, which the extension used before:
//! raw HTML is escaped, and `<br>` and `<img>` have no closing slash.

use comrak::nodes::{NodeLink, NodeValue};
use comrak::{Arena, Node, Options, parse_document};

use crate::escape::escape_markup as escape_html;

/// Render one piece of inline Markdown as HTML. Block syntax such as "# " or "1. " at the start stays text
pub fn inline_html(text: &str) -> String {
    let after_leading = text.trim_start();
    let leading = &text[..text.len() - after_leading.len()];
    let body = after_leading.trim_end();
    let trailing = &after_leading[body.len()..];
    format!("{leading}{}{trailing}", render_body(body))
}

fn render_body(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }
    // Comrak reads a whole document. When the text starts with block syntax, a backslash escape keeps it text
    render_as_paragraph(body)
        .or_else(|| {
            with_escaped_block_start(body).and_then(|escaped| render_as_paragraph(&escaped))
        })
        .unwrap_or_else(|| escape_html(body))
}

fn render_as_paragraph(body: &str) -> Option<String> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    // markdown-it strikes out only "~~text~~". With subscript on, comrak reads "~text~" as subscript,
    // and the subscript is rendered back as plain tildes
    options.extension.subscript = true;
    let root = parse_document(&arena, body, &options);
    let mut blocks = root.children();
    let paragraph = blocks.next()?;
    if blocks.next().is_some() || !matches!(paragraph.data().value, NodeValue::Paragraph) {
        return None;
    }
    let mut html = String::new();
    render_children(paragraph, &mut html);
    Some(html)
}

// Put a backslash in front of the first punctuation, such as "#" in "# Title" or "." in "1. Item"
fn with_escaped_block_start(body: &str) -> Option<String> {
    let index = body.find(|c: char| !c.is_ascii_digit())?;
    let next = body[index..].chars().next()?;
    next.is_ascii_punctuation()
        .then(|| format!("{}\\{}", &body[..index], &body[index..]))
}

fn render_children(node: Node<'_>, html: &mut String) {
    for child in node.children() {
        render_node(child, html);
    }
}

fn render_node(node: Node<'_>, html: &mut String) {
    let value = node.data().value.clone();
    match value {
        NodeValue::Text(text) => html.push_str(&escape_html(&text)),
        NodeValue::SoftBreak => html.push('\n'),
        NodeValue::LineBreak => html.push_str("<br>\n"),
        NodeValue::Code(code) => {
            html.push_str("<code>");
            html.push_str(&escape_html(&code.literal));
            html.push_str("</code>");
        }
        // markdown-it does not read raw HTML with its default options. The tag is shown as text
        NodeValue::HtmlInline(raw) => html.push_str(&escape_html(&raw)),
        NodeValue::Emph => wrap(node, "em", html),
        NodeValue::Strong => wrap(node, "strong", html),
        NodeValue::Strikethrough => wrap(node, "s", html),
        NodeValue::Subscript => {
            html.push('~');
            render_children(node, html);
            html.push('~');
        }
        NodeValue::Link(link) => render_link(node, &link, html),
        NodeValue::Image(image) => render_image(node, &image, html),
        _ => render_children(node, html),
    }
}

fn wrap(node: Node<'_>, tag: &str, html: &mut String) {
    html.push_str(&format!("<{tag}>"));
    render_children(node, html);
    html.push_str(&format!("</{tag}>"));
}

fn render_link(node: Node<'_>, link: &NodeLink, html: &mut String) {
    if !is_safe_url(&link.url) {
        // markdown-it does not make a link to an unsafe URL. The source stays as text
        if is_autolink(node, link) {
            html.push_str(&format!("&lt;{}&gt;", escape_html(&link.url)));
        } else {
            html.push('[');
            render_children(node, html);
            html.push_str(&format!("]({})", escape_html(&link.url)));
        }
        return;
    }
    html.push_str(&format!(
        r#"<a href="{}""#,
        escape_html(&normalized_url(&link.url))
    ));
    push_title(&link.title, html);
    html.push('>');
    render_children(node, html);
    html.push_str("</a>");
}

// An autolink such as <https://example.com> shows its URL as the link text
fn is_autolink(node: Node<'_>, link: &NodeLink) -> bool {
    let text = plain_text(node);
    link.url == text || link.url == format!("mailto:{text}")
}

fn render_image(node: Node<'_>, image: &NodeLink, html: &mut String) {
    let alt = plain_text(node);
    if !is_safe_url(&image.url) {
        html.push_str(&format!(
            "![{}]({})",
            escape_html(&alt),
            escape_html(&image.url)
        ));
        return;
    }
    html.push_str(&format!(
        r#"<img src="{}" alt="{}""#,
        escape_html(&normalized_url(&image.url)),
        escape_html(&alt)
    ));
    push_title(&image.title, html);
    html.push('>');
}

fn push_title(title: &str, html: &mut String) {
    if !title.is_empty() {
        html.push_str(&format!(r#" title="{}""#, escape_html(title)));
    }
}

// The text of the node without markup, such as the alt text of an image
fn plain_text(node: Node<'_>) -> String {
    let mut text = String::new();
    for child in node.children() {
        match &child.data().value {
            NodeValue::Text(value) => text.push_str(value),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::HtmlInline(raw) => text.push_str(raw),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push('\n'),
            _ => text.push_str(&plain_text(child)),
        }
    }
    text
}

// The same check as markdown-it: no script or file URLs, and data URLs only for images
fn is_safe_url(url: &str) -> bool {
    let url = url.trim().to_lowercase();
    let is_bad_protocol = ["vbscript:", "javascript:", "file:", "data:"]
        .iter()
        .any(|protocol| url.starts_with(protocol));
    !is_bad_protocol
        || [
            "data:image/gif;",
            "data:image/png;",
            "data:image/jpeg;",
            "data:image/webp;",
        ]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

// Percent-encode the characters that a URL cannot hold, in the same way as markdown-it (mdurl).
// Existing "%XX" escapes are kept
fn normalized_url(url: &str) -> String {
    const KEEP: &str = ";/?:@&=+$,-_.!~*'()#";
    let bytes = url.as_bytes();
    let mut encoded = String::new();
    for (index, c) in url.char_indices() {
        let is_escape = c == '%'
            && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit);
        if c.is_ascii_alphanumeric() || KEEP.contains(c) || is_escape {
            encoded.push(c);
        } else {
            let mut buffer = [0; 4];
            for byte in c.encode_utf8(&mut buffer).bytes() {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}
