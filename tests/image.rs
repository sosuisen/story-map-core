// マップの画像出力
// Rendering the map as an SVG or PNG image

#![cfg(feature = "image")]

use std::sync::LazyLock;

use resvg::usvg::{Options, Tree};
use story_map_core::image::{ImageOptions, render_svg};
use story_map_core::parse;

// Loading the system fonts takes time, so the tests share one set
static IMAGE_OPTIONS: LazyLock<ImageOptions> = LazyLock::new(ImageOptions::default);

fn svg_of(outline: &str) -> String {
    render_svg(&parse(outline), &IMAGE_OPTIONS)
}

fn tree_of(svg: &str) -> Tree {
    let mut options = Options::default();
    options.fontdb = IMAGE_OPTIONS.fonts.clone();
    Tree::from_str(svg, &options).unwrap()
}

// SVGは、マップ全体が入る大きさのSVG文書になる
#[test]
fn the_svg_is_large_enough_for_the_whole_map() {
    let tree = tree_of(&svg_of("- Activity A\n\t- Task A1\n- Activity B"));

    let content = tree.root().abs_bounding_box();
    assert!(content.width() > 0.0 && content.height() > 0.0);
    assert!(content.right() <= tree.size().width());
    assert!(content.bottom() <= tree.size().height());
}
