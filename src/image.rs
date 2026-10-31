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
// The map title is a bold heading, like `<h1>`
const MAP_TITLE_FONT_SIZE: f32 = FONT_SIZE * 2.0;
const MAP_TITLE_LINE_HEIGHT: f32 = LINE_HEIGHT * 2.0;
const MAP_TITLE_BASELINE: f32 = BASELINE * 2.0;
const MAP_TITLE_MARGIN: f32 = MAP_TITLE_FONT_SIZE * 0.67;
// A note is a paragraph, like `<p>`
const NOTE_MARGIN: f32 = FONT_SIZE;
// A level title is gray text in the first column
const LEVEL_TITLE_COLOR: &str = "#888888";
const LEVEL_TITLE_MARGIN: f32 = 8.0;
const LEVEL_TITLE_PADDING_X: f32 = 8.0;
const LEVEL_TITLE_PADDING_Y: f32 = 4.0;
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
    let level_titles: Vec<TextSpec> = map
        .level_titles
        .iter()
        .map(|title| TextSpec::new(title, FONT_SIZE))
        .collect();
    let level_title_widths = measure(&level_titles, options);
    // The blocks are stacked from the top, like the HTML page: the title, the notes above, the grid, and the notes below
    let mut flow = Flow::new(PAGE_PADDING);
    let mut texts = Vec::new();
    if !map.title.is_empty() {
        let top = flow.place(MAP_TITLE_LINE_HEIGHT, MAP_TITLE_MARGIN);
        texts.push((
            TextSpec::bold(&map.title, MAP_TITLE_FONT_SIZE),
            top + MAP_TITLE_BASELINE,
        ));
    }
    for note in &map.leading_notes {
        let top = flow.place(LINE_HEIGHT, NOTE_MARGIN);
        texts.push((TextSpec::new(&one_line(note), FONT_SIZE), top + BASELINE));
    }
    let grid_top = flow.place(0.0, 0.0);
    let grid = Grid::new(map, &contents, &level_title_widths, grid_top);
    flow.place(grid.height, 0.0);
    for note in &map.trailing_notes {
        let top = flow.place(LINE_HEIGHT, NOTE_MARGIN);
        texts.push((TextSpec::new(&one_line(note), FONT_SIZE), top + BASELINE));
    }
    let specs: Vec<TextSpec> = texts
        .iter()
        .map(|(spec, _)| TextSpec {
            text: spec.text.clone(),
            ..*spec
        })
        .collect();
    let content_width = measure(&specs, options)
        .into_iter()
        .fold(grid.width, f32::max);
    let width = 2.0 * PAGE_PADDING + content_width;
    let height = flow.end() + PAGE_PADDING;
    let notes: String = texts
        .iter()
        .map(|(spec, baseline)| spec.to_svg(PAGE_PADDING, *baseline, ""))
        .collect();
    let cards: String = map
        .cells
        .iter()
        .zip(&contents)
        .enumerate()
        .map(|(index, (cell, content))| render_card(index, cell, content, &grid))
        .collect();
    let bands = render_bands(map, &grid);
    let level_titles = render_level_titles(map, &grid);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><defs>{SHADOW_FILTER}</defs><rect width="{width}" height="{height}" fill="white"/>{notes}{bands}{level_titles}{cards}</svg>"#
    )
}

// HTML shows a line break inside a paragraph as a space
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Blocks stacked from the top. Like CSS, the margins between two blocks collapse into the larger one
struct Flow {
    bottom: f32,
    pending_margin: f32,
}

impl Flow {
    fn new(top: f32) -> Self {
        Self {
            bottom: top,
            pending_margin: 0.0,
        }
    }

    // Put a block with the same margin above and below it, and return its top
    fn place(&mut self, height: f32, margin: f32) -> f32 {
        let top = self.bottom + self.pending_margin.max(margin);
        self.bottom = top + height;
        self.pending_margin = margin;
        top
    }

    fn end(&self) -> f32 {
        self.bottom + self.pending_margin
    }
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
            texts.push(TextSpec::new(
                &format!("{}{}", checkbox_emoji(&cell.card), cell.card.text),
                FONT_SIZE,
            ));
            texts.extend(
                cell.card
                    .tags
                    .iter()
                    .map(|tag| TextSpec::new(tag, TAG_FONT_SIZE)),
            );
        }
        let mut widths = measure(&texts, options).into_iter();
        let mut texts = texts.into_iter().map(|text| text.text);
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
                r#"<text x="{}" y="{}" font-family="sans-serif" font-size="{FONT_SIZE}" fill="{LEVEL_TITLE_COLOR}">{title}</text>"#,
                PAGE_PADDING + LEVEL_TITLE_MARGIN + LEVEL_TITLE_PADDING_X,
                grid.row_tops[*row] + LEVEL_TITLE_MARGIN + LEVEL_TITLE_PADDING_Y + BASELINE
            )
        })
        .collect()
}

// One band per level behind the cards, with a dashed line at the bottom
fn render_bands(map: &StoryMap, grid: &Grid) -> String {
    let left = PAGE_PADDING;
    let width = grid.width;
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
    // The size of the grid itself, without the padding of the page
    width: f32,
    height: f32,
}

impl Grid {
    // Like CSS Grid, each column is as wide as its widest card
    fn new(map: &StoryMap, contents: &[CardContent], level_title_widths: &[f32], top: f32) -> Self {
        let first_card_column = usize::from(!map.level_titles.is_empty());
        let mut column_widths = vec![0.0_f32; first_card_column + map.column_count];
        if first_card_column == 1 {
            column_widths[0] = level_title_widths
                .iter()
                .map(|width| width + 2.0 * (LEVEL_TITLE_PADDING_X + LEVEL_TITLE_MARGIN))
                .fold(0.0, f32::max);
        }
        for (cell, content) in map.cells.iter().zip(contents) {
            let card_width =
                (content.width() + 2.0 * (CARD_PADDING_X + CARD_BORDER)).max(CARD_MIN_WIDTH);
            let width = &mut column_widths[first_card_column + cell.column];
            *width = width.max(card_width + 2.0 * CARD_MARGIN);
        }
        let first_row_by_level = starts_of(&map.rows_by_level, 0);
        // Like CSS Grid, a row without cards or titles has no height. Only the dashed line of the band shows
        let row_count: usize = map.rows_by_level.iter().sum();
        let mut row_heights = vec![BAND_BORDER; row_count];
        for cell in &map.cells {
            row_heights[first_row_by_level[cell.level] + cell.row_in_level] = ROW_HEIGHT;
        }
        for (title, row) in map.level_titles.iter().zip(&first_row_by_level) {
            if !title.is_empty() {
                row_heights[*row] = ROW_HEIGHT;
            }
        }
        Self {
            column_lefts: starts_of(&column_widths, PAGE_PADDING),
            row_tops: starts_of(&row_heights, top),
            first_row_by_level,
            width: column_widths.iter().sum::<f32>(),
            height: row_heights.iter().sum::<f32>(),
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

/// A text to measure and draw, with its font
struct TextSpec {
    text: String,
    font_size: f32,
    bold: bool,
}

impl TextSpec {
    fn new(text: &str, font_size: f32) -> Self {
        Self {
            text: text.to_string(),
            font_size,
            bold: false,
        }
    }

    fn bold(text: &str, font_size: f32) -> Self {
        Self {
            bold: true,
            ..Self::new(text, font_size)
        }
    }

    // A `<text>` element with its baseline at (x, y)
    fn to_svg(&self, x: f32, y: f32, extra: &str) -> String {
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
fn measure(texts: &[TextSpec], options: &ImageOptions) -> Vec<f32> {
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
