use nom::Parser;
use nom::{
    bytes::complete::{tag_no_case, take_till1},
    character::complete::{space0, space1},
    IResult, Input as _,
};

use super::{
    combinator::{
        blank_lines, eol_or_eof, line_starts_iter, node, trim_line_end, GreenElement, NodeBuilder,
    },
    element::element_nodes,
    input::Input,
    SyntaxKind::*,
};

fn dyn_block_node_base(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, begin) = dyn_block_begin_node(input)?;
    let (input, pre_blank) = blank_lines(input)?;

    for (input, contents) in line_starts_iter(input.as_str()).map(|i| input.take_split(i)) {
        if let Ok((input, end)) = dyn_block_end_node(input) {
            let (input, post_blank) = blank_lines(input)?;
            let mut children = vec![begin];
            children.extend(pre_blank);
            if contents.is_empty() {
                children.push(node(BLOCK_CONTENT, []));
            } else {
                children.push(node(BLOCK_CONTENT, element_nodes(contents)?));
            }
            children.push(end);
            children.extend(post_blank);

            return Ok((input, node(DYN_BLOCK, children)));
        }
    }

    Err(nom::Err::Error(()))
}

fn dyn_block_begin_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, (ws, begin, ws_, name, (args, ws__, nl))) = (
        space0,
        tag_no_case("#+BEGIN:"),
        space1,
        take_till1(|c: char| c.is_ascii_whitespace()),
        trim_line_end,
    )
        .parse(input)?;

    let mut b = NodeBuilder::new();
    b.ws(ws);
    b.text(begin);
    b.ws(ws_);
    b.text(name);
    b.text(args);
    b.ws(ws__);
    b.nl(nl);

    Ok((input, b.finish(DYN_BLOCK_BEGIN)))
}

fn dyn_block_end_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, (ws, end, ws_, nl)) =
        (space0, tag_no_case("#+END:"), space0, eol_or_eof).parse(input)?;

    let mut b = NodeBuilder::new();
    b.ws(ws);
    b.text(end);
    b.ws(ws_);
    b.nl(nl);

    Ok((input, b.finish(DYN_BLOCK_END)))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn dyn_block_node(input: Input) -> IResult<Input, GreenElement, ()> {
    crate::lossless_parser!(dyn_block_node_base, input)
}

