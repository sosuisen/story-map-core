// マップの画像出力
// Rendering the map as an SVG or PNG image

#![cfg(feature = "image")]

use std::sync::LazyLock;

use proptest::prelude::*;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{FontFamily, FontStyle, Group, Node, Options, Paint, Path, Rect, Text, Tree};
use storymap::image::{ImageOptions, render_png, render_svg};
use storymap::parse;

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
    let tree = tree_of(&svg_of("- Short\n- A longer activity name\n- Short"));

    let short = card_box(&tree, 0);
    let long = card_box(&tree, 1);
    assert!(long.width() > short.width());
    // 文字は、カードの枠に収まる
    assert!(text_box(&tree, "A longer activity name").right() < long.right());
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

const LONG_TEXT: &str = "A card with a long text that does not fit on one line of the card";

// The lines of the first card, which wraps at the default maximum width. Each line is inside the card
fn wrapped_lines(outline: &str) -> Vec<String> {
    let tree = tree_of(&svg_of(outline));
    let card = card_box(&tree, 0);
    assert_eq!(card.width(), 240.0);
    let lines = texts_of(&tree);
    assert!(lines.iter().all(|(_, line)| contains(card, *line)));
    lines.into_iter().map(|(text, _)| text).collect()
}

// 上限より長い文のカードは、上限の幅（既定240px）で、複数の行に折り返す
#[test]
fn a_long_card_wraps_at_the_maximum_width() {
    let lines = wrapped_lines(&format!("- {LONG_TEXT}"));

    assert!(lines.len() > 1);
    // 行をつなぐと、元の文に戻る
    assert_eq!(lines.join(" "), LONG_TEXT);
}

// 上限は、ImageOptionsで変えられる
#[test]
fn the_maximum_width_can_be_changed_in_the_options() {
    let options = ImageOptions {
        max_card_width: 400.0,
        ..IMAGE_OPTIONS.clone()
    };
    let tree = tree_of(&render_svg(&parse(&format!("- {LONG_TEXT}")), &options));

    assert_eq!(card_box(&tree, 0).width(), 400.0);
}

// 日本語の文は、文字の間で折り返す
#[test]
fn a_japanese_text_wraps_between_characters() {
    let text = "日本語の文には空白がないので、どの文字の間でも折り返してカードに収める";

    let lines = wrapped_lines(&format!("- {text}"));

    assert!(lines.len() > 1);
    assert_eq!(lines.concat(), text);
}

// 日本語の文の行は、カードの幅を使い切ってから折り返す
#[test]
fn a_japanese_line_fills_the_card_before_it_wraps() {
    let tree = tree_of(&svg_of(
        "- 日本語の文には空白がないので、どの文字の間でも折り返してカードに収める",
    ));

    let card = card_box(&tree, 0);
    let (_, first_line) = &texts_of(&tree)[0];
    // 1行には、あと2文字（16pxの全角文字）を足す余白が残らない
    let inner_width = card.width() - 2.0 * (8.0 + 2.0);
    assert!(first_line.width() > inner_width - 2.0 * 16.0);
}

// 句読点や閉じ括弧は行の先頭に来ず、開き括弧は行の末尾に来ない
#[test]
fn punctuation_does_not_start_a_line_and_an_opening_bracket_does_not_end_one() {
    let text = "あ。い、う」え）お。か、き」く）け。こ、さ「し（す。せ、そ「た（ち。つ、て「と（な。に、ぬ「ね（の。";

    let lines = wrapped_lines(&format!("- {text}"));

    assert!(lines.len() > 2);
    assert_eq!(lines.concat(), text);
    for line in &lines {
        assert!(!line.starts_with(['。', '、', '」', '）']), "{lines:?}");
        assert!(!line.ends_with(['「', '（']), "{lines:?}");
    }
}

// 上限より長い1語は、語の途中で折り返す
#[test]
fn a_word_longer_than_the_maximum_breaks_inside() {
    let word = "https://example.com/a/very/long/address/that/has/no/spaces/at/all";

    let lines = wrapped_lines(&format!("- {word}"));

    assert!(lines.len() > 1);
    assert_eq!(lines.concat(), word);
}

// タグも、本文に続けて折り返す
#[test]
fn tags_wrap_after_the_text() {
    let tree = tree_of(&svg_of("- A task with tags #first #second #third #fourth"));

    let card = card_box(&tree, 0);
    assert_eq!(card.width(), 240.0);
    let last_tag = node_box(&tree, "card-0-tag-3");
    assert!(text_box(&tree, "A task with tags").bottom() < last_tag.top());
    assert!(contains(card, last_tag));
}

