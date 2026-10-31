//! Lay out the items of the outline on the grid of the story map

use std::sync::LazyLock;

use regex::Regex;

use crate::tokens::{Token, tokenize};
use crate::{Card, Cell, StoryMap};

static DONE_CHECK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[[xX]\] ").unwrap());
static OPEN_CHECK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[ \] ").unwrap());

// Trailing hashtags such as "#tag1 #tag2" are tags of the item, not part of its text
static TRAILING_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:\s+#\S+)+$").unwrap());

// A slice marker at the start of the text, after an optional checkbox, moves the item up or down the slices (ADR 005).
// "^ " keeps the item in the slice of its parent. "v " skips one slice below the default, "vv " skips two, and so on.
// The space after the marker is required
static SLICE_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\[[ xX]\] )?(\^|v+) ").unwrap());

// A heading whose text is exactly "story map" (any case) starts the story map section
static STORY_MAP_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^story\s+map$").unwrap());

// A line with only a "-" marker is an item with no text. CommonMark does not read it that way:
// right after a paragraph, it is the underline of a setext heading, so the parent item turns into a heading,
// and the story map section ends there. A zero-width space in the item keeps it a list item.
// The placeholder is removed again from the card text, so the card is blank
static EMPTY_ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^([ \t]*-)[ \t]*$").unwrap());
const EMPTY_ITEM_PLACEHOLDER: &str = "\u{200B}";

// An item starting with "//" is a memo about the map. It is not shown on the map
const MEMO_PREFIX: &str = "//";

// The bands of level 1 to 3 (0-based 0 to 2) are always drawn
const MIN_LEVELS: usize = 3;

/// Read a Markdown outline as a story map.
///
/// The items of the first bullet list are the activities. Their children go one level further down, and so on.
/// With a "Story Map" heading, only the section under that heading is read
pub fn parse(outline: &str) -> StoryMap {
    let all_tokens = tokenize(&with_empty_item_placeholder(&normalized(outline)));
    let mut builder = MapBuilder {
        // The map title is the first heading of the whole document, even when it is above the "Story Map" heading
        title: first_heading_of(&all_tokens),
        ..MapBuilder::default()
    };
    let tokens = story_map_tokens(&all_tokens);
    for (index, token) in tokens.iter().enumerate() {
        builder.read(token, tokens.get(index + 1));
    }
    builder.build()
}

// The same line endings and NUL replacement as CommonMark parsers apply before reading
fn normalized(outline: &str) -> String {
    outline
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "\u{FFFD}")
}

fn with_empty_item_placeholder(outline: &str) -> String {
    EMPTY_ITEM_LINE
        .replace_all(outline, format!("$1 {EMPTY_ITEM_PLACEHOLDER}"))
        .into_owned()
}

fn first_heading_of(tokens: &[Token]) -> String {
    tokens
        .iter()
        .position(|token| *token == Token::HeadingOpen)
        .and_then(|index| inline_content(tokens.get(index + 1)))
        .unwrap_or_default()
        .to_string()
}

fn inline_content(token: Option<&Token>) -> Option<&str> {
    match token {
        Some(Token::Inline(content)) => Some(content),
        _ => None,
    }
}

// With a "Story Map" heading, only the content from that heading up to the next heading is the story map.
// Without it, the whole document is the story map
fn story_map_tokens(tokens: &[Token]) -> &[Token] {
    let heading_index = tokens.iter().enumerate().position(|(index, token)| {
        *token == Token::HeadingOpen
            && STORY_MAP_HEADING
                .is_match(inline_content(tokens.get(index + 1)).unwrap_or("").trim())
    });
    let Some(heading_index) = heading_index else {
        return tokens;
    };
    // A heading is three tokens: HeadingOpen, Inline, HeadingClose
    let start = (heading_index + 3).min(tokens.len());
    let end = tokens[start..]
        .iter()
        .position(|token| *token == Token::HeadingOpen)
        .map_or(tokens.len(), |offset| start + offset);
    &tokens[start..end]
}

// How many slices the marker moves the item from the default slice (one below the parent):
// -1 for "^", the number of v's for "v...", and 0 without a marker
fn slice_shift_of(content: &str) -> isize {
    match SLICE_MARKER
        .captures(content)
        .and_then(|captures| captures.get(2))
    {
        None => 0,
        Some(marker) if marker.as_str() == "^" => -1,
        Some(marker) => marker.len() as isize,
    }
}

// The text without its slice marker. The checkbox, if any, stays
fn without_slice_marker(content: &str) -> String {
    SLICE_MARKER.replace(content, "$1").into_owned()
}

fn card_of(content: &str) -> Card {
    let text = TRAILING_TAGS
        .replace(content, "")
        .replace(EMPTY_ITEM_PLACEHOLDER, "");
    let tags = TRAILING_TAGS
        .find(content)
        .map(|tags| {
            tags.as_str()
                .split_whitespace()
                .map(|tag| tag[1..].to_string())
                .collect()
        })
        .unwrap_or_default();
    let done = DONE_CHECK.is_match(&text);
    let todo = OPEN_CHECK.is_match(&text);
    let text = if done || todo {
        text[4..].to_string()
    } else {
        text
    };
    Card {
        text,
        done,
        todo,
        tags,
    }
}

/// The latest item at an indent depth: a memo (hidden with its children), or a placed item with its level and row
#[derive(Debug, Clone, Copy)]
enum IndentItem {
    Memo,
    Placed { level: usize, row_in_level: usize },
}

/// An activity column group: the top activity card, and the cells below it in sub-columns
#[derive(Debug, Default)]
struct ColumnGroup {
    activity: Option<Card>,
    sub_columns: Vec<Vec<(Card, usize, usize)>>,
    last_indent_depth: usize,
}

