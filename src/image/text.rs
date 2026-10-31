//! Texts on the canvas, and their widths measured with the fonts

use comrak::nodes::NodeValue;
use comrak::{Arena, Node, Options, parse_document};
use resvg::usvg::{self, Tree};

use super::ImageOptions;
use crate::escape::escape_markup;

// The image has one line per text, so a line break becomes a space
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A piece of text in one font
#[derive(Debug, Clone, Default)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
}

/// A text to measure and draw, with its font
#[derive(Debug, Clone)]
pub(super) struct TextSpec {
    runs: Vec<Run>,
    font_size: f32,
    bold: bool,
}

impl TextSpec {
    /// Plain text, shown as it is
    pub(super) fn new(text: &str, font_size: f32) -> Self {
        Self {
            runs: vec![Run {
                text: text.to_string(),
                ..Run::default()
            }],
            font_size,
            bold: false,
        }
    }

    /// A bold heading, shown as it is
    pub(super) fn bold(text: &str, font_size: f32) -> Self {
        Self {
            bold: true,
            ..Self::new(text, font_size)
        }
    }

    /// Inline Markdown. Bold, italic, and code get their fonts. Other markup, such as links, becomes plain text
    pub(super) fn markdown(prefix: &str, markdown: &str, font_size: f32) -> Self {
        let mut runs = vec![Run {
            text: prefix.to_string(),
            ..Run::default()
        }];
        runs.extend(runs_of(markdown));
        Self {
            runs,
            font_size,
            bold: false,
        }
    }

    /// The pieces that a line may break between: words, spaces, and single CJK characters.
    /// Each piece has one run. Spaces in a row become one space
    pub(super) fn pieces(&self) -> Vec<TextSpec> {
        self.runs
            .iter()
            .flat_map(split_run)
            .map(|run| Self {
                runs: vec![run],
                font_size: self.font_size,
                bold: self.bold,
            })
            .collect()
    }

    /// Each character as a text of its own, in the font of its run
    pub(super) fn characters(&self) -> Vec<TextSpec> {
        self.runs
            .iter()
            .flat_map(|run| {
                run.text.chars().map(|ch| Run {
                    text: ch.to_string(),
                    ..run.clone()
                })
            })
            .map(|run| Self {
                runs: vec![run],
                font_size: self.font_size,
                bold: self.bold,
            })
            .collect()
    }

    pub(super) fn is_space(&self) -> bool {
        self.runs.iter().all(|run| run.text == " ")
    }

    /// The pieces of one line as one text. Neighboring runs of the same font become one run
    pub(super) fn joined(pieces: &[TextSpec]) -> TextSpec {
        let mut runs: Vec<Run> = Vec::new();
        for run in pieces.iter().flat_map(|piece| &piece.runs) {
            match runs.last_mut() {
                Some(last) if last.same_font(run) => last.text.push_str(&run.text),
                _ => runs.push(run.clone()),
            }
        }
        Self {
            runs,
            font_size: pieces.first().map_or(0.0, |piece| piece.font_size),
            bold: pieces.first().is_some_and(|piece| piece.bold),
        }
    }

    // A `<text>` element with its baseline at (x, y)
    pub(super) fn to_svg(&self, x: f32, y: f32, extra: &str) -> String {
        let weight = if self.bold {
            r#" font-weight="bold""#
        } else {
            ""
        };
        let spans: String = self.runs.iter().map(run_to_svg).collect();
        format!(
            r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{}"{weight}{extra}>{spans}</text>"#,
            self.font_size
        )
    }
}

impl Run {
    fn same_font(&self, other: &Run) -> bool {
        (self.bold, self.italic, self.code) == (other.bold, other.italic, other.code)
    }
}

fn split_run(run: &Run) -> Vec<Run> {
    let piece = |text: &str| Run {
        text: text.to_string(),
        ..run.clone()
    };
    let mut pieces = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, pieces: &mut Vec<Run>| {
        if !word.is_empty() {
            pieces.push(piece(word));
            word.clear();
        }
    };
    for ch in run.text.chars() {
        if ch.is_whitespace() {
            flush(&mut word, &mut pieces);
            if pieces.last().is_none_or(|last| last.text != " ") {
                pieces.push(piece(" "));
            }
        } else if is_cjk(ch) {
            flush(&mut word, &mut pieces);
            pieces.push(piece(&ch.to_string()));
        } else {
            word.push(ch);
        }
    }
    flush(&mut word, &mut pieces);
    pieces
}

// Chinese, Japanese, and Korean text has no spaces, so a line may break between any two of these characters
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{3000}'..='\u{30FF}' // CJK symbols, Hiragana, and Katakana
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs
        | '\u{AC00}'..='\u{D7AF}' // Hangul syllables
        | '\u{FF00}'..='\u{FFEF}' // Fullwidth forms
    )
}

fn run_to_svg(run: &Run) -> String {
    let mut attributes = String::new();
    if run.bold {
        attributes.push_str(r#" font-weight="bold""#);
    }
    if run.italic {
        attributes.push_str(r#" font-style="italic""#);
    }
    if run.code {
        attributes.push_str(r#" font-family="monospace""#);
    }
    let text = escape_markup(&run.text);
    if attributes.is_empty() {
        text
    } else {
        format!("<tspan{attributes}>{text}</tspan>")
    }
}

fn runs_of(markdown: &str) -> Vec<Run> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &Options::default());
    let mut runs = Vec::new();
    match root.first_child() {
        Some(paragraph) if matches!(paragraph.data().value, NodeValue::Paragraph) => {
            collect_runs(paragraph, &Run::default(), &mut runs);
        }
        _ => runs.push(Run {
            text: one_line(markdown),
            ..Run::default()
        }),
    }
    runs
}

fn collect_runs(node: Node<'_>, style: &Run, runs: &mut Vec<Run>) {
    for child in node.children() {
        let value = child.data().value.clone();
        match value {
            NodeValue::Text(text) => runs.push(Run {
                text: text.to_string(),
                ..style.clone()
            }),
            NodeValue::Code(code) => runs.push(Run {
                text: code.literal,
                code: true,
                ..style.clone()
            }),
            NodeValue::SoftBreak | NodeValue::LineBreak => runs.push(Run {
                text: " ".to_string(),
                ..style.clone()
            }),
            NodeValue::Strong => collect_runs(
                child,
                &Run {
                    bold: true,
                    ..style.clone()
                },
                runs,
            ),
            NodeValue::Emph => collect_runs(
                child,
                &Run {
                    italic: true,
                    ..style.clone()
                },
                runs,
            ),
            _ => collect_runs(child, style, runs),
        }
    }
}

// The width of each text when it is drawn with the fonts
pub(super) fn measure(texts: &[TextSpec], options: &ImageOptions) -> Vec<f32> {
    let elements: String = texts
        .iter()
        .enumerate()
        .map(|(index, text)| text.to_svg(0.0, 0.0, &format!(r#" id="text-{index}""#)))
        .collect();
    let svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{elements}</svg>"#);
    let usvg_options = usvg::Options {
        fontdb: options.fonts.clone(),
        ..usvg::Options::default()
    };
    let tree = Tree::from_str(&svg, &usvg_options).expect("the measuring svg is well formed");
    (0..texts.len())
        .map(|index| {
            tree.node_by_id(&format!("text-{index}"))
                .map_or(0.0, |node| node.abs_bounding_box().width())
        })
        .collect()
}
