//! Texts on the canvas, and their widths measured with the fonts

use resvg::usvg::{self, Tree};

use super::ImageOptions;

// HTML shows a line break inside a paragraph as a space
pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A text to measure and draw, with its font
#[derive(Debug, Clone)]
pub(super) struct TextSpec {
    pub(super) text: String,
    pub(super) font_size: f32,
    pub(super) bold: bool,
}

impl TextSpec {
    pub(super) fn new(text: &str, font_size: f32) -> Self {
        Self {
            text: text.to_string(),
            font_size,
            bold: false,
        }
    }

    pub(super) fn bold(text: &str, font_size: f32) -> Self {
        Self {
            bold: true,
            ..Self::new(text, font_size)
        }
    }

    // A `<text>` element with its baseline at (x, y)
    pub(super) fn to_svg(&self, x: f32, y: f32, extra: &str) -> String {
        let weight = if self.bold {
            r#" font-weight="bold""#
        } else {
            ""
        };
        format!(
            r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{}"{weight}{extra}>{}</text>"#,
            self.font_size, self.text
        )
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
