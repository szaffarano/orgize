use std::iter::once;

use memchr::memchr2_iter;
use nom::{IResult, Input as _};
use rowan::GreenNode;

use super::{
    block::block_node,
    clock::clock_node,
    combinator::GreenElement,
    comment::comment_node,
    drawer::drawer_node,
    dyn_block::dyn_block_node,
    fixed_width::fixed_width_node,
    fn_def::fn_def_node,
    input::Input,
    keyword::{affiliated_keyword_nodes, keyword_node},
    latex_environment::latex_environment_node,
    list::list_node,
    paragraph::{paragraph_node, paragraph_nodes},
    rule::rule_node,
    table::{org_table_node, table_el_node},
};

/// Recognizes multiple org-mode elements
///
/// input must not contains blank line in the beginning
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn element_nodes(input: Input) -> Result<Vec<GreenElement>, nom::Err<()>> {
    debug_assert!(!input.is_empty());
    // TODO:
    // debug_assert!(
    //     blank_lines(input).unwrap().1.is_empty(),
    //     "input must not starts with blank lines: {:?}",
    //     input.s
    // );

    let mut i = input;
    let mut nodes = vec![];

    'l: while !i.is_empty() {
        for (input, head) in ElementPositions::new(i) {
            if let Ok((input, element)) = element_node(input) {
                if !head.is_empty() {
                    nodes.extend(paragraph_nodes(head)?);
                }
                nodes.push(element);
                debug_assert!(input.len() < i.len(), "{} < {}", input.len(), i.len());
                i = input;
                continue 'l;
            }
        }
        nodes.extend(paragraph_nodes(i)?);
        break;
    }

    debug_assert_eq!(
        input.as_str(),
        nodes.iter().fold(String::new(), |s, n| s + &n.to_string()),
        "parser must be lossless"
    );

    Ok(nodes)
}

/// Recognizes an org-mode element expect paragraph
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(input), fields(input = input.s))
)]
pub fn element_node<'a>(input: Input<'a>) -> IResult<Input<'a>, GreenElement, ()> {
    // skip affiliated keyword first
    let (i, nodes) = affiliated_keyword_nodes(input)?;

    let has_affiliated_keyword = !nodes.is_empty();

    // find first non-whitespace character
    let byte = i.bytes().find(|&b| b != b' ' && b != b'\t');

    debug_assert!(
        !(has_affiliated_keyword && matches!(byte, None | Some(b'\n') | Some(b'\r'))),
        "affiliated_keyword must not followed by blank lines: {:?}",
        input.s
    );

    // parsers below don't skip affiliated keywords themselves: run them on the
    // input after the keywords, then put the keywords in front of the element
    let with_keywords = |(input, element): (Input<'a>, GreenElement)| {
        let GreenElement::Node(node) = element else {
            return (input, element);
        };
        if nodes.is_empty() {
            return (input, GreenElement::Node(node));
        }
        let children = nodes
            .iter()
            .cloned()
            .chain(node.children().map(|c| c.to_owned()))
            .collect::<Vec<_>>();
        (
            input,
            GreenElement::Node(GreenNode::new(node.kind(), children)),
        )
    };

    let result = match byte {
        Some(b'[') => fn_def_node(input),
        Some(b'0'..=b'9') | Some(b'*') => list_node(input),
        // clock doesn't have affiliated keywords
        Some(b'C') if !has_affiliated_keyword => clock_node(input),
        Some(b'-') => rule_node(i)
            .map(with_keywords)
            .or_else(|_| list_node(input)),
        Some(b':') => drawer_node(i)
            .map(with_keywords)
            .or_else(|_| fixed_width_node(input)),
        Some(b'|') => org_table_node(i).map(with_keywords),
        Some(b'+') => table_el_node(i)
            .map(with_keywords)
            .or_else(|_| list_node(input)),
        Some(b'#') => block_node(input)
            .or_else(|_| dyn_block_node(i).map(with_keywords))
            .or_else(|_| keyword_node(input))
            .or_else(|_| comment_node(input)),
        Some(b'\\') => latex_environment_node(i).map(with_keywords),
        _ => Err(nom::Err::Error(())),
    };

    if has_affiliated_keyword {
        result.or_else(|_| paragraph_node(input))
    } else {
        result
    }
}

