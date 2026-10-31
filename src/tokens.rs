//! A flat token stream of the Markdown block structure.
//!
//! The layout rules were first written against the token stream of markdown-it.
//! Comrak builds a tree instead, so this module flattens the tree into the same kind of stream.
//! Only the tokens that the layout looks at are produced.

use comrak::nodes::{ListType, NodeValue, Sourcepos};
use comrak::{Arena, Node, Options, parse_document};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    HeadingOpen,
    HeadingClose,
    BulletListOpen,
    BulletListClose,
    OrderedListOpen,
    OrderedListClose,
    ListItemOpen,
    ParagraphOpen,
    /// The raw Markdown text of a heading, a paragraph, or a table cell
    Inline(String),
}

/// Parse a Markdown document into the token stream
pub(crate) fn tokenize(markdown: &str) -> Vec<Token> {
    let arena = Arena::new();
    let mut options = Options::default();
    // A table after the outline is not a paragraph, so it must not turn into a note
    options.extension.table = true;
    let root = parse_document(&arena, markdown, &options);
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut tokens = Vec::new();
    push_children(root, &lines, 0, &mut tokens);
    tokens
}

fn push_children(node: Node<'_>, lines: &[&str], quote_depth: usize, tokens: &mut Vec<Token>) {
    for child in node.children() {
        push_node(child, lines, quote_depth, tokens);
    }
}

fn push_node(node: Node<'_>, lines: &[&str], quote_depth: usize, tokens: &mut Vec<Token>) {
    let (value, sourcepos) = {
        let ast = node.data();
        (ast.value.clone(), ast.sourcepos)
    };
    match value {
        NodeValue::Heading(heading) => {
            let content = if heading.setext {
                setext_heading_content(lines, sourcepos, quote_depth)
            } else {
                atx_heading_content(line_from(lines, sourcepos))
            };
            tokens.extend([
                Token::HeadingOpen,
                Token::Inline(content),
                Token::HeadingClose,
            ]);
        }
        NodeValue::List(list) => {
            let (open, close) = match list.list_type {
                ListType::Bullet => (Token::BulletListOpen, Token::BulletListClose),
                ListType::Ordered => (Token::OrderedListOpen, Token::OrderedListClose),
            };
            tokens.push(open);
            push_children(node, lines, quote_depth, tokens);
            tokens.push(close);
        }
        NodeValue::Item(_) => {
            tokens.push(Token::ListItemOpen);
            push_children(node, lines, quote_depth, tokens);
        }
        NodeValue::Paragraph => {
            tokens.push(Token::ParagraphOpen);
            tokens.push(Token::Inline(block_content(
                lines,
                sourcepos,
                quote_depth,
                sourcepos.end.line,
            )));
        }
        NodeValue::BlockQuote => push_children(node, lines, quote_depth + 1, tokens),
        NodeValue::Table(_) => {
            for row in node.children() {
                for cell in row.children() {
                    let cell_pos = cell.data().sourcepos;
                    tokens.push(Token::Inline(cell_content(lines, cell_pos)));
                }
            }
        }
        _ => {}
    }
}

/// The text of the first line of a block, from the first column of the block
fn line_from<'a>(lines: &[&'a str], sourcepos: Sourcepos) -> &'a str {
    let line = lines.get(sourcepos.start.line - 1).copied().unwrap_or("");
    line.get(sourcepos.start.column - 1..).unwrap_or("")
}

/// The raw text of a block from its first line up to `last_line`.
/// The indentation and the block quote markers in front of the following lines are removed
fn block_content(
    lines: &[&str],
    sourcepos: Sourcepos,
    quote_depth: usize,
    last_line: usize,
) -> String {
    let mut content = String::from(line_from(lines, sourcepos));
    for line in lines.iter().take(last_line).skip(sourcepos.start.line) {
        content.push('\n');
        content.push_str(without_quote_markers(line, quote_depth));
    }
    content.trim().to_string()
}

fn without_quote_markers(line: &str, quote_depth: usize) -> &str {
    let mut rest = line.trim_start();
    for _ in 0..quote_depth {
        match rest.strip_prefix('>') {
            Some(after) => rest = after.trim_start(),
            None => break,
        }
    }
    rest
}

/// The content of an ATX heading such as "## Title ##", in the same way as markdown-it reads it:
/// without the opening "#" run, and without a closing "#" run that follows a space
fn atx_heading_content(line: &str) -> String {
    let after_hashes = line.trim_start_matches('#');
    let rest = after_hashes.trim_end_matches([' ', '\t']);
    let without_closing = rest.trim_end_matches('#');
    let content = if !without_closing.is_empty() && without_closing.ends_with([' ', '\t']) {
        without_closing
    } else {
        rest
    };
    content.trim().to_string()
}

/// The content of a setext heading: the lines above the "===" or "---" underline
fn setext_heading_content(lines: &[&str], sourcepos: Sourcepos, quote_depth: usize) -> String {
    block_content(lines, sourcepos, quote_depth, sourcepos.end.line - 1)
}

fn cell_content(lines: &[&str], sourcepos: Sourcepos) -> String {
    let line = lines.get(sourcepos.start.line - 1).copied().unwrap_or("");
    line.get(sourcepos.start.column - 1..sourcepos.end.column)
        .unwrap_or("")
        .trim()
        .trim_matches('|')
        .trim()
        .to_string()
}
