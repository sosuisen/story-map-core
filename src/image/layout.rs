//! The positions of the blocks and the grid tracks on the canvas

use super::{
    BAND_BORDER, CARD_MARGIN, CardContent, LEVEL_TITLE_MARGIN, LEVEL_TITLE_PADDING_X, PAGE_PADDING,
    ROW_HEIGHT,
};
use crate::StoryMap;

/// Blocks stacked from the top. Like CSS, the margins between two blocks collapse into the larger one
pub(super) struct Flow {
    bottom: f32,
    pending_margin: f32,
}

impl Flow {
    pub(super) fn new(top: f32) -> Self {
        Self {
            bottom: top,
            pending_margin: 0.0,
        }
    }

    // Put a block with the same margin above and below it, and return its top
    pub(super) fn place(&mut self, height: f32, margin: f32) -> f32 {
        let top = self.bottom + self.pending_margin.max(margin);
        self.bottom = top + height;
        self.pending_margin = margin;
        top
    }

    pub(super) fn end(&self) -> f32 {
        self.bottom + self.pending_margin
    }
}

/// The positions of the grid tracks on the canvas
pub(super) struct Grid {
    pub(super) column_lefts: Vec<f32>,
    pub(super) column_widths: Vec<f32>,
    // With level titles, the first column holds them and the cards start at the second column
    pub(super) first_card_column: usize,
    pub(super) row_tops: Vec<f32>,
    pub(super) row_heights: Vec<f32>,
    pub(super) first_row_by_level: Vec<usize>,
    // The size of the grid itself, without the padding of the page
    pub(super) width: f32,
    pub(super) height: f32,
}

impl Grid {
    // Like CSS Grid, each column is as wide as its widest card
    pub(super) fn new(
        map: &StoryMap,
        contents: &[CardContent],
        level_title_widths: &[f32],
        top: f32,
    ) -> Self {
        let first_card_column = usize::from(!map.level_titles.is_empty());
        let mut column_widths = vec![0.0_f32; first_card_column + map.column_count];
        if first_card_column == 1 {
            column_widths[0] = level_title_widths
                .iter()
                .map(|width| width + 2.0 * (LEVEL_TITLE_PADDING_X + LEVEL_TITLE_MARGIN))
                .fold(0.0, f32::max);
        }
        for (cell, content) in map.cells.iter().zip(contents) {
            let width = &mut column_widths[first_card_column + cell.column];
            *width = width.max(content.card_width() + 2.0 * CARD_MARGIN);
        }
        let first_row_by_level = starts_of(&map.rows_by_level, 0);
        // Like CSS Grid, a row without cards or titles has no height. Only the dashed line of the band shows.
        // Otherwise each row is as tall as its tallest card
        let row_count: usize = map.rows_by_level.iter().sum();
        let mut row_heights = vec![BAND_BORDER; row_count];
        for (cell, content) in map.cells.iter().zip(contents) {
            let height = &mut row_heights[first_row_by_level[cell.level] + cell.row_in_level];
            *height = height.max(content.card_height() + 2.0 * CARD_MARGIN);
        }
        for (title, row) in map.level_titles.iter().zip(&first_row_by_level) {
            if !title.is_empty() {
                row_heights[*row] = row_heights[*row].max(ROW_HEIGHT);
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
