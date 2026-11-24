use nom::Parser;
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{alpha1, digit1, space0, space1},
    combinator::{opt, recognize},
    IResult, Input as _,
};

use super::{
    combinator::{
        blank_lines, eol_or_eof, line_starts_iter, node, token, trim_line_end, GreenElement,
        NodeBuilder,
    },
    element::element_nodes,
    input::Input,
    keyword::affiliated_keyword_nodes,
    SyntaxKind::*,
};

fn block_node_base(input: Input) -> IResult<Input, GreenElement, ()> {
    let (input, affiliated_keywords) = affiliated_keyword_nodes(input)?;
    let (input, (block_begin, name)) = block_begin_node(input)?;
    let (input, pre_blank) = blank_lines(input)?;

    let kind = match name {
        s if s.eq_ignore_ascii_case("COMMENT") => COMMENT_BLOCK,
        s if s.eq_ignore_ascii_case("EXAMPLE") => EXAMPLE_BLOCK,
        s if s.eq_ignore_ascii_case("EXPORT") => EXPORT_BLOCK,
        s if s.eq_ignore_ascii_case("SRC") => SOURCE_BLOCK,
        s if s.eq_ignore_ascii_case("CENTER") => CENTER_BLOCK,
        s if s.eq_ignore_ascii_case("QUOTE") => QUOTE_BLOCK,
        s if s.eq_ignore_ascii_case("VERSE") => VERSE_BLOCK,
        _ => SPECIAL_BLOCK,
    };

    for (input, contents) in line_starts_iter(&input).map(|i| input.take_split(i)) {
        if let Ok((input, block_end)) = block_end_node(input, name) {
            let (input, post_blank) = blank_lines(input)?;

            let mut children = vec![];
            children.extend(affiliated_keywords);
            children.push(block_begin);
            children.extend(pre_blank);
            if kind.is_greater_element() {
                let elements = if contents.is_empty() {
                    vec![]
                } else {
                    element_nodes(contents)?
                };
                children.push(node(BLOCK_CONTENT, elements));
            } else {
                children.push(node(BLOCK_CONTENT, comma_quoted_text_nodes(contents)));
            }
            children.push(block_end);
            children.extend(post_blank);
            return Ok((input, node(kind, children)));
        }
    }

    Err(nom::Err::Error(()))
}

fn block_begin_node(input: Input<'_>) -> IResult<Input<'_>, (GreenElement, &str), ()> {
    let (input, (ws1, begin, name)) = (space0, tag_no_case("#+BEGIN_"), alpha1).parse(input)?;

    let mut b = NodeBuilder::new();
    b.ws(ws1);
    b.text(begin);
    b.text(name);

    if name.eq_ignore_ascii_case("SRC") {
        let (input, language) = opt((
            space1,
            take_while1(|c: char| c != ' ' && c != '\t' && c != '\n' && c != '\r'),
        ))
        .parse(input)?;
        let (input, switches) = opt((space1, source_block_switches)).parse(input)?;
        let (input, ws1) = space0(input)?;
        let (input, (parameters, ws2, nl)) = trim_line_end(input)?;

        if let Some((ws, language)) = language {
            b.ws(ws);
            b.token(SRC_BLOCK_LANGUAGE, language);
        }
        if let Some((ws, switches)) = switches {
            b.ws(ws);
            b.token(SRC_BLOCK_SWITCHES, switches);
        }
        b.ws(ws1);
        if !parameters.is_empty() {
            b.token(SRC_BLOCK_PARAMETERS, parameters);
        }
        b.ws(ws2);
        b.nl(nl);
        Ok((input, (b.finish(BLOCK_BEGIN), name.as_str())))
    } else if name.eq_ignore_ascii_case("EXPORT") {
        let (input, ty) = opt((
            space1,
            take_while1(|c: char| c != ' ' && c != '\t' && c != '\n' && c != '\r'),
        ))
        .parse(input)?;
        let (input, data) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
        let (input, nl) = eol_or_eof(input)?;

        if let Some((ws, ty)) = ty {
            b.ws(ws);
            b.token(EXPORT_BLOCK_TYPE, ty);
        }
        b.text(data);
        b.nl(nl);
        Ok((input, (b.finish(BLOCK_BEGIN), name.as_str())))
    } else {
        let (input, data) = take_while(|c: char| c != '\n' && c != '\r').parse(input)?;
        let (input, nl) = eol_or_eof(input)?;

        b.text(data);
        b.nl(nl);
        Ok((input, (b.finish(BLOCK_BEGIN), name.as_str())))
    }
}

/// Recognizes source block switches, as Org's `org-element-src-block-parser`
/// does: `-l "FORMAT"`, `-i`, `-k`, `-r`, and `-n` or `+n` with an optional
/// number, separated by spaces
fn source_block_switches(input: Input) -> IResult<Input, Input, ()> {
    fn switch(i: Input) -> IResult<Input, Input, ()> {
        alt((
            recognize((tag("-l"), take_while1(|c| c == ' '), quoted)),
            recognize(alt((tag("-i"), tag("-k"), tag("-r")))),
            recognize((
                alt((tag("-n"), tag("+n"))),
                opt((take_while(|c| c == ' '), digit1)),
            )),
        ))
        .parse(i)
    }

    // `".+"`: from a double quote to the last double quote on the line
    fn quoted(i: Input) -> IResult<Input, Input, ()> {
        let line = i.as_str().split(['\n', '\r']).next().unwrap_or_default();
        match line.rfind('"') {
            Some(end) if line.starts_with('"') && end >= 2 => Ok(i.take_split(end + 1)),
            _ => Err(nom::Err::Error(())),
        }
    }

    let (mut i, _) = switch(input)?;

    while let Ok((i_, _)) = (take_while1(|c| c == ' '), switch).parse(i) {
        i = i_;
    }

    Ok(input.take_split(input.len() - i.len()))
}

fn block_end_node<'a>(input: Input<'a>, name: &str) -> IResult<Input<'a>, GreenElement, ()> {
    let (input, (ws, end, name, ws_, nl)) =
        (space0, tag_no_case("#+END_"), tag(name), space0, eol_or_eof).parse(input)?;

    let mut b = NodeBuilder::new();
    b.ws(ws);
    b.text(end);
    b.text(name);
    b.ws(ws_);
    b.nl(nl);

    Ok((input, b.finish(BLOCK_END)))
}

