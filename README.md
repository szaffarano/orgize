# Orgize

[![Crates.io](https://img.shields.io/crates/v/orgize-ng.svg)](https://crates.io/crates/orgize-ng)
[![Documentation](https://docs.rs/orgize-ng/badge.svg)](https://docs.rs/orgize-ng)
[![Build status](https://img.shields.io/github/actions/workflow/status/szaffarano/orgize/ci.yml)](https://github.com/szaffarano/orgize/actions/workflows/ci.yml)
![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)

A Rust library for parsing org-mode files.

Live Demo: <https://szaffarano.github.io/orgize/>

## About this fork

This is a maintained fork of [PoiScript/orgize](https://github.com/PoiScript/orgize),
which has had no recent activity. Many thanks to PoiScript for creating it. The goal here is
to keep the crate alive: updated dependencies, CI, and bug fixes (including some
pending upstream pull requests and issues).

It is published as [`orgize-ng`](https://crates.io/crates/orgize-ng), while the library is
still named `orgize`, so existing code keeps working:

```toml
[dependencies]
orgize = { package = "orgize-ng", version = "0.10.0-alpha.11" }
```

If the upstream project becomes active again, this fork can be retired or merged back.

## Parse

Parse an org-mode string with `Org::parse`:

```rust
use orgize::{Org, rowan::ast::AstNode};

let org = Org::parse("* DONE Title :tag:");
assert_eq!(
    format!("{:#?}", org.document().syntax()),
    r#"DOCUMENT@0..18
  HEADLINE@0..18
    HEADLINE_STARS@0..1 "*"
    WHITESPACE@1..2 " "
    HEADLINE_KEYWORD_DONE@2..6 "DONE"
    WHITESPACE@6..7 " "
    HEADLINE_TITLE@7..13
      TEXT@7..13 "Title "
    HEADLINE_TAGS@13..18
      COLON@13..14 ":"
      TEXT@14..17 "tag"
      COLON@17..18 ":"
"#);
```

Use `ParseConfig::parse` for a custom parse config:

```rust
use orgize::{Org, ParseConfig, ast::Headline};

let config = ParseConfig {
    // custom todo keywords
    todo_keywords: (vec!["TASK".to_string()], vec![]),
    ..Default::default()
};
let org = config.parse("* TASK Title 1");
let hdl = org.first_node::<Headline>().unwrap();
assert_eq!(hdl.todo_keyword().unwrap(), "TASK");
```

## Traverse

Use `org.traverse(&mut traversal)` to walk through the syntax tree.

```rust
use orgize::{
    export::{from_fn, Container, Event},
    Org,
};

let mut hdl_count = 0;
let mut handler = from_fn(|event| {
    if matches!(event, Event::Enter(Container::Headline(_))) {
        hdl_count += 1;
    }
});
Org::parse("* 1\n** 2\n*** 3\n****4").traverse(&mut handler);
assert_eq!(hdl_count, 3);
```

## Modify

Use `org.replace_range(TextRange::new(start, end), "new_text")` to modify the syntax tree:

```rust
use orgize::{Org, ParseConfig, ast::Headline, TextRange};

let mut org = Org::parse("hello\n* world");

let hdl = org.first_node::<Headline>().unwrap();
org.replace_range(hdl.text_range(), "** WORLD!");

let hdl = org.first_node::<Headline>().unwrap();
assert_eq!(hdl.level(), 2);

org.replace_range(TextRange::up_to(hdl.start()), "");
assert_eq!(org.to_org(), "** WORLD!");
```

## Render to html

Use `Org::to_html` to export the syntax tree to HTML:

```rust
use orgize::Org;

assert_eq!(
    Org::parse("* title\n*section*").to_html(),
    "<main><h1>title</h1><section><p><b>section</b></p></section></main>"
);
```

See `examples/html-slugify.rs` for how to customize the HTML export.

## Features

- **`datetime`**: adds the ability to convert `Timestamp` into `jiff::civil::DateTime`, disabled by default.

- **`indexmap`**: adds the ability to convert `PropertyDrawer` properties into `IndexMap`, disabled by default.

## API compatibility

`element.syntax()` exposes the internal syntax tree and some low-level rowan APIs, which is useful for intricate tasks.

The tree structure can change between versions, so `element.syntax()` is not covered by semantic
versioning: updates might break code that relies on it.
