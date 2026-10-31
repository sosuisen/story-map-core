//! Turn a Markdown outline into a user story map.
//!
//! [`parse`] reads the outline and lays out the cards on a grid. The result, [`StoryMap`], holds only data.
//! Texts stay raw Markdown, so that each app can render them in its own way.
//! With the `html` feature, the [`html`] module renders the map as HTML.
//! With the `image` feature, the [`image`] module renders the map as an SVG or PNG image.

mod layout;
mod tokens;

#[cfg(feature = "html")]
pub mod html;

#[cfg(feature = "image")]
pub mod image;

pub use layout::parse;

/// One item of the outline, shown as a card on the map
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// The raw Markdown text, without the checkbox, the slice marker, and the tags
    pub text: String,
    /// The item starts with a done check `[x]`
    pub done: bool,
    /// The item starts with an open check `[ ]`
    pub todo: bool,
    /// The trailing hashtags, without the "#"
    pub tags: Vec<String>,
}

/// A card placed on the grid
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub card: Card,
    /// The level from the top, starting at 0. The activities are on level 0
    pub level: usize,
    /// The row inside the band of the level: 0 for the first row, 1 for a child marked with "^", and so on
    pub row_in_level: usize,
    /// The grid column, starting at 0
    pub column: usize,
}

/// A story map read from an outline
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryMap {
    /// The raw text of the first heading of the document (empty if none)
    pub title: String,
    /// The raw Markdown of the paragraphs above the outline
    pub leading_notes: Vec<String>,
    /// The raw Markdown of the paragraphs below the outline
    pub trailing_notes: Vec<String>,
    /// The raw Markdown of the level titles from the top. An empty string is a level with no title
    pub level_titles: Vec<String>,
    /// The cards in outline order
    pub cells: Vec<Cell>,
    /// The number of rows of each level band. There are always at least 3 levels
    pub rows_by_level: Vec<usize>,
    /// The number of grid columns that the cards use
    pub column_count: usize,
}

// Run the code examples in the README as doctests, so that they keep working
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