fn comma_quoted_text_nodes(input: Input) -> Vec<GreenElement> {
    let mut nodes = vec![];

    let s = input.as_str();

    let mut start = 0;
    for i in line_starts_iter(s) {
        // line must start with either ",*" or ",#+"
        if s.get(i..i + 2) != Some(",*") && s.get(i..i + 3) != Some(",#+") {
            continue;
        }

        let text = &s[start..i];
        if !text.is_empty() {
            nodes.push(token(TEXT, text));
        }

        nodes.push(token(COMMA, ","));
        start = i + 1;
    }

    if !s[start..].is_empty() {
        nodes.push(token(TEXT, &s[start..]));
    }

    nodes
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn block_node(input: Input) -> IResult<Input, GreenElement, ()> {
    crate::lossless_parser!(block_node_base, input)
}

#[test]
fn test_parse() {
    use crate::ast::{ExampleBlock, SourceBlock};
    use crate::tests::to_ast;

    let to_src_block = to_ast::<SourceBlock>(block_node);
    let to_example_block = to_ast::<ExampleBlock>(block_node);

    insta::assert_debug_snapshot!(
        to_example_block(
r#"#+BEGIN_EXAMPLE
,* headline
,#+block
text
    #+END_EXAMPLE"#
        ).syntax,
        @r###"
    EXAMPLE_BLOCK@0..59
      BLOCK_BEGIN@0..16
        TEXT@0..8 "#+BEGIN_"
        TEXT@8..15 "EXAMPLE"
        NEW_LINE@15..16 "\n"
      BLOCK_CONTENT@16..42
        COMMA@16..17 ","
        TEXT@17..28 "* headline\n"
        COMMA@28..29 ","
        TEXT@29..42 "#+block\ntext\n"
      BLOCK_END@42..59
        WHITESPACE@42..46 "    "
        TEXT@46..52 "#+END_"
        TEXT@52..59 "EXAMPLE"
    "###
    );

    insta::assert_debug_snapshot!(
        to_src_block(
r#"#+BEGIN_SRC


    #+END_SRC"#
        ).syntax,
        @r###"
    SOURCE_BLOCK@0..27
      BLOCK_BEGIN@0..12
        TEXT@0..8 "#+BEGIN_"
        TEXT@8..11 "SRC"
        NEW_LINE@11..12 "\n"
      BLANK_LINE@12..13 "\n"
      BLANK_LINE@13..14 "\n"
      BLOCK_CONTENT@14..14
      BLOCK_END@14..27
        WHITESPACE@14..18 "    "
        TEXT@18..24 "#+END_"
        TEXT@24..27 "SRC"
    "###
    );

    insta::assert_debug_snapshot!(
        to_src_block(
r#"#+begin_src
    #+end_src"#
        ).syntax,
        @r###"
    SOURCE_BLOCK@0..25
      BLOCK_BEGIN@0..12
        TEXT@0..8 "#+begin_"
        TEXT@8..11 "src"
        NEW_LINE@11..12 "\n"
      BLOCK_CONTENT@12..12
      BLOCK_END@12..25
        WHITESPACE@12..16 "    "
        TEXT@16..22 "#+end_"
        TEXT@22..25 "src"
    "###
    );

    insta::assert_debug_snapshot!(
        to_src_block(
r#"#+BEGIN_SRC javascript  -n 20 -r  :var n=0, l=2  :foo=bar
alert('Hello World!');
    #+END_SRC

    "#).syntax,
        @r###"
    SOURCE_BLOCK@0..100
      BLOCK_BEGIN@0..58
        TEXT@0..8 "#+BEGIN_"
        TEXT@8..11 "SRC"
        WHITESPACE@11..12 " "
        SRC_BLOCK_LANGUAGE@12..22 "javascript"
        WHITESPACE@22..24 "  "
        SRC_BLOCK_SWITCHES@24..32 "-n 20 -r"
        WHITESPACE@32..34 "  "
        SRC_BLOCK_PARAMETERS@34..57 ":var n=0, l=2  :foo=bar"
        NEW_LINE@57..58 "\n"
      BLOCK_CONTENT@58..81
        TEXT@58..81 "alert('Hello World!');\n"
      BLOCK_END@81..95
        WHITESPACE@81..85 "    "
        TEXT@85..91 "#+END_"
        TEXT@91..94 "SRC"
        NEW_LINE@94..95 "\n"
      BLANK_LINE@95..96 "\n"
      BLANK_LINE@96..100 "    "
    "###
    );

    // TODO: more testing
}

#[test]
fn empty_greater_block() {
    use crate::Org;

    for s in [
        "#+begin_quote\n#+end_quote\n",
        "#+begin_center\n#+end_center\n",
        "#+begin_note\n\n#+end_note\n",
        "* h\n#+begin_quote\n#+end_quote\n",
    ] {
        assert_eq!(Org::parse(s).to_org(), s);
    }
}

#[test]
fn source_block_switches_and_parameters() {
    use crate::ast::SourceBlock;
    use crate::tests::to_ast;

    let to_src_block = to_ast::<SourceBlock>(block_node);

    for (begin, switches, parameters) in [
        ("rust -n :tangle x", Some("-n"), Some(":tangle x")),
        (
            "rust -n 20 -r :tangle x",
            Some("-n 20 -r"),
            Some(":tangle x"),
        ),
        ("rust +n -i -k", Some("+n -i -k"), None),
        (
            r#"rust -l "(ref:%s)" -n :exports code"#,
            Some(r#"-l "(ref:%s)" -n"#),
            Some(":exports code"),
        ),
        ("rust :var x=-n", None, Some(":var x=-n")),
        ("rust -x :tangle x", None, Some("-x :tangle x")),
    ] {
        let block = to_src_block(&format!("#+begin_src {begin}\n#+end_src\n"));
        assert_eq!(block.switches().as_deref(), switches, "{begin}");
        assert_eq!(block.parameters().as_deref(), parameters, "{begin}");
    }

    insta::assert_debug_snapshot!(
        to_src_block("#+begin_src rust -n :tangle x\n#+end_src\n").syntax,
        @r##"
    SOURCE_BLOCK@0..40
      BLOCK_BEGIN@0..30
        TEXT@0..8 "#+begin_"
        TEXT@8..11 "src"
        WHITESPACE@11..12 " "
        SRC_BLOCK_LANGUAGE@12..16 "rust"
        WHITESPACE@16..17 " "
        SRC_BLOCK_SWITCHES@17..19 "-n"
        WHITESPACE@19..20 " "
        SRC_BLOCK_PARAMETERS@20..29 ":tangle x"
        NEW_LINE@29..30 "\n"
      BLOCK_CONTENT@30..30
      BLOCK_END@30..40
        TEXT@30..36 "#+end_"
        TEXT@36..39 "src"
        NEW_LINE@39..40 "\n"
    "##
    );
}