struct ElementPositions<'a> {
    input: Input<'a>,
    pos: usize,
}

impl<'a> ElementPositions<'a> {
    fn new(input: Input<'a>) -> Self {
        ElementPositions { input, pos: 0 }
    }
}

impl<'a> Iterator for ElementPositions<'a> {
    type Item = (Input<'a>, Input<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.input.s.len() {
            return None;
        }

        let bytes = &self.input.as_bytes()[self.pos..];

        let mut iter = once(0).chain(memchr2_iter(b'\r', b'\n', bytes).map(|i| i + 1));

        while let Some(i) = iter.next() {
            let b = *bytes[i..].iter().find(|&&b| b != b' ' && b != b'\t')?;

            if matches!(
                b,
                b'[' | b'0'..=b'9' | b'*' | b'C' | b'-' | b':' | b'|' | b'+' | b'#' | b'\\'
            ) {
                let previous = self.pos;
                self.pos = iter
                    .next()
                    .map_or_else(|| self.input.s.len(), |i| i + self.pos);

                debug_assert!(
                    previous < self.pos && self.pos <= self.input.s.len(),
                    "{} < {} < {}",
                    previous,
                    self.pos,
                    self.input.s.len()
                );

                let (input, head) = self.input.take_split(i + previous);

                return Some((input, head));
            }
        }

        None
    }
}

#[test]
fn positions() {
    let config = crate::ParseConfig::default();
    let s = "+\n\n    C\n    \r\n-\n\t\t[\n:  \r\n";
    let vec = ElementPositions::new((s, &config).into()).collect::<Vec<_>>();
    assert_eq!(vec.len(), 5);
    assert_eq!(vec[0].0.s, "+\n\n    C\n    \r\n-\n\t\t[\n:  \r\n");
    assert_eq!(vec[1].0.s, "    C\n    \r\n-\n\t\t[\n:  \r\n");
    assert_eq!(vec[2].0.s, "-\n\t\t[\n:  \r\n");
    assert_eq!(vec[3].0.s, "\t\t[\n:  \r\n");
    assert_eq!(vec[4].0.s, ":  \r\n");
}

