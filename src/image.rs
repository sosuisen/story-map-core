//! Render a story map as an SVG or PNG image.
//!
//! The image follows the look of the HTML output.
//! The text is measured with real fonts, so that each column is as wide as its longest card.

use std::sync::Arc;

pub use resvg::usvg::fontdb;
use resvg::usvg::{self, Tree};

use crate::style::{
    ALT_SHADE, BORDER_SHADE, CARD_SHADE, checkbox_emoji, is_alt_band, level_color, shaded,
};
use crate::{Cell, StoryMap};

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
const CARD_RADIUS: f32 = 6.0;
const CARD_HEIGHT: f32 = LINE_HEIGHT + 2.0 * (CARD_PADDING_Y + CARD_BORDER);
const ROW_HEIGHT: f32 = CARD_HEIGHT + 2.0 * CARD_MARGIN;
const BAND_BORDER: f32 = 2.0;
// A level title is gray text in the first column
const TITLE_COLOR: &str = "#888888";
const TITLE_MARGIN: f32 = 8.0;
const TITLE_PADDING_X: f32 = 8.0;
const TITLE_PADDING_Y: f32 = 4.0;
// A tag is a small white pill after the text
const TAG_FONT_SIZE: f32 = FONT_SIZE * 0.8;
const TAG_MARGIN: f32 = 4.0;
const TAG_PADDING_X: f32 = 6.0;
const TAG_RADIUS: f32 = 8.0;
const TAG_HEIGHT: f32 = TAG_FONT_SIZE * 1.25;
// The distance from the top of the pill to the baseline of the tag text
const TAG_BASELINE: f32 = TAG_FONT_SIZE;

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
    let contents = CardContent::measure_all(map, options);
    let title_texts: Vec<(String, f32)> = map
        .level_titles
        .iter()
        .map(|title| (title.clone(), FONT_SIZE))
        .collect();
    let title_widths = measure(&title_texts, options);
    let grid = Grid::new(map, &contents, &title_widths);
    let cards: String = map
        .cells
        .iter()
        .zip(&contents)
        .enumerate()
        .map(|(index, (cell, content))| render_card(index, cell, content, &grid))
        .collect();
    let bands = render_bands(map, &grid);
    let titles = render_level_titles(map, &grid);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{1}"><defs>{SHADOW_FILTER}</defs><rect width="{0}" height="{1}" fill="white"/>{bands}{titles}{cards}</svg>"#,
        grid.width, grid.height
    )
}

/// The texts inside a card, with their measured widths
struct CardContent {
    // The text of the card with its checkbox as an emoji
    text: String,
    text_width: f32,
    tags: Vec<(String, f32)>,
}

impl CardContent {
    fn measure_all(map: &StoryMap, options: &ImageOptions) -> Vec<Self> {
        let mut texts = Vec::new();
        for cell in &map.cells {
            texts.push((
                format!("{}{}", checkbox_emoji(&cell.card), cell.card.text),
                FONT_SIZE,
            ));
            texts.extend(
                cell.card
                    .tags
                    .iter()
                    .map(|tag| (tag.clone(), TAG_FONT_SIZE)),
            );
        }
        let mut widths = measure(&texts, options).into_iter();
        let mut texts = texts.into_iter().map(|(text, _)| text);
        map.cells
            .iter()
            .map(|cell| Self {
                text: texts.next().unwrap_or_default(),
                text_width: widths.next().unwrap_or_default(),
                tags: cell
                    .card
                    .tags
                    .iter()
                    .map(|_| {
                        (
                            texts.next().unwrap_or_default(),
                            widths.next().unwrap_or_default(),
                        )
                    })
                    .collect(),
            })
            .collect()
    }

    // The width of the text and the tags, without the padding of the card
    fn width(&self) -> f32 {
        self.text_width
            + self
                .tags
                .iter()
                .map(|(_, width)| TAG_MARGIN + width + 2.0 * TAG_PADDING_X)
                .sum::<f32>()
    }
}

// The shadow of an open card, like `box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2)`
const SHADOW_FILTER: &str = r#"<filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="2" dy="2" stdDeviation="2" flood-color="black" flood-opacity="0.2"/></filter>"#;

