use nom::Parser;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_till, take_till1},
    character::complete::{space0, space1},
    IResult,
};

use super::{
    combinator::{eol_or_eof, GreenElement, NodeBuilder},
    input::Input,
    timestamp::{timestamp_active_node, timestamp_inactive_node},
    SyntaxKind::*,
};

pub fn planning_node(input: Input) -> IResult<Input, GreenElement, ()> {
    debug_assert!(!input.is_empty());
    crate::lossless_parser!(planning_node_base, input)
}

fn planning_keyword(input: Input) -> IResult<Input, Input, ()> {
    alt((tag("DEADLINE:"), tag("SCHEDULED:"), tag("CLOSED:"))).parse(input)
}

fn planning_node_base(input: Input) -> IResult<Input, GreenElement, ()> {
    // like Org, any line starting with a planning keyword is a planning line:
    // a keyword followed by a timestamp anywhere on it sets that keyword, and
    // any other text on the line is ignored
    (space0, planning_keyword).parse(input)?;

    let (input, line) = take_till(|c| c == '\n' || c == '\r').parse(input)?;
    let (input, nl) = eol_or_eof(input)?;

    let mut b = NodeBuilder::new();
    let mut i = line;

    while !i.is_empty() {
        if let Ok((rest, (ws, keyword, ws_, timestamp))) = (
            space0,
            planning_keyword,
            space0,
            alt((timestamp_active_node, timestamp_inactive_node)),
        )
            .parse(i)
        {
            let mut b_ = NodeBuilder::new();
            b_.ws(ws);
            b_.text(keyword);
            b_.ws(ws_);
            b_.push(timestamp);
            b.push(b_.finish(match keyword.as_str() {
                "DEADLINE:" => PLANNING_DEADLINE,
                "SCHEDULED:" => PLANNING_SCHEDULED,
                "CLOSED:" => PLANNING_CLOSED,
                _ => unreachable!(),
            }));
            i = rest;
        } else if let Ok((rest, ws)) = space1::<_, ()>(i) {
            b.ws(ws);
            i = rest;
        } else {
            let (rest, word) = take_till1(|c| c == ' ' || c == '\t').parse(i)?;
            b.text(word);
            i = rest;
        }
    }

    b.nl(nl);

    Ok((input, b.finish(PLANNING)))
}

#[test]
fn prase() {
    use crate::{ast::Planning, tests::to_ast, ParseConfig};

    let to_planning = to_ast::<Planning>(planning_node);

    insta::assert_debug_snapshot!(
        to_planning("SCHEDULED: <2019-04-08 Mon>").syntax,
        @r###"
    PLANNING@0..27
      PLANNING_SCHEDULED@0..27
        TEXT@0..10 "SCHEDULED:"
        WHITESPACE@10..11 " "
        TIMESTAMP_ACTIVE@11..27
          L_ANGLE@11..12 "<"
          TIMESTAMP_YEAR@12..16 "2019"
          MINUS@16..17 "-"
          TIMESTAMP_MONTH@17..19 "04"
          MINUS@19..20 "-"
          TIMESTAMP_DAY@20..22 "08"
          WHITESPACE@22..23 " "
          TIMESTAMP_DAYNAME@23..26 "Mon"
          R_ANGLE@26..27 ">"
    "###
    );

    let config = &ParseConfig::default();

    assert!(planning_node(("   ", config).into()).is_err());
    // like Org, a line starting with a planning keyword is a planning line even
    // without a timestamp
    assert!(planning_node((" SCHEDULED:   ", config).into()).is_ok());
    assert!(planning_node(("junk SCHEDULED: <2019-04-08 Mon>", config).into()).is_err());

    let planning = to_planning("CLOSED: [2026-10-20] foo SCHEDULED: <2026-10-22> bar\n");
    assert!(planning.closed().is_some());
    assert!(planning.scheduled().is_some());

    insta::assert_debug_snapshot!(
        to_planning("SCHEDULED: <2026-10-20 Tue> trailing words\n").syntax,
        @r#"
    PLANNING@0..43
      PLANNING_SCHEDULED@0..27
        TEXT@0..10 "SCHEDULED:"
        WHITESPACE@10..11 " "
        TIMESTAMP_ACTIVE@11..27
          L_ANGLE@11..12 "<"
          TIMESTAMP_YEAR@12..16 "2026"
          MINUS@16..17 "-"
          TIMESTAMP_MONTH@17..19 "10"
          MINUS@19..20 "-"
          TIMESTAMP_DAY@20..22 "20"
          WHITESPACE@22..23 " "
          TIMESTAMP_DAYNAME@23..26 "Tue"
          R_ANGLE@26..27 ">"
      WHITESPACE@27..28 " "
      TEXT@28..36 "trailing"
      WHITESPACE@36..37 " "
      TEXT@37..42 "words"
      NEW_LINE@42..43 "\n"
    "#
    );
}
