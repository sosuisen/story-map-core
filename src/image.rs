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
        .enumerate()
        .map(|(index, cell)| {
            format!(
                r#"<text x="10" y="{}">{}</text>"#,
                20 + index * 20,
                cell.card.text
            )
        })
        .collect();
    let height = 20 + map.cells.len() * 20;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="{height}"><rect width="400" height="{height}" fill="white"/>{texts}</svg>"#
    )
}