fn render_card(index: usize, cell: &Cell, content: &CardContent, grid: &Grid) -> String {
    let column = grid.first_card_column + cell.column;
    let left = grid.column_lefts[column] + CARD_MARGIN;
    let top = grid.row_tops[grid.first_row_by_level[cell.level] + cell.row_in_level] + CARD_MARGIN;
    let width = grid.column_widths[column] - 2.0 * CARD_MARGIN;
    let base = level_color(cell.level);
    let fill = shaded(base, CARD_SHADE);
    // A done card has no border, so it is shorter by the border, like in CSS
    let border = if cell.card.done { 0.0 } else { CARD_BORDER };
    let height = CARD_HEIGHT - 2.0 * (CARD_BORDER - border);
    // An SVG stroke is centered on the outline, but a CSS border is inside the box
    let frame = if border > 0.0 {
        format!(
            r#"<rect id="card-{index}" x="{}" y="{}" width="{}" height="{}" rx="{CARD_RADIUS}" fill="{fill}" stroke="{}" stroke-width="{border}"/>"#,
            left + border / 2.0,
            top + border / 2.0,
            width - border,
            height - border,
            shaded(base, BORDER_SHADE)
        )
    } else {
        format!(
            r#"<rect id="card-{index}" x="{left}" y="{top}" width="{width}" height="{height}" rx="{CARD_RADIUS}" fill="{fill}"/>"#
        )
    };
    let frame = if cell.card.todo {
        format!(r#"<g id="card-{index}-shadow" filter="url(#shadow)">{frame}</g>"#)
    } else {
        frame
    };
    let text_left = left + border + CARD_PADDING_X;
    let baseline = top + border + CARD_PADDING_Y + BASELINE;
    let mut svg = format!(
        r#"{frame}<text x="{text_left}" y="{baseline}" font-family="sans-serif" font-size="{FONT_SIZE}">{}</text>"#,
        content.text
    );
    let mut tag_left = text_left + content.text_width + TAG_MARGIN;
    for (tag_index, (tag, tag_width)) in content.tags.iter().enumerate() {
        let tag_width_with_padding = tag_width + 2.0 * TAG_PADDING_X;
        svg.push_str(&format!(
            r#"<rect id="card-{index}-tag-{tag_index}" x="{tag_left}" y="{}" width="{tag_width_with_padding}" height="{TAG_HEIGHT}" rx="{TAG_RADIUS}" fill="white"/><text x="{}" y="{baseline}" font-family="sans-serif" font-size="{TAG_FONT_SIZE}">{tag}</text>"#,
            baseline - TAG_BASELINE,
            tag_left + TAG_PADDING_X
        ));
        tag_left += tag_width_with_padding + TAG_MARGIN;
    }
    svg
}

// Each title sits on the first row of its band. Levels that do not exist, and empty titles, get no title
fn render_level_titles(map: &StoryMap, grid: &Grid) -> String {
    map.level_titles
        .iter()
        .zip(&grid.first_row_by_level)
        .filter(|(title, _)| !title.is_empty())
        .map(|(title, row)| {
            format!(
                r#"<text x="{}" y="{}" font-family="sans-serif" font-size="{FONT_SIZE}" fill="{TITLE_COLOR}">{title}</text>"#,
                PAGE_PADDING + TITLE_MARGIN + TITLE_PADDING_X,
                grid.row_tops[*row] + TITLE_MARGIN + TITLE_PADDING_Y + BASELINE
            )
        })
        .collect()
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
            let fill = if is_alt_band(level) {
                shaded(base, ALT_SHADE)
            } else {
                base
            };
            let line_y = top + height - BAND_BORDER / 2.0;
            format!(
                r#"<rect id="band-{level}" x="{left}" y="{top}" width="{width}" height="{height}" fill="{fill}"/><line x1="{left}" y1="{line_y}" x2="{}" y2="{line_y}" stroke="{}" stroke-width="{BAND_BORDER}" stroke-dasharray="6 6"/>"#,
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
    // With level titles, the first column holds them and the cards start at the second column
    first_card_column: usize,
    row_tops: Vec<f32>,
    row_heights: Vec<f32>,
    first_row_by_level: Vec<usize>,
    width: f32,
    height: f32,
}

impl Grid {
    // Like CSS Grid, each column is as wide as its widest card
    fn new(map: &StoryMap, contents: &[CardContent], title_widths: &[f32]) -> Self {
        let first_card_column = usize::from(!map.level_titles.is_empty());
        let mut column_widths = vec![0.0_f32; first_card_column + map.column_count];
        if first_card_column == 1 {
            column_widths[0] = title_widths
                .iter()
                .map(|width| width + 2.0 * (TITLE_PADDING_X + TITLE_MARGIN))
                .fold(0.0, f32::max);
        }
        for (cell, content) in map.cells.iter().zip(contents) {
            let card_width =
                (content.width() + 2.0 * (CARD_PADDING_X + CARD_BORDER)).max(CARD_MIN_WIDTH);
            let width = &mut column_widths[first_card_column + cell.column];
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
            first_card_column,
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

// The width of each text when it is drawn with the fonts at its font size
fn measure(texts: &[(String, f32)], options: &ImageOptions) -> Vec<f32> {
    let elements: String = texts
        .iter()
        .enumerate()
        .map(|(index, (text, font_size))| {
            format!(r#"<text id="text-{index}" font-family="sans-serif" font-size="{font_size}">{text}</text>"#)
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
