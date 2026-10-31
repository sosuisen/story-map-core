//! Render a story map as an SVG or PNG image.
//!
//! The image follows the look of the HTML output.
//! The text is measured with real fonts, so that each column is as wide as its longest card.

mod layout;
mod text;

use std::sync::Arc;

pub use resvg::usvg::fontdb;
use resvg::{tiny_skia, usvg};

use crate::style::{
    ALT_SHADE, BORDER_SHADE, CARD_SHADE, checkbox_emoji, is_alt_band, level_color, shaded,
};
use crate::{Cell, StoryMap};
use layout::{Flow, Grid};
use text::{TextSpec, measure, measure_advances};

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
const CARD_MAX_WIDTH: f32 = 240.0;
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
        use_installed_generic_fonts(&mut fonts);
        Self {
            fonts: Arc::new(fonts),
            scale: 1.0,
        }
    }
}

// fontdb maps "sans-serif" to Arial and "monospace" to Courier New.
// Linux usually has neither, and then no text is drawn
fn use_installed_generic_fonts(fonts: &mut fontdb::Database) {
    if let Some(family) = installed_family(fonts, SANS_SERIF_FAMILIES, false) {
        fonts.set_sans_serif_family(family);
    }
    if let Some(family) = installed_family(fonts, MONOSPACE_FAMILIES, true) {
        fonts.set_monospace_family(family);
    }
}

const SANS_SERIF_FAMILIES: &[&str] = &[
    "Arial",
    "Helvetica",
    "Liberation Sans",
    "DejaVu Sans",
    "Noto Sans",
];
const MONOSPACE_FAMILIES: &[&str] = &[
    "Courier New",
    "Menlo",
    "Liberation Mono",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
];

// The first of the known families that is installed, or else the first installed family of the kind
fn installed_family(fonts: &fontdb::Database, known: &[&str], monospaced: bool) -> Option<String> {
    let installed = |name: &str| {
        fonts
            .faces()
            .any(|face| face.families.iter().any(|(family, _)| family == name))
    };
    known
        .iter()
        .find(|name| installed(name))
        .map(|name| name.to_string())
        .or_else(|| {
            fonts
                .faces()
                .find(|face| face.monospaced == monospaced)
                .and_then(|face| face.families.first())
                .map(|(family, _)| family.clone())
        })
}

/// Render the map as an SVG document
pub fn render_svg(map: &StoryMap, options: &ImageOptions) -> String {
    let contents = CardContent::measure_all(map, options);
    let level_titles: Vec<TextSpec> = map
        .level_titles
        .iter()
        .map(|title| TextSpec::markdown("", title, FONT_SIZE))
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
        texts.push((TextSpec::markdown("", note, FONT_SIZE), top + BASELINE));
    }
    let grid_top = flow.place(0.0, 0.0);
    let grid = Grid::new(map, &contents, &level_title_widths, grid_top);
    flow.place(grid.height, 0.0);
    for note in &map.trailing_notes {
        let top = flow.place(LINE_HEIGHT, NOTE_MARGIN);
        texts.push((TextSpec::markdown("", note, FONT_SIZE), top + BASELINE));
    }
    let specs: Vec<TextSpec> = texts.iter().map(|(spec, _)| spec.clone()).collect();
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

