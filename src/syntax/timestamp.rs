use nom::Parser;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_till, take_while1, take_while_m_n},
    character::complete::{digit1, space0, space1},
    combinator::{iterator, map, opt},
    IResult,
};

use super::{
    combinator::{
        colon_token, l_angle_token, l_bracket_token, l_parens_token, minus2_token, minus_token,
        node, percent2_token, r_angle_token, r_bracket_token, r_parens_token, slash_token,
        GreenElement, NodeBuilder,
    },
    input::Input,
    SyntaxKind::*,
};

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn timestamp_diary_node(input: Input) -> IResult<Input, GreenElement, ()> {
    let mut parser = map(
        (
            l_angle_token,
            percent2_token,
            l_parens_token,
            take_till(|c| c == ')' || c == '>' || c == '\n'),
            r_parens_token,
            r_angle_token,
        ),
        |(l_angle, percent2, l_paren, value, r_paren, r_angle)| {
            node(
                TIMESTAMP_DIARY,
                [
                    l_angle,
                    percent2,
                    l_paren,
                    value.text_token(),
                    r_paren,
                    r_angle,
                ],
            )
        },
    );
    crate::lossless_parser!(parser, input)
}

fn date(i: Input) -> IResult<Input, [GreenElement; 5], ()> {
    map(
        (
            take_while_m_n(4, 4, |c: char| c.is_ascii_digit()),
            minus_token,
            take_while_m_n(2, 2, |c: char| c.is_ascii_digit()),
            minus_token,
            take_while_m_n(2, 2, |c: char| c.is_ascii_digit()),
        ),
        |(year, minus, month, minus_, day)| {
            [
                year.token(TIMESTAMP_YEAR),
                minus,
                month.token(TIMESTAMP_MONTH),
                minus_,
                day.token(TIMESTAMP_DAY),
            ]
        },
    )
    .parse(i)
}

fn dayname(i: Input) -> IResult<Input, GreenElement, ()> {
    map(
        take_while1(|c: char| {
            !c.is_ascii_whitespace()
                && !c.is_ascii_digit()
                && c != '+'
                && c != '-'
                && c != ']'
                && c != '>'
                && c != '.'
        }),
        |i: Input| i.token(TIMESTAMP_DAYNAME),
    )
    .parse(i)
}

fn time(i: Input) -> IResult<Input, [GreenElement; 3], ()> {
    map(
        (
            take_while_m_n(1, 2, |c: char| c.is_ascii_digit()),
            colon_token,
            take_while_m_n(2, 2, |c: char| c.is_ascii_digit()),
        ),
        |(hour, colon, minute)| {
            [
                hour.token(TIMESTAMP_HOUR),
                colon,
                minute.token(TIMESTAMP_MINUTE),
            ]
        },
    )
    .parse(i)
}

fn value_and_unit(input: Input) -> IResult<Input, [GreenElement; 2], ()> {
    let (input, value) = digit1(input)?;
    let (input, unit) = alt((tag("h"), tag("d"), tag("w"), tag("m"), tag("y"))).parse(input)?;
    Ok((
        input,
        [value.token(TIMESTAMP_VALUE), unit.token(TIMESTAMP_UNIT)],
    ))
}

/// Recognizes a repeater (`+1w`, `++1d`, `.+2d/4d`) or a delay (`-1d`, `--2d`)
///
/// A repeater may carry a habit deadline (`/4d`) after its interval.
fn repeater_or_delay(input: Input) -> IResult<Input, Vec<GreenElement>, ()> {
    let is_repeater = !input.as_str().starts_with('-');
    let (input, mark) = alt((
        map(alt((tag("++"), tag("+"), tag(".+"))), |i: Input| {
            i.token(TIMESTAMP_REPEATER_MARK)
        }),
        map(alt((tag("--"), tag("-"))), |i: Input| {
            i.token(TIMESTAMP_DELAY_MARK)
        }),
    ))
    .parse(input)?;
    let (input, [value, unit]) = value_and_unit(input)?;

    let mut children = vec![mark, value, unit];

    if is_repeater {
        let (input, habit) = opt((slash_token, value_and_unit)).parse(input)?;
        if let Some((slash, [value, unit])) = habit {
            children.extend([slash, value, unit]);
        }
        return Ok((input, children));
    }

    Ok((input, children))
}

