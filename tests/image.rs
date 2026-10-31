// マップの画像出力
// Rendering the map as an SVG or PNG image

#![cfg(feature = "image")]

use std::sync::LazyLock;

use resvg::usvg::{Group, Node, Options, Paint, Path, Rect, Text, Tree};
use story_map_core::image::{ImageOptions, render_svg};
use story_map_core::parse;

// Loading the system fonts takes time, so the tests share one set
static IMAGE_OPTIONS: LazyLock<ImageOptions> = LazyLock::new(ImageOptions::default);

fn svg_of(outline: &str) -> String {
    render_svg(&parse(outline), &IMAGE_OPTIONS)
}

fn tree_of(svg: &str) -> Tree {
    let options = Options {
        fontdb: IMAGE_OPTIONS.fonts.clone(),
        ..Options::default()
    };
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

    let content = tree.root().abs_stroke_bounding_box();
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

// 子のカードは、親の下に同じ列で並ぶ
#[test]
fn a_child_card_is_below_its_parent_in_the_same_column() {
    let tree = tree_of(&svg_of("- Activity A\n\t- Task A1\n\t\t- Detail"));

    let activity = text_box(&tree, "Activity A");
    let task = text_box(&tree, "Task A1");
    let detail = text_box(&tree, "Detail");
    assert!(activity.bottom() < task.top() && task.bottom() < detail.top());
    assert_eq!(activity.left(), task.left());
    assert_eq!(task.left(), detail.left());
}

// The outer box of the card frame at the given index of the cells, with its border
fn card_box(tree: &Tree, index: usize) -> Rect {
    tree.node_by_id(&format!("card-{index}"))
        .unwrap()
        .abs_stroke_bounding_box()
}

// 長い文字のカードの列は、短い文字の列より広くなる
#[test]
fn a_column_with_a_long_text_is_wider() {
    let tree = tree_of(&svg_of(
        "- Short\n- A much longer activity name than the others\n- Short",
    ));

    let short = card_box(&tree, 0);
    let long = card_box(&tree, 1);
    assert!(long.width() > short.width());
    // 文字は、カードの枠に収まる
    assert!(text_box(&tree, "A much longer activity name than the others").right() < long.right());
    // 広い列の分だけ、右の列は右にずれる
    assert!(long.right() < card_box(&tree, 2).left());
}

// 短い文字のカードも、最小の幅（120px）を持つ
#[test]
fn a_short_card_has_the_minimum_width() {
    let tree = tree_of(&svg_of("- A"));

    assert_eq!(card_box(&tree, 0).width(), 120.0);
}

// 同じ列のカードは、同じ幅になる
#[test]
fn cards_in_the_same_column_have_the_same_width() {
    let tree = tree_of(&svg_of("- Activity\n\t- A task with a much longer name"));

    assert_eq!(card_box(&tree, 0).width(), card_box(&tree, 1).width());
}

fn color_hex(paint: &Paint) -> String {
    match paint {
        Paint::Color(color) => format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue),
        _ => panic!("not a plain color"),
    }
}

fn path_by_id<'a>(tree: &'a Tree, id: &str) -> &'a Path {
    match tree.node_by_id(id) {
        Some(Node::Path(path)) => path,
        _ => panic!("no path with id {id}"),
    }
}

fn fill_of(tree: &Tree, id: &str) -> String {
    color_hex(path_by_id(tree, id).fill().unwrap().paint())
}

fn stroke_of(tree: &Tree, id: &str) -> Option<String> {
    path_by_id(tree, id)
        .stroke()
        .map(|stroke| color_hex(stroke.paint()))
}

// カードと帯は、レベルの色で塗られる
#[test]
fn cards_and_bands_are_painted_in_the_level_colors() {
    let tree = tree_of(&svg_of("- Activity\n\t- Task"));

    // 帯はレベルの基本色、カードはそれより暗い色、枠線はさらに暗い色になる
    assert_eq!(fill_of(&tree, "band-0"), "#e0efff");
    assert_eq!(fill_of(&tree, "card-0"), "#bedeff");
    assert_eq!(stroke_of(&tree, "card-0").as_deref(), Some("#208cff"));
    assert_eq!(fill_of(&tree, "band-1"), "#e0ffee");
    assert_eq!(fill_of(&tree, "card-1"), "#beffdc");
}

// レベル4より下の帯は、1つおきに少し明るくなる
#[test]
fn every_second_band_below_level_4_is_lighter() {
    let tree = tree_of(&svg_of(
        "- A\n\t- B\n\t\t- C\n\t\t\t- D\n\t\t\t\t- E\n\t\t\t\t\t- F",
    ));

    assert_eq!(fill_of(&tree, "band-3"), "#fff3e0");
    assert_eq!(fill_of(&tree, "band-4"), "#fffaf3");
    assert_eq!(fill_of(&tree, "band-5"), "#fff3e0");
}

fn has_shadow(tree: &Tree, index: usize) -> bool {
    match tree.node_by_id(&format!("card-{index}-shadow")) {
        Some(Node::Group(group)) => !group.filters().is_empty(),
        _ => false,
    }
}

// 完了のカードは枠線がなく、未完了のカードには影がある
#[test]
fn a_done_card_has_no_border_and_an_open_card_has_a_shadow() {
    let tree = tree_of(&svg_of("- Activity\n\t- [x] Done\n\t- [ ] Open"));

    assert!(stroke_of(&tree, "card-0").is_some());
    assert!(!has_shadow(&tree, 0));
    assert_eq!(stroke_of(&tree, "card-1"), None);
    assert!(!has_shadow(&tree, 1));
    assert!(stroke_of(&tree, "card-2").is_some());
    assert!(has_shadow(&tree, 2));
}

fn contents_of(tree: &Tree) -> Vec<String> {
    texts_of(tree).into_iter().map(|(text, _)| text).collect()
}

// チェックボックスは、絵文字として描かれる
#[test]
fn checkboxes_are_drawn_as_emoji() {
    let tree = tree_of(&svg_of("- Activity\n\t- [x] Done\n\t- [ ] Open"));

    let contents = contents_of(&tree);
    assert!(contents.contains(&"✅ Done".to_string()));
    assert!(contents.contains(&"⬜ Open".to_string()));
}

// タグは、本文の後ろに丸い背景付きで描かれる
#[test]
fn tags_are_drawn_after_the_text_on_round_backgrounds() {
    let tree = tree_of(&svg_of("- Activity #tag1 #tag2"));

    let text = text_box(&tree, "Activity");
    let tag1 = text_box(&tree, "tag1");
    let tag2 = text_box(&tree, "tag2");
    assert!(text.right() < tag1.left() && tag1.right() < tag2.left());
    // タグの背景は白で、タグの文字を囲む
    assert_eq!(fill_of(&tree, "card-0-tag-0"), "#ffffff");
    let background = tree.node_by_id("card-0-tag-0").unwrap().abs_bounding_box();
    assert!(background.left() < tag1.left() && tag1.right() < background.right());
    // タグまで含めて、カードの枠に収まる
    assert!(
        tree.node_by_id("card-0-tag-1")
            .unwrap()
            .abs_bounding_box()
            .right()
            < card_box(&tree, 0).right()
    );
}
