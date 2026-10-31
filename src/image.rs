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
use text::{TextSpec, measure};

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
const DEFAULT_MAX_CARD_WIDTH: f32 = 160.0;
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
    /// The widest that a card may be, in CSS pixels. A longer text wraps onto more lines.
    /// Like CSS, the minimum width of 120 wins over a smaller value
    pub max_card_width: f32,
}

impl Default for ImageOptions {
    /// The fonts of the OS, at a scale of 1.0, with cards up to 160 pixels wide
    fn default() -> Self {
        let mut fonts = fontdb::Database::new();
        fonts.load_system_fonts();
        use_installed_generic_fonts(&mut fonts);
        Self {
            fonts: Arc::new(fonts),
            scale: 1.0,
            max_card_width: DEFAULT_MAX_CARD_WIDTH,
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

/// One line of a card: the text, then the tags after it
#[derive(Default)]
struct Line {
    text: Vec<TextSpec>,
    // Each tag with its left edge from the start of the line, and the width of its text
    tags: Vec<(f32, TextSpec, f32)>,
    width: f32,
}

impl Line {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.tags.is_empty()
    }
}

/// The lines of a card
struct CardContent {
    lines: Vec<Line>,
    // The width of the widest line
    width: f32,
    max_card_width: f32,
}

impl CardContent {
    fn measure_all(map: &StoryMap, options: &ImageOptions) -> Vec<Self> {
        let line_width = options.max_card_width - 2.0 * (CARD_PADDING_X + CARD_BORDER);
        // The text of each card with its checkbox as an emoji, cut where a line may break
        let pieces = map
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
        let mut tags = tags
            .iter()
            .cloned()
            .zip(measure(&tags, options))
            .collect::<Vec<_>>()
            .into_iter();
        map.cells
            .iter()
            .zip(wrap_all(pieces, line_width, options))
            .map(|(cell, mut lines)| {
                place_tags(
                    &mut lines,
                    tags.by_ref().take(cell.card.tags.len()),
                    line_width,
                );
                Self {
                    width: lines.iter().map(|line| line.width).fold(0.0, f32::max),
                    lines,
                    max_card_width: options.max_card_width,
                }
            })
            .collect()
    }

    // Like CSS min-width and max-width, where the minimum wins. A card that wraps fills the maximum width
    fn card_width(&self) -> f32 {
        let natural = if self.lines.len() > 1 {
            self.max_card_width
        } else {
            (self.width + 2.0 * (CARD_PADDING_X + CARD_BORDER)).min(self.max_card_width)
        };
        natural.max(CARD_MIN_WIDTH)
    }

    // The height that the lines need. The row may make the card taller
    fn card_height(&self, border: f32) -> f32 {
        self.lines.len() as f32 * LINE_HEIGHT + 2.0 * (CARD_PADDING_Y + border)
    }
}

// Fill each line with pieces until the next one would pass the width.
// A line is measured as a whole, not piece by piece. usvg draws a text that has a Japanese character
// all in the fallback font, so the width of a piece depends on the rest of the line.
// Each measurement is slow, so all cards are wrapped together and each step measures one text of every card
fn wrap_all(cards: Vec<Vec<TextSpec>>, line_width: f32, options: &ImageOptions) -> Vec<Vec<Line>> {
    let mut wrappings: Vec<Wrapping> = cards
        .into_iter()
        .map(|pieces| Wrapping {
            pieces,
            start: 0,
            lines: Vec::new(),
            search: None,
            line_width,
        })
        .collect();
    loop {
        let (indexes, texts): (Vec<usize>, Vec<TextSpec>) = wrappings
            .iter_mut()
            .enumerate()
            .filter_map(|(index, wrapping)| wrapping.next_text().map(|text| (index, text)))
            .unzip();
        if texts.is_empty() {
            break;
        }
        for (index, width) in indexes.into_iter().zip(measure(&texts, options)) {
            wrappings[index].take_width(width);
        }
    }
    wrappings
        .into_iter()
        .map(|wrapping| wrapping.lines)
        .collect()
}

/// The wrapping of one card, one measurement at a time
struct Wrapping {
    pieces: Vec<TextSpec>,
    // The first piece of the current line
    start: usize,
    lines: Vec<Line>,
    // A binary search for the end of the current line: the most pieces known to fit, their width,
    // and the fewest known not to fit. Most cards fit on one line, so the search starts after the whole rest fails
    search: Option<(usize, f32, usize)>,
    line_width: f32,
}

impl Wrapping {
    // The text to measure next, or None when the card is done.
    // A space at the start or the end of a line is dropped, like in CSS
    fn next_text(&mut self) -> Option<TextSpec> {
        loop {
            while self.pieces.get(self.start).is_some_and(TextSpec::is_space) {
                self.start += 1;
            }
            let rest = &self.pieces[self.start..];
            if rest.is_empty() {
                return None;
            }
            match self.search {
                None => return Some(TextSpec::joined(rest)),
                Some((fits, _, too_many)) if too_many - fits > 1 => {
                    return Some(TextSpec::joined(&rest[..(fits + too_many) / 2]));
                }
                Some((0, _, _)) => {
                    // A word that is wider than a line is cut into characters, like CSS `overflow-wrap: anywhere`.
                    // A single character cannot be cut, and passes the width
                    let characters = rest[0].characters();
                    if characters.len() > 1 {
                        self.pieces.splice(self.start..=self.start, characters);
                        self.search = None;
                    } else {
                        self.end_line(1, self.line_width);
                    }
                }
                Some((fits, width, _)) => self.end_line(fits, width),
            }
        }
    }

    fn take_width(&mut self, width: f32) {
        let fits = width <= self.line_width;
        self.search = match self.search {
            None if fits => {
                self.end_line(self.pieces.len() - self.start, width);
                None
            }
            None => Some((0, 0.0, self.pieces.len() - self.start)),
            Some((known, known_width, too_many)) => {
                let middle = (known + too_many) / 2;
                if fits {
                    Some((middle, width, too_many))
                } else {
                    Some((known, known_width, middle))
                }
            }
        };
    }

    fn end_line(&mut self, count: usize, width: f32) {
        let mut text = self.pieces[self.start..self.start + count].to_vec();
        while text.last().is_some_and(TextSpec::is_space) {
            text.pop();
        }
        self.lines.push(Line {
            text,
            tags: Vec::new(),
            width,
        });
        self.start += count;
        self.search = None;
    }
}

// The tags follow the text, and wrap onto new lines in the same way
fn place_tags(
    lines: &mut Vec<Line>,
    tags: impl IntoIterator<Item = (TextSpec, f32)>,
    line_width: f32,
) {
    if lines.is_empty() {
        lines.push(Line::default());
    }
    for (tag, width) in tags {
        let advance = TAG_MARGIN + width + 2.0 * TAG_PADDING_X;
        if lines
            .last()
            .is_some_and(|line| !line.is_empty() && line.width + advance > line_width)
        {
            lines.push(Line::default());
        }
        let Some(line) = lines.last_mut() else {
            unreachable!("there is always a line");
        };
        line.tags.push((line.width, tag, width));
        line.width += advance;
    }
}

// A done card has no border, like in CSS
fn border_of(cell: &Cell) -> f32 {
    if cell.card.done { 0.0 } else { CARD_BORDER }
}

// The shadow of an open card, like `box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2)`
const SHADOW_FILTER: &str = r#"<filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="2" dy="2" stdDeviation="2" flood-color="black" flood-opacity="0.2"/></filter>"#;

fn render_card(index: usize, cell: &Cell, content: &CardContent, grid: &Grid) -> String {
    let column = grid.first_card_column + cell.column;
    let left = grid.column_lefts[column] + CARD_MARGIN;
    let row = grid.first_row_by_level[cell.level] + cell.row_in_level;
    let top = grid.row_tops[row] + CARD_MARGIN;
    let width = grid.column_widths[column] - 2.0 * CARD_MARGIN;
    // Like `align-self: stretch` in CSS, every card fills the height of its row
    let height = grid.row_heights[row] - 2.0 * CARD_MARGIN;
    let base = level_color(cell.level);
    let fill = shaded(base, CARD_SHADE);
    let border = border_of(cell);
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

fn render_line(
    card: usize,
    line: &Line,
    left: f32,
    baseline: f32,
    tag_index: &mut usize,
) -> String {
    let mut svg = if line.text.is_empty() {
        String::new()
    } else {
        TextSpec::joined(&line.text).to_svg(left, baseline, "")
    };
    for (x, tag, tag_width) in &line.tags {
        let tag_left = left + x + TAG_MARGIN;
        let tag_width_with_padding = tag_width + 2.0 * TAG_PADDING_X;
        svg.push_str(&format!(
            r#"<rect id="card-{card}-tag-{tag_index}" x="{tag_left}" y="{}" width="{tag_width_with_padding}" height="{TAG_HEIGHT}" rx="{TAG_RADIUS}" fill="white"/>{}"#,
            baseline - TAG_BASELINE,
            tag.to_svg(tag_left + TAG_PADDING_X, baseline, "")
        ));
        *tag_index += 1;
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
