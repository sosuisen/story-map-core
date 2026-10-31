// アウトラインの読み取りとレイアウト
// Reading the outline and laying out the cards

use storymap::{Card, Cell, parse};

fn texts(cells: &[Cell]) -> Vec<&str> {
    cells.iter().map(|cell| cell.card.text.as_str()).collect()
}

fn cell<'a>(cells: &'a [Cell], text: &str) -> &'a Cell {
    cells.iter().find(|cell| cell.card.text == text).unwrap()
}

// アクティビティが、アウトラインの順でレベル0に横一列に並ぶ
#[test]
fn activities_are_on_level_0_in_outline_order() {
    let map = parse("- Activity A\n- Activity B\n- Activity C");

    assert_eq!(
        texts(&map.cells),
        ["Activity A", "Activity B", "Activity C"]
    );
    let positions: Vec<_> = map
        .cells
        .iter()
        .map(|cell| (cell.level, cell.column))
        .collect();
    assert_eq!(positions, [(0, 0), (0, 1), (0, 2)]);
    assert_eq!(map.column_count, 3);
}

// 子は親の1つ下のレベルに、親と同じ列で並ぶ。兄弟は右の列に並ぶ
#[test]
fn children_go_one_level_down_and_siblings_go_right() {
    let map = parse("- Activity A\n\t- Task A1\n\t\t- Task A2\n\t- Task A3\n- Activity B");

    assert_eq!(
        (
            cell(&map.cells, "Task A1").level,
            cell(&map.cells, "Task A1").column
        ),
        (1, 0)
    );
    assert_eq!(
        (
            cell(&map.cells, "Task A2").level,
            cell(&map.cells, "Task A2").column
        ),
        (2, 0)
    );
    assert_eq!(
        (
            cell(&map.cells, "Task A3").level,
            cell(&map.cells, "Task A3").column
        ),
        (1, 1)
    );
    // アクティビティの幅は子の列の数になる
    assert_eq!(cell(&map.cells, "Activity B").column, 2);
}

// スペースのインデントも、タブと同じに読む
#[test]
fn space_indent_is_read_the_same_as_tab_indent() {
    let map = parse("- Activity A\n  - Task A1\n    - Task A2");

    assert_eq!(cell(&map.cells, "Task A2").level, 2);
}

// チェックボックスは、本文から外れて done・todo になる
#[test]
fn checkboxes_become_done_and_todo() {
    let map = parse("- A\n\t- [x] Done\n\t- [X] Done too\n\t- [ ] Open");

    let done = &cell(&map.cells, "Done").card;
    assert!(done.done && !done.todo);
    assert!(cell(&map.cells, "Done too").card.done);
    let open = &cell(&map.cells, "Open").card;
    assert!(open.todo && !open.done);
}

// 末尾のハッシュタグは、「#」なしのタグになる
#[test]
fn trailing_hashtags_become_tags() {
    let map = parse("- Activity A #tag1 #tag2");

    assert_eq!(
        map.cells[0].card,
        Card {
            text: "Activity A".to_string(),
            done: false,
            todo: false,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        }
    );
}

// 「^」の子は親と同じレベルの次の行に、「v」の子は1レベルあけて置かれる
#[test]
fn caret_and_v_markers_move_the_child_between_levels() {
    let map = parse("- A\n\t- Task\n\t\t- ^ Same level\n\t\t- v Skip one\n\t\t- [ ] vv Skip two");

    let same = cell(&map.cells, "Same level");
    assert_eq!((same.level, same.row_in_level), (1, 1));
    assert_eq!(cell(&map.cells, "Skip one").level, 3);
    let skip_two = cell(&map.cells, "Skip two");
    assert_eq!(skip_two.level, 4);
    assert!(skip_two.card.todo);
    // レベル1の帯は2行になり、飛ばしたレベルも数に入る
    assert_eq!(map.rows_by_level, [1, 2, 1, 1, 1]);
}

// 空白の続かない「^」や「v」は、マーカーではない
#[test]
fn a_marker_without_a_space_is_text() {
    let map = parse("- A\n\t- ^Task\n\t- vsCode");

    assert_eq!(texts(&map.cells), ["A", "^Task", "vsCode"]);
}

// 「//」の項目はメモで、その子もカードにならない
#[test]
fn memo_items_and_their_children_make_no_cards() {
    let map = parse("- A\n\t- // memo\n\t\t- child\n\t- ^ // memo too\n\t- Task");

    assert_eq!(texts(&map.cells), ["A", "Task"]);
    assert_eq!(cell(&map.cells, "Task").column, 0);
}

// 「-」だけの行は、空のカードになる
#[test]
fn a_dash_only_line_is_a_blank_card() {
    let map = parse("- A\n\t- Task\n\t\t-\n- B");

    assert_eq!(texts(&map.cells), ["A", "Task", "", "B"]);
    assert_eq!(map.cells[2].level, 2);
}

// 最初の見出しがタイトルになり、リストの上下の段落はノートになる
#[test]
fn the_first_heading_is_the_title_and_paragraphs_are_notes() {
    let map = parse("# Map **Title**\n\nAbove *it*.\n\n- A\n\nBelow.");

    assert_eq!(map.title, "Map **Title**");
    assert_eq!(map.leading_notes, ["Above *it*."]);
    assert_eq!(map.trailing_notes, ["Below."]);
}

// 見出しがなければ、タイトルは空になる
#[test]
fn the_title_is_empty_without_a_heading() {
    assert_eq!(parse("- A").title, "");
}

// 最初の順序付きリストは、レベルのタイトルになる。空の項目はタイトルなしのレベルになる
#[test]
fn the_first_ordered_list_gives_the_level_titles() {
    let map = parse("1. Activities\n2.\n3. Details\n\n- A");

    assert_eq!(map.level_titles, ["Activities", "", "Details"]);
}

// 「Story Map」見出しがあれば、その節だけを読む。タイトルは文書の最初の見出しのまま
#[test]
fn only_the_story_map_section_is_read() {
    let map = parse(
        "# Design\n\n- Before\n\n## story   MAP\n\nNote.\n\n- Inside\n\n## Appendix\n\n- After",
    );

    assert_eq!(map.title, "Design");
    assert_eq!(map.leading_notes, ["Note."]);
    assert_eq!(texts(&map.cells), ["Inside"]);
}

// 改行コードがCRLFでも、LFと同じに読む
#[test]
fn crlf_is_read_the_same_as_lf() {
    assert_eq!(
        parse("- A\r\n\t- Task\r\n\t\t-"),
        parse("- A\n\t- Task\n\t\t-")
    );
}