#[derive(Debug, Default)]
struct MapBuilder {
    title: String,
    leading_notes: Vec<String>,
    trailing_notes: Vec<String>,
    list_seen: bool,
    // The items of the first top-level ordered list, used as the level titles from the top
    level_titles: Vec<String>,
    in_ordered_list: bool,
    ordered_list_seen: bool,
    columns: Vec<ColumnGroup>,
    open_lists: usize,
    // The number of rows each level needs, which is the largest row_in_level + 1 over all columns
    rows_by_level: Vec<usize>,
    // The latest item at each indent depth. Used to decide the level and the row_in_level of a child
    item_by_indent_depth: Vec<Option<IndentItem>>,
}

impl MapBuilder {
    fn read(&mut self, token: &Token, next: Option<&Token>) {
        match token {
            Token::BulletListOpen => {
                self.open_lists += 1;
                self.list_seen = true;
            }
            Token::BulletListClose => self.open_lists = self.open_lists.saturating_sub(1),
            Token::OrderedListOpen if self.open_lists == 0 && !self.ordered_list_seen => {
                self.in_ordered_list = true;
                self.ordered_list_seen = true;
            }
            Token::OrderedListClose if self.in_ordered_list => self.in_ordered_list = false,
            // Every item is a level. An empty item (no inline token follows) stays a level with no title
            Token::ListItemOpen if self.in_ordered_list => self.level_titles.push(String::new()),
            Token::Inline(content) if self.in_ordered_list => {
                if let Some(last) = self.level_titles.last_mut() {
                    *last = content.clone();
                }
            }
            Token::ParagraphOpen if !self.list_seen && !self.in_ordered_list => {
                self.leading_notes
                    .push(inline_content(next).unwrap_or("").to_string());
            }
            Token::ParagraphOpen if self.list_seen && self.open_lists == 0 => {
                self.trailing_notes
                    .push(inline_content(next).unwrap_or("").to_string());
            }
            Token::Inline(content) if self.open_lists > 0 => self.read_item(content),
            _ => {}
        }
    }

    fn read_item(&mut self, content: &str) {
        let indent_depth = self.open_lists - 1;
        let parent = indent_depth
            .checked_sub(1)
            .and_then(|depth| self.item_by_indent_depth.get(depth).copied().flatten());
        // A memo item makes no card and does not change the layout. Its children are memos too.
        // A slice marker in front of the "//" does not change that
        let item = if without_slice_marker(content).starts_with(MEMO_PREFIX)
            || matches!(parent, Some(IndentItem::Memo))
        {
            IndentItem::Memo
        } else {
            self.place_item(content, indent_depth, parent)
        };
        if self.item_by_indent_depth.len() <= indent_depth {
            self.item_by_indent_depth.resize(indent_depth + 1, None);
        }
        self.item_by_indent_depth[indent_depth] = Some(item);
    }

    fn place_item(
        &mut self,
        content: &str,
        indent_depth: usize,
        parent: Option<IndentItem>,
    ) -> IndentItem {
        // A child goes one level below its parent. With a "^" marker, the child stays on the parent level instead,
        // one row further down inside the band. With "v" markers, the child skips that many levels further down
        let slice_shift = slice_shift_of(content);
        let (level, row_in_level) = match parent {
            Some(IndentItem::Placed {
                level,
                row_in_level,
            }) if slice_shift == -1 => (level, row_in_level + 1),
            Some(IndentItem::Placed { level, .. }) => (level + 1 + slice_shift as usize, 0),
            _ => (0, 0),
        };
        self.need_rows(level, row_in_level + 1);
        let card = card_of(&without_slice_marker(content));
        if indent_depth == 0 {
            self.columns.push(ColumnGroup {
                activity: Some(card),
                ..ColumnGroup::default()
            });
        } else if let Some(column) = self.columns.last_mut() {
            // A sibling (same or shallower indent depth) starts a new sub-column to the right
            if column.sub_columns.is_empty() || indent_depth <= column.last_indent_depth {
                column.sub_columns.push(Vec::new());
            }
            if let Some(sub_column) = column.sub_columns.last_mut() {
                sub_column.push((card, level, row_in_level));
            }
            column.last_indent_depth = indent_depth;
        }
        IndentItem::Placed {
            level,
            row_in_level,
        }
    }

    fn need_rows(&mut self, level: usize, rows: usize) {
        if self.rows_by_level.len() <= level {
            self.rows_by_level.resize(level + 1, 1);
        }
        self.rows_by_level[level] = self.rows_by_level[level].max(rows);
    }

    fn build(mut self) -> StoryMap {
        if self.rows_by_level.len() < MIN_LEVELS {
            self.rows_by_level.resize(MIN_LEVELS, 1);
        }
        let mut cells = Vec::new();
        let mut next_column = 0;
        for group in self.columns {
            if let Some(activity) = group.activity {
                cells.push(Cell {
                    card: activity,
                    level: 0,
                    row_in_level: 0,
                    column: next_column,
                });
            }
            for (offset, sub_column) in group.sub_columns.iter().enumerate() {
                cells.extend(sub_column.iter().map(|(card, level, row_in_level)| Cell {
                    card: card.clone(),
                    level: *level,
                    row_in_level: *row_in_level,
                    column: next_column + offset,
                }));
            }
            next_column += group.sub_columns.len().max(1);
        }
        StoryMap {
            title: self.title,
            leading_notes: self.leading_notes,
            trailing_notes: self.trailing_notes,
            level_titles: self.level_titles,
            cells,
            rows_by_level: self.rows_by_level,
            column_count: next_column,
        }
    }
}
