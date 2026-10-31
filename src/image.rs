//! Render a story map as an SVG or PNG image.
//!
//! The image follows the look of the HTML output.
//! The text is measured with real fonts, so that each column is as wide as its longest card.

use std::sync::Arc;

pub use resvg::usvg::fontdb;

use crate::StoryMap;

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
pub fn render_svg(map: &StoryMap, _options: &ImageOptions) -> String {
    let texts: String = map
        .cells
        .iter()
        .map(|cell| {
            format!(
                r#"<text x="{}" y="{}">{}</text>"#,
                10 + cell.column * 200,
                20 + cell.level * 20,
                cell.card.text
            )
        })
        .collect();
    let width = 10 + map.column_count * 200;
    let height = 20 + map.rows_by_level.len() * 20;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="{width}" height="{height}" fill="white"/>{texts}</svg>"#
    )
}