fn timestamp_node_base(
    input: Input,
    l_parser: impl Fn(Input) -> IResult<Input, GreenElement, ()>,
    r_parser: impl Fn(Input) -> IResult<Input, GreenElement, ()>,
) -> IResult<Input, Vec<GreenElement>, ()> {
    let (input, l_angle) = l_parser(input)?;
    let (input, start_date) = date(input)?;
    let (input, start_dayname) = opt((space1, dayname)).parse(input)?;
    let (input, start_time) = opt((space1, time)).parse(input)?;

    let mut b = NodeBuilder::new();
    b.push(l_angle);
    b.children.extend(start_date);

    if let Some((ws, dayname)) = start_dayname {
        b.push(ws.ws_token());
        b.push(dayname);
    }

    if input.as_str().starts_with('-') {
        let (ws, start_time) = match start_time {
            Some(start_time) => start_time,
            None => return Err(nom::Err::Error(())),
        };

        let (input, minus) = minus_token(input)?;
        let (input, end_time) = time(input)?;

        b.ws(ws);
        b.children.extend(start_time);
        b.push(minus);
        b.children.extend(end_time);

        let mut iter = iterator(input, (space1, repeater_or_delay));
        for (ws, items) in &mut iter {
            b.push(ws.ws_token());
            b.children.extend(items);
        }
        let (input, _) = iter.finish()?;

        let (input, space) = space0(input)?;
        let (input, r_angle) = r_parser(input)?;

        b.ws(space);
        b.push(r_angle);

        return Ok((input, b.children));
    }

    if let Some((ws, start_time)) = start_time {
        b.ws(ws);
        b.children.extend(start_time);
    }

    let mut iter = iterator(input, (space1, repeater_or_delay));
    for (ws, items) in &mut iter {
        b.push(ws.ws_token());
        b.children.extend(items);
    }
    let (input, _) = iter.finish()?;

    let (input, space) = space0(input)?;
    let (input, r_angle) = r_parser(input)?;

    b.ws(space);
    b.push(r_angle);

    if input.as_str().starts_with("--") {
        let (input, minus2) = minus2_token(input)?;
        let (input, l_angle) = l_parser(input)?;
        let (input, end_date) = date(input)?;
        let (input, end_dayname) = opt((space1, dayname)).parse(input)?;
        let (input, end_time) = opt((space1, time)).parse(input)?;

        b.children.extend([minus2, l_angle]);
        b.children.extend(end_date);
        if let Some((ws, dayname)) = end_dayname {
            b.push(ws.ws_token());
            b.push(dayname);
        }
        if let Some((ws, end_time)) = end_time {
            b.ws(ws);
            b.children.extend(end_time);
        }
        let mut iter = iterator(input, (space1, repeater_or_delay));
        for (ws, items) in &mut iter {
            b.push(ws.ws_token());
            b.children.extend(items);
        }
        let (input, _) = iter.finish()?;

        let (input, space_) = space0(input)?;
        let (input, r_angle) = r_parser(input)?;

        b.ws(space_);
        b.push(r_angle);

        Ok((input, b.children))
    } else {
        Ok((input, b.children))
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn timestamp_active_node(input: Input) -> IResult<Input, GreenElement, ()> {
    fn parser(input: Input) -> IResult<Input, GreenElement, ()> {
        let (input, children) = timestamp_node_base(input, l_angle_token, r_angle_token)?;
        Ok((input, node(TIMESTAMP_ACTIVE, children)))
    }
    crate::lossless_parser!(parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn timestamp_inactive_node(input: Input) -> IResult<Input, GreenElement, ()> {
    fn parser(input: Input) -> IResult<Input, GreenElement, ()> {
        let (input, children) = timestamp_node_base(input, l_bracket_token, r_bracket_token)?;
        Ok((input, node(TIMESTAMP_INACTIVE, children)))
    }
    crate::lossless_parser!(parser, input)
}

#[test]
fn parse() {
    use crate::{ast::Timestamp, tests::to_ast};

    let to_timestamp = to_ast::<Timestamp>(timestamp_inactive_node);

    to_timestamp("[2003-09-16]");
    to_timestamp("[2003-09-16 09:09]");
    to_timestamp("[2003-09-16 Tue]");
    to_timestamp("[2003-09-16 Tue 09:09]");
    to_timestamp("[2003-09-16]--[2003-09-16]");
    to_timestamp("[2003-09-16 09:09]--[2003-09-16 09:09]");
    to_timestamp("[2003-09-16]--[2003-09-16 09:09]");
    to_timestamp("[2003-09-16 Tue]--[2003-09-16 Tue]");
    to_timestamp("[2003-09-16 Tue 09:09]--[2003-09-16 Tue 09:09]");
    to_timestamp("[2003-09-16 Tue 09:09-09:09]");
    to_timestamp("[2003-09-16 09:09-09:09 ]");
    to_timestamp("[2003-09-16 09:09 +1w .+1d]");
    to_timestamp("[2003-09-16 09:09]--[2003-09-16  +1w .+1d --1d ]");
    to_timestamp("[2003-09-16 Tue 09:09 +1w]--[2003-09-16 .+1d --1d ]");
    to_timestamp("[2003-09-16 09:09-10:19 +1w --1d]");

    let ts = to_timestamp("[2003-09-16 Tue +1w]");
    assert!(!ts.is_range());
    insta::assert_debug_snapshot!(
        ts.syntax,
        @r###"
    TIMESTAMP_INACTIVE@0..20
      L_BRACKET@0..1 "["
      TIMESTAMP_YEAR@1..5 "2003"
      MINUS@5..6 "-"
      TIMESTAMP_MONTH@6..8 "09"
      MINUS@8..9 "-"
      TIMESTAMP_DAY@9..11 "16"
      WHITESPACE@11..12 " "
      TIMESTAMP_DAYNAME@12..15 "Tue"
      WHITESPACE@15..16 " "
      TIMESTAMP_REPEATER_MARK@16..17 "+"
      TIMESTAMP_VALUE@17..18 "1"
      TIMESTAMP_UNIT@18..19 "w"
      R_BRACKET@19..20 "]"
    "###
    );

    let ts = to_timestamp("[2003-09-16 Tue 09:39]--[2003-09-16 Tue 10:39]");
    assert!(ts.is_range());
    insta::assert_debug_snapshot!(
        ts.syntax,
        @r###"
    TIMESTAMP_INACTIVE@0..46
      L_BRACKET@0..1 "["
      TIMESTAMP_YEAR@1..5 "2003"
      MINUS@5..6 "-"
      TIMESTAMP_MONTH@6..8 "09"
      MINUS@8..9 "-"
      TIMESTAMP_DAY@9..11 "16"
      WHITESPACE@11..12 " "
      TIMESTAMP_DAYNAME@12..15 "Tue"
      WHITESPACE@15..16 " "
      TIMESTAMP_HOUR@16..18 "09"
      COLON@18..19 ":"
      TIMESTAMP_MINUTE@19..21 "39"
      R_BRACKET@21..22 "]"
      MINUS2@22..24 "--"
      L_BRACKET@24..25 "["
      TIMESTAMP_YEAR@25..29 "2003"
      MINUS@29..30 "-"
      TIMESTAMP_MONTH@30..32 "09"
      MINUS@32..33 "-"
      TIMESTAMP_DAY@33..35 "16"
      WHITESPACE@35..36 " "
      TIMESTAMP_DAYNAME@36..39 "Tue"
      WHITESPACE@39..40 " "
      TIMESTAMP_HOUR@40..42 "10"
      COLON@42..43 ":"
      TIMESTAMP_MINUTE@43..45 "39"
      R_BRACKET@45..46 "]"
    "###
    );

    let ts = to_timestamp("[2003-09-16 Tue 09:39-10:39]");
    assert!(ts.is_range());
    insta::assert_debug_snapshot!(
        ts.syntax,
        @r###"
    TIMESTAMP_INACTIVE@0..28
      L_BRACKET@0..1 "["
      TIMESTAMP_YEAR@1..5 "2003"
      MINUS@5..6 "-"
      TIMESTAMP_MONTH@6..8 "09"
      MINUS@8..9 "-"
      TIMESTAMP_DAY@9..11 "16"
      WHITESPACE@11..12 " "
      TIMESTAMP_DAYNAME@12..15 "Tue"
      WHITESPACE@15..16 " "
      TIMESTAMP_HOUR@16..18 "09"
      COLON@18..19 ":"
      TIMESTAMP_MINUTE@19..21 "39"
      MINUS@21..22 "-"
      TIMESTAMP_HOUR@22..24 "10"
      COLON@24..25 ":"
      TIMESTAMP_MINUTE@25..27 "39"
      R_BRACKET@27..28 "]"
    "###
    );
}

#[test]
fn one_digit_hour() {
    use crate::{ast::Timestamp, tests::to_ast};

    let to_timestamp = to_ast::<Timestamp>(timestamp_active_node);

    let ts = to_timestamp("<2026-10-20 Tue 9:00>");
    assert_eq!(ts.hour_start().unwrap(), "9");

    let ts = to_timestamp("<2026-10-20 Tue 9:00-17:00>");
    assert!(ts.is_range());
    insta::assert_debug_snapshot!(
        ts.syntax,
        @r#"
    TIMESTAMP_ACTIVE@0..27
      L_ANGLE@0..1 "<"
      TIMESTAMP_YEAR@1..5 "2026"
      MINUS@5..6 "-"
      TIMESTAMP_MONTH@6..8 "10"
      MINUS@8..9 "-"
      TIMESTAMP_DAY@9..11 "20"
      WHITESPACE@11..12 " "
      TIMESTAMP_DAYNAME@12..15 "Tue"
      WHITESPACE@15..16 " "
      TIMESTAMP_HOUR@16..17 "9"
      COLON@17..18 ":"
      TIMESTAMP_MINUTE@18..20 "00"
      MINUS@20..21 "-"
      TIMESTAMP_HOUR@21..23 "17"
      COLON@23..24 ":"
      TIMESTAMP_MINUTE@24..26 "00"
      R_ANGLE@26..27 ">"
    "#
    );
}

#[test]
fn habit_repeater() {
    use crate::{ast::Timestamp, tests::to_ast};

    let to_timestamp = to_ast::<Timestamp>(timestamp_active_node);

    let ts = to_timestamp("<2026-10-20 Tue .+2d/4d>");
    assert_eq!(ts.repeater_value(), Some(2));
    assert_eq!(ts.repeater_deadline_value(), Some(4));
    insta::assert_debug_snapshot!(
        ts.syntax,
        @r#"
    TIMESTAMP_ACTIVE@0..24
      L_ANGLE@0..1 "<"
      TIMESTAMP_YEAR@1..5 "2026"
      MINUS@5..6 "-"
      TIMESTAMP_MONTH@6..8 "10"
      MINUS@8..9 "-"
      TIMESTAMP_DAY@9..11 "20"
      WHITESPACE@11..12 " "
      TIMESTAMP_DAYNAME@12..15 "Tue"
      WHITESPACE@15..16 " "
      TIMESTAMP_REPEATER_MARK@16..18 ".+"
      TIMESTAMP_VALUE@18..19 "2"
      TIMESTAMP_UNIT@19..20 "d"
      SLASH@20..21 "/"
      TIMESTAMP_VALUE@21..22 "4"
      TIMESTAMP_UNIT@22..23 "d"
      R_ANGLE@23..24 ">"
    "#
    );
}