#[test]
fn parse() {
    use crate::syntax::{SyntaxKind, SyntaxNode};
    use crate::{syntax::combinator::node, ParseConfig};

    let t = |input: &str| {
        let config = &ParseConfig::default();
        let children = element_nodes((input, config).into()).unwrap();
        SyntaxNode::new_root(node(SyntaxKind::SECTION, children).into_node().unwrap())
    };

    // paragraph stops at blank lines
    insta::assert_debug_snapshot!(
        t(r#"a

b"#),
        @r###"
    SECTION@0..4
      PARAGRAPH@0..3
        TEXT@0..2 "a\n"
        BLANK_LINE@2..3 "\n"
      PARAGRAPH@3..4
        TEXT@3..4 "b"
    "###
    );

    // paragraph followed by special element
    insta::assert_debug_snapshot!(
        t("Table:\n|cell"),
        @r###"
    SECTION@0..12
      PARAGRAPH@0..7
        TEXT@0..7 "Table:\n"
      ORG_TABLE@7..12
        ORG_TABLE_STANDARD_ROW@7..12
          PIPE@7..8 "|"
          ORG_TABLE_CELL@8..12
            TEXT@8..12 "cell"
    "###
    );
}

#[test]
fn affiliated_keywords() {
    use crate::syntax::{SyntaxKind, SyntaxNode};
    use crate::{syntax::combinator::node, ParseConfig};

    let t = |input: &str| {
        let config = &ParseConfig::default();
        let children = element_nodes((input, config).into()).unwrap();
        SyntaxNode::new_root(node(SyntaxKind::SECTION, children).into_node().unwrap())
    };

    // affiliated keywords + paragraph
    insta::assert_debug_snapshot!(
        t("#+ATTR_HTML: :width 300px\n[[./img/a.jpg]]"),
        @r###"
    SECTION@0..41
      PARAGRAPH@0..41
        AFFILIATED_KEYWORD@0..26
          HASH_PLUS@0..2 "#+"
          TEXT@2..11 "ATTR_HTML"
          COLON@11..12 ":"
          TEXT@12..25 " :width 300px"
          NEW_LINE@25..26 "\n"
        LINK@26..41
          L_BRACKET2@26..28 "[["
          LINK_PATH@28..39 "./img/a.jpg"
          R_BRACKET2@39..41 "]]"
    "###
    );

    // affiliated keywords + blank lines, fallback to normal keyword
    insta::assert_debug_snapshot!(
        t("#+ATTR_HTML: :width 300px\n#+CAPTION: abc\n\n[[./img/a.jpg]]"),
        @r###"
    SECTION@0..57
      KEYWORD@0..26
        HASH_PLUS@0..2 "#+"
        TEXT@2..11 "ATTR_HTML"
        COLON@11..12 ":"
        TEXT@12..25 " :width 300px"
        NEW_LINE@25..26 "\n"
      KEYWORD@26..42
        HASH_PLUS@26..28 "#+"
        TEXT@28..35 "CAPTION"
        COLON@35..36 ":"
        TEXT@36..40 " abc"
        NEW_LINE@40..41 "\n"
        BLANK_LINE@41..42 "\n"
      PARAGRAPH@42..57
        LINK@42..57
          L_BRACKET2@42..44 "[["
          LINK_PATH@44..55 "./img/a.jpg"
          R_BRACKET2@55..57 "]]"
    "###
    );

    // affiliated keywords + special element
    insta::assert_debug_snapshot!(
        t("#+CAPTION: a footnote def\n[fn:WORD] https://orgmode.org"),
        @r###"
    SECTION@0..55
      FN_DEF@0..55
        AFFILIATED_KEYWORD@0..26
          HASH_PLUS@0..2 "#+"
          TEXT@2..9 "CAPTION"
          COLON@9..10 ":"
          TEXT@10..25 " a footnote def"
          NEW_LINE@25..26 "\n"
        L_BRACKET@26..27 "["
        TEXT@27..29 "fn"
        COLON@29..30 ":"
        TEXT@30..34 "WORD"
        R_BRACKET@34..35 "]"
        TEXT@35..55 " https://orgmode.org"
    "###
    );

    // affiliated keywords + clock
    insta::assert_debug_snapshot!(
        t("#+CAPTION: a footnote def\nCLOCK: [2003-09-16 Tue 09:39]"),
        @r###"
    SECTION@0..55
      PARAGRAPH@0..55
        AFFILIATED_KEYWORD@0..26
          HASH_PLUS@0..2 "#+"
          TEXT@2..9 "CAPTION"
          COLON@9..10 ":"
          TEXT@10..25 " a footnote def"
          NEW_LINE@25..26 "\n"
        TEXT@26..33 "CLOCK: "
        TIMESTAMP_INACTIVE@33..55
          L_BRACKET@33..34 "["
          TIMESTAMP_YEAR@34..38 "2003"
          MINUS@38..39 "-"
          TIMESTAMP_MONTH@39..41 "09"
          MINUS@41..42 "-"
          TIMESTAMP_DAY@42..44 "16"
          WHITESPACE@44..45 " "
          TIMESTAMP_DAYNAME@45..48 "Tue"
          WHITESPACE@48..49 " "
          TIMESTAMP_HOUR@49..51 "09"
          COLON@51..52 ":"
          TIMESTAMP_MINUTE@52..54 "39"
          R_BRACKET@54..55 "]"
    "###
    );

    // affiliated keywords + eof
    insta::assert_debug_snapshot!(
        t("#+CAPTION: Longer caption."),
        @r###"
    SECTION@0..26
      KEYWORD@0..26
        HASH_PLUS@0..2 "#+"
        TEXT@2..9 "CAPTION"
        COLON@9..10 ":"
        TEXT@10..26 " Longer caption."
    "###
    );
}

#[test]
fn affiliated_keywords_before_any_element() {
    use crate::syntax::{SyntaxKind, SyntaxNode};
    use crate::{syntax::combinator::node, ParseConfig};

    let t = |input: &str| {
        let config = &ParseConfig::default();
        let children = element_nodes((input, config).into()).unwrap();
        SyntaxNode::new_root(node(SyntaxKind::SECTION, children).into_node().unwrap())
    };

    insta::assert_debug_snapshot!(
        t("#+NAME: t\n| a |\n\n#+caption: c\n:NOTES:\nx\n:END:\n\n#+NAME: r\n-----\n"),
        @r##"
    SECTION@0..63
      ORG_TABLE@0..17
        AFFILIATED_KEYWORD@0..10
          HASH_PLUS@0..2 "#+"
          TEXT@2..6 "NAME"
          COLON@6..7 ":"
          TEXT@7..9 " t"
          NEW_LINE@9..10 "\n"
        ORG_TABLE_STANDARD_ROW@10..16
          PIPE@10..11 "|"
          WHITESPACE@11..12 " "
          ORG_TABLE_CELL@12..13
            TEXT@12..13 "a"
          WHITESPACE@13..14 " "
          PIPE@14..15 "|"
          WHITESPACE@15..16 "\n"
        BLANK_LINE@16..17 "\n"
      DRAWER@17..47
        AFFILIATED_KEYWORD@17..30
          HASH_PLUS@17..19 "#+"
          TEXT@19..26 "caption"
          COLON@26..27 ":"
          TEXT@27..29 " c"
          NEW_LINE@29..30 "\n"
        DRAWER_BEGIN@30..38
          COLON@30..31 ":"
          TEXT@31..36 "NOTES"
          COLON@36..37 ":"
          NEW_LINE@37..38 "\n"
        DRAWER_CONTENT@38..40
          PARAGRAPH@38..40
            TEXT@38..40 "x\n"
        DRAWER_END@40..46
          COLON@40..41 ":"
          TEXT@41..44 "END"
          COLON@44..45 ":"
          NEW_LINE@45..46 "\n"
        BLANK_LINE@46..47 "\n"
      RULE@47..63
        AFFILIATED_KEYWORD@47..57
          HASH_PLUS@47..49 "#+"
          TEXT@49..53 "NAME"
          COLON@53..54 ":"
          TEXT@54..56 " r"
          NEW_LINE@56..57 "\n"
        TEXT@57..62 "-----"
        NEW_LINE@62..63 "\n"
    "##
    );

    insta::assert_debug_snapshot!(
        t("#+NAME: l\n\\begin{x}\n\\end{x}\n#+NAME: t\n+--+\n|  |\n+--+\n"),
        @r##"
    SECTION@0..53
      LATEX_ENVIRONMENT@0..28
        AFFILIATED_KEYWORD@0..10
          HASH_PLUS@0..2 "#+"
          TEXT@2..6 "NAME"
          COLON@6..7 ":"
          TEXT@7..9 " l"
          NEW_LINE@9..10 "\n"
        WHITESPACE@10..10 ""
        TEXT@10..16 "\\begin"
        L_CURLY@16..17 "{"
        TEXT@17..18 "x"
        R_CURLY@18..19 "}"
        TEXT@19..20 "\n"
        WHITESPACE@20..20 ""
        TEXT@20..24 "\\end"
        L_CURLY@24..25 "{"
        TEXT@25..26 "x"
        R_CURLY@26..27 "}"
        WHITESPACE@27..27 ""
        NEW_LINE@27..28 "\n"
      TABLE_EL@28..53
        AFFILIATED_KEYWORD@28..38
          HASH_PLUS@28..30 "#+"
          TEXT@30..34 "NAME"
          COLON@34..35 ":"
          TEXT@35..37 " t"
          NEW_LINE@37..38 "\n"
        TEXT@38..53 "+--+\n|  |\n+--+\n"
    "##
    );
}
