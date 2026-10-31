// マップのHTML描画
// Rendering the map as HTML

#![cfg(feature = "html")]

use storymap::html::{inline_html, map_css, render_map_content};
use storymap::parse;

// カードは、帯の行と列に置かれる。レベルのタイトルがあれば、カードは2列目から始まる
#[test]
fn cards_are_placed_on_the_rows_and_columns_of_the_bands() {
    let html = render_map_content(&parse("1. Activities\n\n- A\n\t- [x] Task #t"));

    assert!(html.contains(
        r#"<div class="row-label" style="grid-column: 1; grid-row: 1;">Activities</div>"#
    ));
    assert!(
        html.contains(r#"<div class="card level1" style="grid-column: 2; grid-row: 1;">A</div>"#)
    );
    assert!(html.contains(
        r#"<div class="card level2 done" style="grid-column: 2; grid-row: 2;">✅ Task<span class="tag">t</span></div>"#
    ));
    assert!(html.contains(
        r#"<div class="row-band level1" style="grid-column: 1 / 3; grid-row: 1;"></div>"#
    ));
}

// スタイルには、レベルごとの色が入る
#[test]
fn the_css_has_the_colors_of_each_level() {
    let css = map_css();

    assert!(css.contains("--level1-color: #e0efff"));
    assert!(css.contains(".card.level4 {"));
}

// カードには幅の上限（既定160px）がつき、`--card-max-width`で変えられる。長い語も折り返す
#[test]
fn cards_have_a_maximum_width_that_a_variable_can_change() {
    let css = map_css();

    assert!(css.contains("max-width: var(--card-max-width, 160px);"));
    assert!(css.contains("overflow-wrap: anywhere;"));
}

// 同じ行のカードは、行の高さまで伸びて、同じ高さになる
#[test]
fn cards_stretch_to_the_height_of_their_row() {
    let css = map_css();

    let card_rule = css
        .lines()
        .find(|line| line.starts_with(".card {"))
        .unwrap();
    assert!(card_rule.contains("align-self: stretch;"));
}

// インラインのMarkdownは、markdown-itと同じHTMLになる
#[test]
fn inline_markdown_is_rendered_like_markdown_it() {
    assert_eq!(
        inline_html("**bold** ~~gone~~ `x<y`"),
        "<strong>bold</strong> <s>gone</s> <code>x&lt;y</code>"
    );
    assert_eq!(
        inline_html("[a](<http://e.com/ä b>) [b](/x%20y)"),
        r#"<a href="http://e.com/%C3%A4%20b">a</a> <a href="/x%20y">b</a>"#
    );
    assert_eq!(
        inline_html("![i](p.png \"t\")"),
        r#"<img src="p.png" alt="i" title="t">"#
    );
}

// 生のHTMLは、エスケープされて文字として表示される
#[test]
fn raw_html_is_escaped() {
    assert_eq!(
        inline_html("<b>x</b> & \"q\""),
        "&lt;b&gt;x&lt;/b&gt; &amp; &quot;q&quot;"
    );
}

// 行頭のブロック記法は、文字のまま表示される
#[test]
fn block_syntax_at_the_start_stays_text() {
    assert_eq!(inline_html("# h"), "# h");
    assert_eq!(inline_html("1. one"), "1. one");
    assert_eq!(inline_html("> q"), "&gt; q");
}

// javascript: のリンクは、リンクにならない
#[test]
fn a_script_link_is_not_a_link() {
    assert_eq!(inline_html("[j](javascript:x)"), "[j](javascript:x)");
}
