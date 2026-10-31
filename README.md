# storymap

[![crates.io](https://img.shields.io/crates/v/storymap.svg)](https://crates.io/crates/storymap)

Turn a Markdown outline into a user story map.

This crate is the core of the VSCode extension [Story Mapping](https://github.com/sosuisen/vscode-user-story-mapping).
It reads the outline and lays out the cards on a grid.
The result holds only data, so you can draw the map in your own way.
With the `html` feature, the crate also renders the map as HTML.
With the `image` feature, it renders the map as an SVG or PNG image.

## Outline Format

The items of the first bullet list are the activities.
Their children go one level further down, and so on.

```markdown
# My Map

1. Activities
2. Tasks
3. Details

- Activity A
  - [x] Task A1 #done
    - Detail A1a
  - ^ Task A2
- Activity B
  - v Task B1
```

- The first heading is the title of the map.
- The first ordered list gives the titles of the levels from the top. It is optional.
- `[x]` and `[ ]` mark a card as done or open.
- Trailing hashtags such as `#done` are tags of the card.
- `^ ` keeps a child on the level of its parent, one row further down.
- `v ` skips one level. `vv ` skips two, and so on.
- An item that starts with `//` is a memo. It and its children make no cards.
- A paragraph above the list is a note above the map. A paragraph below the list is a note below the map.
- With a `Story Map` heading, only the section under that heading is read.

See the README of the extension for the full syntax.

## Usage

Add the crate to `Cargo.toml`:

```toml
[dependencies]
storymap = "0.3"
```

Read an outline as a `StoryMap`:

```rust
let map = storymap::parse("- Activity A\n  - Task A1");

for cell in &map.cells {
    println!("{} at level {}, column {}", cell.card.text, cell.level, cell.column);
}
```

The texts in a `StoryMap` stay raw Markdown.
Render them in the way that fits your app.

### HTML

The `html` feature is on by default:

```rust
use storymap::html::{map_css, render_map_content};

let map = storymap::parse("- Activity A\n  - Task A1");
let page = format!("<style>{}</style><div class=\"map-zoom\">{}</div>", map_css(), render_map_content(&map));
```

`inline_html` renders one piece of inline Markdown, such as a card text.
Raw HTML in the text is escaped.

A card is up to 240px wide, and a longer text wraps onto more lines.
To change the width, set the CSS variable `--card-max-width` on the page:

```css
:root { --card-max-width: 320px; }
```

If you do not need HTML, turn the feature off:

```toml
storymap = { version = "0.3", default-features = false }
```

### Image

The `image` feature is off by default. Turn it on:

```toml
storymap = { version = "0.3", features = ["image"] }
```

Render the map as an SVG document or a PNG image:

```rust
use storymap::image::{ImageOptions, render_png, render_svg};

let map = storymap::parse("- Activity A\n  - Task A1");
let options = ImageOptions { scale: 2.0, ..ImageOptions::default() };
let svg: String = render_svg(&map, &options);
let png: Vec<u8> = render_png(&map, &options);
```

The image follows the look of the HTML output.
A card is up to `ImageOptions::max_card_width` wide, which is 240 by default.
A longer text wraps onto more lines.
Bold, italic, and code get their fonts. Links become plain text.

The text is measured and drawn with the fonts in `ImageOptions::fonts`.
The default options load the fonts of the OS.
To get the same image on every machine, load your own fonts into a `fontdb::Database`.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Release

The crate is published on [crates.io](https://crates.io/crates/storymap).

1. Set the `version` in `Cargo.toml`, and merge the change into `main`.
2. Tag the commit as `v<version>`, such as `v0.2.1`, and push the tag.
3. Run `cargo publish` on the tagged commit.

The tag and the `version` in `Cargo.toml` must be the same.
A published version cannot be deleted or replaced, so run `cargo publish --dry-run` first.

## License

MIT
