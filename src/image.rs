//! Render a story map as an SVG or PNG image.
//!
//! The image follows the look of the HTML output.
//! The text is measured with real fonts, so that each column is as wide as its longest card.

use std::sync::Arc;

pub use resvg::usvg::fontdb;
use resvg::usvg::{self, Tree};

use crate::StoryMap;

// The sizes follow the CSS of the HTML output, in CSS pixels
const FONT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = 20.0;
// The distance from the top of a line to the baseline of the text
const BASELINE: f32 = 15.0;
const PAGE_PADDING: f32 = 16.0;
const CARD_MARGIN: f32 = 8.0;
const CARD_BORDER: f32 = 2.0;
const CARD_PADDING_X: f32 = 8.0;
const CARD_PADDING_Y: f32 = 4.0;
const CARD_MIN_WIDTH: f32 = 120.0;
const CARD_HEIGHT: f32 = LINE_HEIGHT + 2.0 * (CARD_PADDING_Y + CARD_BORDER);
const ROW_HEIGHT: f32 = CARD_HEIGHT + 2.0 * CARD_MARGIN;

/// How to render the image
#[derive(Debug, Clone)]
pub struct ImageOptions {
    /// The fonts to measure and draw the text with
    pub fonts: Arc<fontdb::Database>,
    /// The size of the PNG image, where 1.0 is one pixel per CSS pixel
    pub scale: f32,
}

impl Default for ImageOptions {
    /// The fonts of the OS, at a scale of 1.0
    fn default() -> Self {
        let mut fonts = fontdb::Database::new();
        fonts.load_system_fonts();
        Self {
            fonts: Arc::new(fonts),
            scale: 1.0,
        }
    }
}

/// Render the map as an SVG document
pub fn render_svg(map: &StoryMap, options: &ImageOptions) -> String {
    let texts: Vec<&str> = map
        .cells
        .iter()
        .map(|cell| cell.card.text.as_str())
        .collect();
    let text_widths = measure(&texts, options);
    let mut column_widths = vec![0.0_f32; map.column_count];
    for (cell, text_width) in map.cells.iter().zip(&text_widths) {
        let card_width = (text_width + 2.0 * (CARD_PADDING_X + CARD_BORDER)).max(CARD_MIN_WIDTH);
        let width = &mut column_widths[cell.column];
        *width = width.max(card_width + 2.0 * CARD_MARGIN);
    }
    let column_lefts: Vec<f32> = column_widths
        .iter()
        .scan(PAGE_PADDING, |left, width| {
            let column_left = *left;
            *left += width;
            Some(column_left)
        })
        .collect();
    let first_row_by_level: Vec<usize> = map
        .rows_by_level
        .iter()
        .scan(0, |next_row, rows| {
            let first = *next_row;
            *next_row += rows;
            Some(first)
        })
        .collect();
    let cards: String = map
        .cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let x = column_lefts[cell.column] + CARD_MARGIN;
            let y = PAGE_PADDING
                + (first_row_by_level[cell.level] + cell.row_in_level) as f32 * ROW_HEIGHT
                + CARD_MARGIN;
            let width = column_widths[cell.column] - 2.0 * CARD_MARGIN;
            format!(
                r#"<rect id="card-{index}" x="{x}" y="{y}" width="{width}" height="{CARD_HEIGHT}" fill="none" stroke="black" stroke-width="{CARD_BORDER}"/><text x="{}" y="{}" font-family="sans-serif" font-size="{FONT_SIZE}">{}</text>"#,
                x + CARD_BORDER + CARD_PADDING_X,
                y + CARD_BORDER + CARD_PADDING_Y + BASELINE,
                cell.card.text
            )
        })
        .collect();
    let width = 2.0 * PAGE_PADDING + column_widths.iter().sum::<f32>();
    let height = 2.0 * PAGE_PADDING + map.rows_by_level.iter().sum::<usize>() as f32 * ROW_HEIGHT;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="{width}" height="{height}" fill="white"/>{cards}</svg>"#
    )
}

// The width of each text when it is drawn with the fonts
fn measure(texts: &[&str], options: &ImageOptions) -> Vec<f32> {
    let elements: String = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            format!(r#"<text id="text-{index}" font-family="sans-serif" font-size="{FONT_SIZE}">{text}</text>"#)
        })
        .collect();
    let svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{elements}</svg>"#);
    let mut usvg_options = usvg::Options::default();
    usvg_options.fontdb = options.fonts.clone();
    let tree = Tree::from_str(&svg, &usvg_options).expect("the measuring svg is well formed");
    (0..texts.len())
        .map(|index| {
            tree.node_by_id(&format!("text-{index}"))
                .map_or(0.0, |node| node.abs_bounding_box().width())
        })
        .collect()
}