#[test]
fn parse() {
    use crate::{ast::DynBlock, tests::to_ast};

    let to_dyn_block = to_ast::<DynBlock>(dyn_block_node);

    insta::assert_debug_snapshot!(
        to_dyn_block(
            r#"#+BEGIN: clocktable :scope file

CONTENTS
#+END:
    "#).syntax,
        @r##"
    DYN_BLOCK@0..53
      DYN_BLOCK_BEGIN@0..32
        TEXT@0..8 "#+BEGIN:"
        WHITESPACE@8..9 " "
        TEXT@9..19 "clocktable"
        TEXT@19..31 " :scope file"
        NEW_LINE@31..32 "\n"
      BLANK_LINE@32..33 "\n"
      BLOCK_CONTENT@33..42
        PARAGRAPH@33..42
          TEXT@33..42 "CONTENTS\n"
      DYN_BLOCK_END@42..49
        TEXT@42..48 "#+END:"
        NEW_LINE@48..49 "\n"
      BLANK_LINE@49..53 "    "
    "##
    );
}

#[test]
fn dyn_block_element() {
    use crate::syntax::{combinator::node, element::element_nodes, SyntaxKind, SyntaxNode};
    use crate::ParseConfig;

    let t = |input: &str| {
        let config = &ParseConfig::default();
        let children = element_nodes((input, config).into()).unwrap();
        SyntaxNode::new_root(node(SyntaxKind::SECTION, children).into_node().unwrap())
    };

    // a dynamic block, not a `#+BEGIN:` keyword, with any name and with
    // affiliated keywords
    insta::assert_debug_snapshot!(
        t("#+name: d\n#+BEGIN: clock-table :scope file\n| a |\n#+END:\n"),
        @r##"
    SECTION@0..56
      DYN_BLOCK@0..56
        AFFILIATED_KEYWORD@0..10
          HASH_PLUS@0..2 "#+"
          TEXT@2..6 "name"
          COLON@6..7 ":"
          TEXT@7..9 " d"
          NEW_LINE@9..10 "\n"
        DYN_BLOCK_BEGIN@10..43
          TEXT@10..18 "#+BEGIN:"
          WHITESPACE@18..19 " "
          TEXT@19..30 "clock-table"
          TEXT@30..42 " :scope file"
          NEW_LINE@42..43 "\n"
        BLOCK_CONTENT@43..49
          ORG_TABLE@43..49
            ORG_TABLE_STANDARD_ROW@43..49
              PIPE@43..44 "|"
              WHITESPACE@44..45 " "
              ORG_TABLE_CELL@45..46
                TEXT@45..46 "a"
              WHITESPACE@46..47 " "
              PIPE@47..48 "|"
              WHITESPACE@48..49 "\n"
        DYN_BLOCK_END@49..56
          TEXT@49..55 "#+END:"
          NEW_LINE@55..56 "\n"
    "##
    );
}

#[test]
fn dyn_block_content() {
    use crate::{ast::DynBlock, ParseConfig};
    use rowan::ast::AstNode;

    let t = |input: &str| {
        let org = ParseConfig::default().parse(input);
        let block = org
            .document()
            .syntax()
            .descendants()
            .find_map(DynBlock::cast)
            .unwrap();
        (block, org.to_org() == input)
    };

    // the content is parsed into elements, as in Org
    let (block, same) =
        t("#+BEGIN: view :status \"DONE\"\n| a | b |\n|---+---|\n| 1 | 2 |\n\n- item\n#+END:\n");
    assert!(same);
    insta::assert_debug_snapshot!(block.syntax, @r##"
    DYN_BLOCK@0..74
      DYN_BLOCK_BEGIN@0..29
        TEXT@0..8 "#+BEGIN:"
        WHITESPACE@8..9 " "
        TEXT@9..13 "view"
        TEXT@13..28 " :status \"DONE\""
        NEW_LINE@28..29 "\n"
      BLOCK_CONTENT@29..67
        ORG_TABLE@29..60
          ORG_TABLE_STANDARD_ROW@29..39
            PIPE@29..30 "|"
            WHITESPACE@30..31 " "
            ORG_TABLE_CELL@31..32
              TEXT@31..32 "a"
            WHITESPACE@32..33 " "
            PIPE@33..34 "|"
            WHITESPACE@34..35 " "
            ORG_TABLE_CELL@35..36
              TEXT@35..36 "b"
            WHITESPACE@36..37 " "
            PIPE@37..38 "|"
            WHITESPACE@38..39 "\n"
          ORG_TABLE_RULE_ROW@39..49
            TEXT@39..49 "|---+---|\n"
          ORG_TABLE_STANDARD_ROW@49..59
            PIPE@49..50 "|"
            WHITESPACE@50..51 " "
            ORG_TABLE_CELL@51..52
              TEXT@51..52 "1"
            WHITESPACE@52..53 " "
            PIPE@53..54 "|"
            WHITESPACE@54..55 " "
            ORG_TABLE_CELL@55..56
              TEXT@55..56 "2"
            WHITESPACE@56..57 " "
            PIPE@57..58 "|"
            WHITESPACE@58..59 "\n"
          BLANK_LINE@59..60 "\n"
        LIST@60..67
          LIST_ITEM@60..67
            LIST_ITEM_INDENT@60..60 ""
            LIST_ITEM_BULLET@60..62 "- "
            LIST_ITEM_CONTENT@62..67
              PARAGRAPH@62..67
                TEXT@62..67 "item\n"
      DYN_BLOCK_END@67..74
        TEXT@67..73 "#+END:"
        NEW_LINE@73..74 "\n"
    "##);
    assert_eq!(u32::from(block.content_start()), 29);
    assert_eq!(u32::from(block.content_end()), 67);

    let (block, same) = t("#+BEGIN: empty\n#+END:\n");
    assert!(same);
    insta::assert_debug_snapshot!(block.syntax, @r##"
    DYN_BLOCK@0..22
      DYN_BLOCK_BEGIN@0..15
        TEXT@0..8 "#+BEGIN:"
        WHITESPACE@8..9 " "
        TEXT@9..14 "empty"
        NEW_LINE@14..15 "\n"
      BLOCK_CONTENT@15..15
      DYN_BLOCK_END@15..22
        TEXT@15..21 "#+END:"
        NEW_LINE@21..22 "\n"
    "##);
    assert_eq!(block.content_start(), block.content_end());
}