// 折り返したカードの行は高くなり、同じ行のカードは同じ高さになる
#[test]
fn a_row_with_a_wrapped_card_is_taller_and_its_cards_match() {
    let tree = tree_of(&svg_of(&format!("- Short\n- {LONG_TEXT}")));
    let one_line = card_box(&tree_of(&svg_of("- Short")), 0);

    let short = card_box(&tree, 0);
    let long = card_box(&tree, 1);
    assert!(long.height() > one_line.height());
    assert_eq!(short.top(), long.top());
    assert_eq!(short.height(), long.height());
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

// レベルのタイトルは1列目に描かれ、カードは2列目から始まる
#[test]
fn level_titles_are_drawn_in_the_first_column() {
    let tree = tree_of(&svg_of("1. Activities\n2. Tasks\n\n- Activity\n\t- Task"));

    let activities = text_box(&tree, "Activities");
    let tasks = text_box(&tree, "Tasks");
    assert!(activities.right() < card_box(&tree, 0).left());
    assert!(tasks.right() < card_box(&tree, 1).left());
    // タイトルは、そのレベルの行に描かれる
    assert!(
        activities.top() > card_box(&tree, 0).top()
            && activities.bottom() < card_box(&tree, 0).bottom()
    );
    assert!(tasks.top() > card_box(&tree, 1).top() && tasks.bottom() < card_box(&tree, 1).bottom());
}

fn node_box(tree: &Tree, id: &str) -> Rect {
    tree.node_by_id(id).unwrap().abs_bounding_box()
}

fn contains(outer: Rect, inner: Rect) -> bool {
    outer.left() <= inner.left()
        && inner.right() <= outer.right()
        && outer.top() <= inner.top()
        && inner.bottom() <= outer.bottom()
}

// `^`で同じレベルに入ったカードは、帯の2行目に描かれる
#[test]
fn a_caret_card_is_drawn_on_the_second_row_of_the_band() {
    let tree = tree_of(&svg_of("- Activity\n\t- Task\n\t\t- ^ Same level"));

    let task = card_box(&tree, 1);
    let same = card_box(&tree, 2);
    assert!(task.bottom() < same.top());
    assert_eq!(task.left(), same.left());
    assert!(contains(node_box(&tree, "band-1"), task));
    assert!(contains(node_box(&tree, "band-1"), same));
}

// カードのないレベルは、細い帯として描かれる
#[test]
fn a_level_without_cards_is_drawn_as_a_thin_band() {
    let tree = tree_of(&svg_of("- Activity\n\t- Task"));

    let empty = node_box(&tree, "band-2");
    assert!(empty.height() > 0.0);
    assert!(empty.height() < card_box(&tree, 1).height());
}

// タイトルとリストの上のノートは、マップの上に描かれる
#[test]
fn the_title_and_the_leading_notes_are_drawn_above_the_map() {
    let tree = tree_of(&svg_of(
        "# Map Title\n\nFirst note.\n\nSecond note.\n\n- Activity",
    ));

    let title = text_box(&tree, "Map Title");
    let first = text_box(&tree, "First note.");
    let second = text_box(&tree, "Second note.");
    assert!(title.bottom() < first.top() && first.bottom() < second.top());
    assert!(second.bottom() < node_box(&tree, "band-0").top());
    // タイトルは、ノートより大きな文字で描かれる
    assert!(title.height() > first.height());
}

// リストの下のノートは、マップの下に描かれる
#[test]
fn the_trailing_notes_are_drawn_below_the_map() {
    let tree = tree_of(&svg_of("- Activity\n\t- Task\n\nBelow the map."));

    let below = text_box(&tree, "Below the map.");
    let last_band = node_box(&tree, "band-2");
    assert!(last_band.bottom() < below.top());
    assert!(below.bottom() <= tree.size().height());
}

// The pieces of a text with their fonts: (text, bold, italic, monospace)
fn spans_of(tree: &Tree, content: &str) -> Vec<(String, bool, bool, bool)> {
    let mut spans = Vec::new();
    find_spans(tree.root(), content, &mut spans);
    spans
}

fn find_spans(group: &Group, content: &str, spans: &mut Vec<(String, bool, bool, bool)>) {
    for node in group.children() {
        match node {
            Node::Group(child) => find_spans(child, content, spans),
            Node::Text(text) if content_of(text) == content => {
                for chunk in text.chunks() {
                    for span in chunk.spans() {
                        let font = span.font();
                        spans.push((
                            chunk.text()[span.start()..span.end()].to_string(),
                            font.weight() >= 700,
                            font.style() == FontStyle::Italic,
                            font.families().contains(&FontFamily::Monospace),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

// 太字・斜体・コードは、書体を変えて描かれる。リンクはただの文字になる
#[test]
fn bold_italic_and_code_are_drawn_in_their_fonts() {
    let tree = tree_of(&svg_of(
        "- **Bold** and *italic* with `code` [link](https://example.com)",
    ));

    let spans = spans_of(&tree, "Bold and italic with code link");
    assert!(spans.contains(&("Bold".to_string(), true, false, false)));
    assert!(spans.contains(&("italic".to_string(), false, true, false)));
    assert!(spans.contains(&("code".to_string(), false, false, true)));
    assert!(
        spans
            .iter()
            .any(|(text, bold, italic, mono)| text.contains("link") && !bold && !italic && !mono)
    );
}

// XMLの特殊文字は、エスケープされる
#[test]
fn xml_special_characters_are_escaped() {
    let tree = tree_of(&svg_of(
        "# A < B & \"C\"\n- x < y & 'z' > \"w\"\n  - tag #a&b",
    ));

    let contents = contents_of(&tree);
    assert!(contents.contains(&"A < B & \"C\"".to_string()));
    assert!(contents.contains(&"x < y & 'z' > \"w\"".to_string()));
    assert!(contents.contains(&"a&b".to_string()));
}

// PNGは、SVGと同じ大きさに倍率を掛けた画像になる
#[test]
fn the_png_is_the_svg_size_times_the_scale() {
    let outline = "- Activity
  - Task";
    let size = tree_of(&svg_of(outline)).size();
    let options = ImageOptions {
        scale: 2.0,
        ..IMAGE_OPTIONS.clone()
    };

    let png = Pixmap::decode_png(&render_png(&parse(outline), &options)).unwrap();

    assert_eq!(png.width(), (size.width() * 2.0).ceil() as u32);
    assert_eq!(png.height(), (size.height() * 2.0).ceil() as u32);
    // 背景は白で塗られる
    let corner = png.pixel(0, 0).unwrap();
    assert_eq!(
        (corner.red(), corner.green(), corner.blue(), corner.alpha()),
        (255, 255, 255, 255)
    );
}

// ノートの中の改行は、空白として描かれる
#[test]
fn line_breaks_in_a_note_are_drawn_as_spaces() {
    // 行末のバックスラッシュは、Markdownの改行である
    let tree = tree_of(&svg_of("Hard\\\nbreak and soft\nbreak\n\n- Activity"));

    assert!(contents_of(&tree).contains(&"Hard break and soft break".to_string()));
}

// One item of an outline: its depth, marker, checkbox, text, and tags
fn outline_item() -> impl Strategy<Value = (usize, &'static str, &'static str, String, Vec<String>)>
{
    (
        0..5usize,
        prop::sample::select(vec!["", "^ ", "v ", "vv "]),
        prop::sample::select(vec!["", "[ ] ", "[x] "]),
        "[a-zA-Z0-9 あいうえお漢字*`_&<>]{1,30}",
        prop::collection::vec("[a-z0-9]{1,8}", 0..3),
    )
}

// A bullet list where each item is at most one level deeper than the item above
fn outline_of(items: &[(usize, &str, &str, String, Vec<String>)]) -> String {
    let mut depth = 0;
    let mut lines = Vec::new();
    for (wanted, marker, checkbox, text, tags) in items {
        depth = (*wanted).min(depth + 1);
        let tags: String = tags.iter().map(|tag| format!(" #{tag}")).collect();
        lines.push(format!(
            "{}- {checkbox}{marker}{}{tags}",
            "  ".repeat(depth),
            text.trim()
        ));
        depth += 1;
    }
    lines.join("\n")
}

fn inside(canvas: Rect, inner: Rect) -> bool {
    canvas.left() <= inner.left()
        && canvas.top() <= inner.top()
        && inner.right() <= canvas.right()
        && inner.bottom() <= canvas.bottom()
}

proptest! {
    // Each case lays out a whole map and searches fallback fonts, so the cases are fewer than the default
    #![proptest_config(ProptestConfig::with_cases(64))]

    // どんなアウトラインでも、すべてのカードと文字が画像の中に収まる
    #[test]
    fn every_card_and_text_fits_in_the_image(
        items in prop::collection::vec(outline_item(), 1..20)
    ) {
        let outline = outline_of(&items);
        let map = parse(&outline);
        let tree = tree_of(&svg_of(&outline));
        let size = tree.size();
        let canvas = Rect::from_xywh(0.0, 0.0, size.width(), size.height()).unwrap();

        for index in 0..map.cells.len() {
            prop_assert!(inside(canvas, card_box(&tree, index)), "card {index} of\n{outline}");
        }
        for (text, text_box) in texts_of(&tree) {
            prop_assert!(inside(canvas, text_box), "text {text:?} of\n{outline}");
        }
    }
}

// 既定のフォントでは、どのOSでも本文とコードの書体が見つかる
#[test]
fn the_default_fonts_have_a_sans_serif_and_a_monospace_font() {
    use storymap::image::fontdb::{Family, Query};

    for family in [Family::SansSerif, Family::Monospace] {
        let query = Query {
            families: &[family],
            ..Query::default()
        };
        assert!(IMAGE_OPTIONS.fonts.query(&query).is_some(), "{family:?}");
    }
}
