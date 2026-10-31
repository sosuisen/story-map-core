//! Render a story map as HTML.
//!
//! [`map_css`] gives the style sheet, and [`render_map_content`] gives the map itself.
//! The page around them, such as a zoom wrapper or toolbar, is up to the app.

mod inline;

pub use crate::escape::escape_markup as escape_html;
pub use inline::inline_html;

use crate::style::{
    ALT_SHADE, BORDER_SHADE, CARD_SHADE, LEVEL_COLORS, checkbox_emoji, is_alt_band, level_number_of,
};
use crate::{Card, StoryMap};

/// The CSS rules that give the map its look. Put them in a `<style>` element of the page.
///
/// A card is up to 240px wide, and a longer text wraps onto more lines.
/// Set the CSS variable `--card-max-width` on the page to change the width
pub fn map_css() -> String {
    format!(
        r#":root {{ {}--card-shade: {CARD_SHADE}; --border-shade: {BORDER_SHADE}; --alt-shade: {ALT_SHADE}; }}
body {{ background: white; color: black; }}
.map-zoom {{ width: fit-content; display: flow-root; padding: 16px; }}
.map-grid {{ display: grid; gap: 0; justify-content: start; align-items: start; }}
.card {{ border: 2px solid; padding: 4px 8px; margin: 8px; border-radius: 6px; min-width: 120px; max-width: var(--card-max-width, 240px); overflow-wrap: anywhere; align-self: stretch; box-sizing: border-box; }}
.todo {{ box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2); }}
{}
.done {{ border: none; box-shadow: none; }}
.tag {{ background: white; border-radius: 8px; padding: 0 6px; margin-left: 4px; font-size: 0.8em; white-space: nowrap; }}
.row-label {{ padding: 4px 8px; margin: 8px; color: #888; white-space: nowrap; }}
.row-band {{ align-self: stretch; justify-self: stretch; border-bottom: 2px dashed; }}
.row-band.level4.alt {{ background: hsl(from var(--level4-color) h s calc(l * var(--alt-shade))); }}"#,
        level_color_variables(),
        level_color_rules()
    )
}

fn level_color_variables() -> String {
    LEVEL_COLORS
        .iter()
        .enumerate()
        .map(|(index, color)| format!("--level{}-color: {color}; ", index + 1))
        .collect()
}

// The CSS rules that give each level its colors. The band is painted in the base color of the level.
// The card background is the base color with a lower lightness, and the card border is darker still
fn level_color_rules() -> String {
    (1..=4)
        .flat_map(|level| {
            [
                format!(
                    ".card.level{level} {{ background: hsl(from var(--level{level}-color) h s calc(l * var(--card-shade))); border-color: hsl(from var(--level{level}-color) h s calc(l * var(--border-shade))); }}"
                ),
                format!(
                    ".row-band.level{level} {{ background: var(--level{level}-color); border-color: hsl(from var(--level{level}-color) h s calc(l * var(--card-shade))); }}"
                ),
            ]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the map: the title, the notes above, the grid of cards, and the notes below
pub fn render_map_content(map: &StoryMap) -> String {
    // The first CSS row of each band. Bands are stacked in level order, and each band is as tall as the level needs
    let first_row_by_level: Vec<usize> = map
        .rows_by_level
        .iter()
        .scan(1, |next_row, rows| {
            let first = *next_row;
            *next_row += rows;
            Some(first)
        })
        .collect();
    // With level titles, the first column holds them and the cards start at the second column.
    // Without level titles there is no title column, and the cards start at the first column (ADR 004)
    let first_column = if map.level_titles.is_empty() { 1 } else { 2 };
    let end_column = first_column + map.column_count;
    // Bands go behind the cards, so they come first in DOM order. One band per level, as tall as the level needs
    let bands = map.rows_by_level.iter().zip(&first_row_by_level).enumerate().map(|(level, (rows, first))| {
        let grid_row = if *rows == 1 {
            first.to_string()
        } else {
            format!("{first} / {}", first + rows)
        };
        format!(
            r#"<div class="row-band {}" style="grid-column: 1 / {end_column}; grid-row: {grid_row};"></div>"#,
            band_class_of(level)
        )
    });
    // Each title sits on the first row of its band. Levels that do not exist, and empty titles, get no title element.
    // Without an ordered list there are no titles at all (ADR 004)
    let labels = map
        .level_titles
        .iter()
        .zip(&first_row_by_level)
        .rev()
        .filter(|(title, _)| !title.is_empty())
        .map(|(title, row)| {
            format!(
                r#"<div class="row-label" style="grid-column: 1; grid-row: {row};">{}</div>"#,
                inline_html(title)
            )
        });
    let cards = map.cells.iter().map(|cell| {
        let row = first_row_by_level[cell.level] + cell.row_in_level;
        let column = first_column + cell.column;
        render_card(
            &cell.card,
            cell.level,
            &format!("grid-column: {column}; grid-row: {row};"),
        )
    });
    let grid: String = bands.chain(labels).chain(cards).collect();
    format!(
        r#"<h1 class="map-title">{}</h1>{}<div class="map-grid">{grid}</div>{}"#,
        map.title,
        render_notes("leading-note", &map.leading_notes),
        render_notes("trailing-note", &map.trailing_notes)
    )
}

fn render_notes(class_name: &str, notes: &[String]) -> String {
    notes
        .iter()
        .map(|note| format!(r#"<p class="{class_name}">{}</p>"#, inline_html(note)))
        .collect()
}

fn render_card(card: &Card, level: usize, position: &str) -> String {
    let mut classes = format!("card level{}", level_number_of(level));
    if card.done {
        classes.push_str(" done");
    }
    if card.todo {
        classes.push_str(" todo");
    }
    let text = format!("{}{}", checkbox_emoji(card), inline_html(&card.text));
    let tags: String = card
        .tags
        .iter()
        .map(|tag| format!(r#"<span class="tag">{}</span>"#, escape_html(tag)))
        .collect();
    format!(r#"<div class="{classes}" style="{position}">{text}{tags}</div>"#)
}

// The band class for a level
fn band_class_of(level: usize) -> String {
    let base = format!("level{}", level_number_of(level));
    if is_alt_band(level) {
        format!("{base} alt")
    } else {
        base
    }
}
