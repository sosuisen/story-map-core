//! Render a story map as an SVG or PNG image.
//!
//! The image follows the look of the HTML output.
//! The text is measured with real fonts, so that each column is as wide as its longest card.

use std::sync::Arc;

pub use resvg::usvg::fontdb;
use resvg::usvg::{self, Tree};

use crate::StoryMap;
use crate::style::{BORDER_SHADE, CARD_SHADE, level_color, shaded};

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
const BAND_BORDER: f32 = 2.0;

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
    let grid = Grid::new(map, &measure(&texts, options));
    let cards: String = map
        .cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let x = grid.column_lefts[cell.column] + CARD_MARGIN;
            let y = grid.row_tops[grid.first_row_by_level[cell.level] + cell.row_in_level] + CARD_MARGIN;
            let width = grid.column_widths[cell.column] - 2.0 * CARD_MARGIN;
            let base = level_color(cell.level);
            format!(
                r#"<rect id="card-{index}" x="{x}" y="{y}" width="{width}" height="{CARD_HEIGHT}" fill="{}" stroke="{}" stroke-width="{CARD_BORDER}"/><text x="{}" y="{}" font-family="sans-serif" font-size="{FONT_SIZE}">{}</text>"#,
                shaded(base, CARD_SHADE),
                shaded(base, BORDER_SHADE),
                x + CARD_BORDER + CARD_PADDING_X,
                y + CARD_BORDER + CARD_PADDING_Y + BASELINE,
                cell.card.text
            )
        })
        .collect();
    let bands = render_bands(map, &grid);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{1}"><rect width="{0}" height="{1}" fill="white"/>{bands}{cards}</svg>"#,
        grid.width, grid.height
    )
}

// One band per level behind the cards, with a dashed line at the bottom
fn render_bands(map: &StoryMap, grid: &Grid) -> String {
    let left = PAGE_PADDING;
    let width = grid.width - 2.0 * PAGE_PADDING;
    map.rows_by_level
        .iter()
        .enumerate()
        .map(|(level, rows)| {
            let first_row = grid.first_row_by_level[level];
            let top = grid.row_tops[first_row];
            let height: f32 = grid.row_heights[first_row..first_row + rows].iter().sum();
            let base = level_color(level);
            let line_y = top + height - BAND_BORDER / 2.0;
            format!(
                r#"<rect id="band-{level}" x="{left}" y="{top}" width="{width}" height="{height}" fill="{base}"/><line x1="{left}" y1="{line_y}" x2="{}" y2="{line_y}" stroke="{}" stroke-width="{BAND_BORDER}" stroke-dasharray="6 6"/>"#,
                left + width,
                shaded(base, CARD_SHADE)
            )
        })
        .collect()
}

/// The positions of the grid tracks on the canvas
struct Grid {
    column_lefts: Vec<f32>,
    column_widths: Vec<f32>,
    row_tops: Vec<f32>,
    row_heights: Vec<f32>,
    first_row_by_level: Vec<usize>,
    width: f32,
    height: f32,
}

impl Grid {
    // Like CSS Grid, each column is as wide as its widest card
    fn new(map: &StoryMap, text_widths: &[f32]) -> Self {
        let mut column_widths = vec![0.0_f32; map.column_count];
        for (cell, text_width) in map.cells.iter().zip(text_widths) {
            let card_width =
                (text_width + 2.0 * (CARD_PADDING_X + CARD_BORDER)).max(CARD_MIN_WIDTH);
            let width = &mut column_widths[cell.column];
            *width = width.max(card_width + 2.0 * CARD_MARGIN);
        }
        let row_count: usize = map.rows_by_level.iter().sum();
        let row_heights = vec![ROW_HEIGHT; row_count];
        Self {
            column_lefts: starts_of(&column_widths, PAGE_PADDING),
            row_tops: starts_of(&row_heights, PAGE_PADDING),
            first_row_by_level: starts_of(&map.rows_by_level, 0),
            width: 2.0 * PAGE_PADDING + column_widths.iter().sum::<f32>(),
            height: 2.0 * PAGE_PADDING + row_heights.iter().sum::<f32>(),
            column_widths,
            row_heights,
        }
    }
}

// The start of each track when the tracks are put one after another from `origin`
fn starts_of<T: Copy + std::ops::Add<Output = T>>(sizes: &[T], origin: T) -> Vec<T> {
    sizes
        .iter()
        .scan(origin, |next, size| {
            let start = *next;
            *next = *next + *size;
            Some(start)
        })
        .collect()
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