/// Render the map as a PNG image, `options.scale` times as large as the SVG
///
/// # Panics
///
/// Panics if `options.scale` is not a positive finite number
pub fn render_png(map: &StoryMap, options: &ImageOptions) -> Vec<u8> {
    assert!(
        options.scale.is_finite() && options.scale > 0.0,
        "the scale must be a positive finite number"
    );
    let usvg_options = usvg::Options {
        fontdb: options.fonts.clone(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&render_svg(map, options), &usvg_options)
        .expect("the rendered svg is well formed");
    // Round up after scaling. Rounding first would add up to one pixel per scale step
    let width = (tree.size().width() * options.scale).ceil() as u32;
    let height = (tree.size().height() * options.scale).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("the image is not too large");
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(options.scale, options.scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("a pixmap can be encoded as png")
}

/// One thing on a line of a card
enum Item {
    Text(TextSpec),
    // A tag with the width of its text
    Tag(TextSpec, f32),
}

/// The lines of a card. Each item has its left edge, measured from the start of the line
struct CardContent {
    lines: Vec<Vec<(f32, Item)>>,
    // The width of the widest line
    width: f32,
}

impl CardContent {
    fn measure_all(map: &StoryMap, options: &ImageOptions) -> Vec<Self> {
        // The text of each card with its checkbox as an emoji, cut where a line may break
        let pieces: Vec<Vec<TextSpec>> = map
            .cells
            .iter()
            .map(|cell| {
                TextSpec::markdown(checkbox_emoji(&cell.card), &cell.card.text, FONT_SIZE).pieces()
            })
            .collect();
        let tags: Vec<TextSpec> = map
            .cells
            .iter()
            .flat_map(|cell| &cell.card.tags)
            .map(|tag| TextSpec::new(tag, TAG_FONT_SIZE))
            .collect();
        let mut advances = measure_advances(&pieces.concat(), options).into_iter();
        let mut tag_widths = measure(&tags, options).into_iter();
        let mut tags = tags.into_iter();
        let line_width = CARD_MAX_WIDTH - 2.0 * (CARD_PADDING_X + CARD_BORDER);
        map.cells
            .iter()
            .zip(pieces)
            .map(|(cell, pieces)| {
                let mut items: Vec<(Item, f32)> = pieces
                    .into_iter()
                    .map(|piece| (Item::Text(piece), advances.next().unwrap_or_default()))
                    .collect();
                for _ in &cell.card.tags {
                    let width = tag_widths.next().unwrap_or_default();
                    let tag = tags.next().expect("one text per tag");
                    items.push((
                        Item::Tag(tag, width),
                        TAG_MARGIN + width + 2.0 * TAG_PADDING_X,
                    ));
                }
                Self::wrap(items, line_width)
            })
            .collect()
    }

    // Fill each line with items until the next one would pass the width.
    // A space at the start or the end of a line is dropped, like in CSS
    fn wrap(items: Vec<(Item, f32)>, line_width: f32) -> Self {
        let mut lines: Vec<Vec<(f32, Item)>> = vec![Vec::new()];
        let mut widths = vec![0.0_f32];
        let mut space: Option<(Item, f32)> = None;
        for (item, advance) in items {
            if matches!(&item, Item::Text(text) if text.is_space()) {
                space = Some((item, advance));
                continue;
            }
            let space_advance = space.as_ref().map_or(0.0, |(_, advance)| *advance);
            let started = lines.last().is_some_and(|line| !line.is_empty());
            let used = widths.last().copied().unwrap_or_default();
            if started && used + space_advance + advance > line_width {
                lines.push(Vec::new());
                widths.push(0.0);
                space = None;
            }
            let (Some(line), Some(width)) = (lines.last_mut(), widths.last_mut()) else {
                unreachable!("there is always a line");
            };
            if let Some((space, space_advance)) = space.take()
                && !line.is_empty()
            {
                line.push((*width, space));
                *width += space_advance;
            }
            line.push((*width, item));
            *width += advance;
        }
        Self {
            lines,
            width: widths.into_iter().fold(0.0, f32::max),
        }
    }

    // Like CSS min-width and max-width. A card that wraps fills the maximum width
    fn card_width(&self) -> f32 {
        if self.lines.len() > 1 {
            CARD_MAX_WIDTH
        } else {
            (self.width + 2.0 * (CARD_PADDING_X + CARD_BORDER))
                .clamp(CARD_MIN_WIDTH, CARD_MAX_WIDTH)
        }
    }

    fn card_height(&self) -> f32 {
        self.lines.len() as f32 * LINE_HEIGHT + 2.0 * (CARD_PADDING_Y + CARD_BORDER)
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
    let height = content.card_height() - 2.0 * (CARD_BORDER - border);
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
    let first_baseline = top + border + CARD_PADDING_Y + BASELINE;
    let mut svg = frame;
    let mut tag_index = 0;
    for (line_index, line) in content.lines.iter().enumerate() {
        let baseline = first_baseline + line_index as f32 * LINE_HEIGHT;
        svg.push_str(&render_line(
            index,
            line,
            text_left,
            baseline,
            &mut tag_index,
        ));
    }
    svg
}

// The pieces of text between two tags are drawn as one text
fn render_line(
    card: usize,
    line: &[(f32, Item)],
    left: f32,
    baseline: f32,
    tag_index: &mut usize,
) -> String {
    let mut svg = String::new();
    let mut texts: Vec<TextSpec> = Vec::new();
    let mut texts_left = 0.0;
    for (x, item) in line {
        match item {
            Item::Text(text) => {
                if texts.is_empty() {
                    texts_left = *x;
                }
                texts.push(text.clone());
            }
            Item::Tag(tag, tag_width) => {
                svg.push_str(&render_texts(&mut texts, left + texts_left, baseline));
                let tag_left = left + x + TAG_MARGIN;
                let tag_width_with_padding = tag_width + 2.0 * TAG_PADDING_X;
                svg.push_str(&format!(
                    r#"<rect id="card-{card}-tag-{tag_index}" x="{tag_left}" y="{}" width="{tag_width_with_padding}" height="{TAG_HEIGHT}" rx="{TAG_RADIUS}" fill="white"/>{}"#,
                    baseline - TAG_BASELINE,
                    tag.to_svg(tag_left + TAG_PADDING_X, baseline, "")
                ));
                *tag_index += 1;
            }
        }
    }
    svg.push_str(&render_texts(&mut texts, left + texts_left, baseline));
    svg
}

fn render_texts(texts: &mut Vec<TextSpec>, left: f32, baseline: f32) -> String {
    if texts.is_empty() {
        return String::new();
    }
    let svg = TextSpec::joined(texts).to_svg(left, baseline, "");
    texts.clear();
    svg
}

// Each title sits on the first row of its band. Levels that do not exist, and empty titles, get no title
fn render_level_titles(map: &StoryMap, grid: &Grid) -> String {
    map.level_titles
        .iter()
        .zip(&grid.first_row_by_level)
        .filter(|(title, _)| !title.is_empty())
        .map(|(title, row)| {
            TextSpec::markdown("", title, FONT_SIZE).to_svg(
                PAGE_PADDING + LEVEL_TITLE_MARGIN + LEVEL_TITLE_PADDING_X,
                grid.row_tops[*row] + LEVEL_TITLE_MARGIN + LEVEL_TITLE_PADDING_Y + BASELINE,
                &format!(r#" fill="{LEVEL_TITLE_COLOR}""#),
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
