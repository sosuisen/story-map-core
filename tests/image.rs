// マップの画像出力
// Rendering the map as an SVG or PNG image

#![cfg(feature = "image")]

use std::sync::LazyLock;

use resvg::usvg::{Group, Node, Options, Rect, Text, Tree};
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

// Every text of the image, with its box on the canvas
fn texts_of(tree: &Tree) -> Vec<(String, Rect)> {
    let mut texts = Vec::new();
    collect_texts(tree.root(), &mut texts);
    texts
}

fn collect_texts(group: &Group, texts: &mut Vec<(String, Rect)>) {
    for node in group.children() {
        match node {
            Node::Group(child) => collect_texts(child, texts),
            Node::Text(text) => texts.push((content_of(text), text.abs_bounding_box())),
            _ => {}
        }
    }
}

fn content_of(text: &Text) -> String {
    text.chunks().iter().map(|chunk| chunk.text()).collect()
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

// カードの文字が、SVGの文字として描かれる
#[test]
fn card_texts_are_drawn_as_svg_text() {
    let tree = tree_of(&svg_of("- Activity A\n\t- Task A1"));

    let texts: Vec<String> = texts_of(&tree).into_iter().map(|(text, _)| text).collect();
    assert!(texts.contains(&"Activity A".to_string()));
    assert!(texts.contains(&"Task A1".to_string()));
}

fn text_box(tree: &Tree, content: &str) -> Rect {
    texts_of(tree)
        .into_iter()
        .find(|(text, _)| text == content)
        .map(|(_, rect)| rect)
        .unwrap()
}

// アクティビティは左から右へ、アウトラインの順に並ぶ
#[test]
fn activities_go_from_left_to_right_in_outline_order() {
    let tree = tree_of(&svg_of("- Activity A\n- Activity B\n- Activity C"));

    let a = text_box(&tree, "Activity A");
    let b = text_box(&tree, "Activity B");
    let c = text_box(&tree, "Activity C");
    assert!(a.right() < b.left() && b.right() < c.left());
    assert_eq!(a.top(), b.top());
    assert_eq!(b.top(), c.top());
}
