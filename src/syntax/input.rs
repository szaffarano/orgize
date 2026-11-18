use nom::{Compare, CompareResult, FindSubstring, Input as NomInput, Needed, Offset};
use std::{
    ops::Deref,
    slice::SliceIndex,
    str::{CharIndices, Chars},
};

use super::{
    combinator::{token, GreenElement},
    SyntaxKind,
};
use crate::config::ParseConfig;

/// A custom Input struct
///
/// It helps us to pass the `ParseConfig` all the way down to each parsers
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    pub(crate) s: &'a str,
    pub(crate) c: &'a ParseConfig,
}

impl<'a> Input<'a> {
    #[inline]
    pub(crate) fn of(&self, i: &'a str) -> Input<'a> {
        Input { s: i, c: self.c }
    }

    #[inline]
    pub(crate) fn slice<R: SliceIndex<str, Output = str>>(&self, range: R) -> Input<'a> {
        self.of(&self.s[range])
    }

    #[inline]
    pub fn as_str(&self) -> &'a str {
        self.s
    }

    #[inline]
    pub fn token(&self, kind: SyntaxKind) -> GreenElement {
        token(kind, self.s)
    }

    #[inline]
    pub fn text_token(&self) -> GreenElement {
        token(SyntaxKind::TEXT, self.s)
    }

    #[inline]
    pub fn ws_token(&self) -> GreenElement {
        token(SyntaxKind::WHITESPACE, self.s)
    }

    #[inline]
    pub fn nl_token(&self) -> GreenElement {
        token(SyntaxKind::NEW_LINE, self.s)
    }
}

impl<'a> Deref for Input<'a> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &'a str {
        self.s
    }
}

impl<'a> From<(&'a str, &'a ParseConfig)> for Input<'a> {
    fn from(value: (&'a str, &'a ParseConfig)) -> Self {
        Input {
            s: value.0,
            c: value.1,
        }
    }
}

impl<'a, 'b> FindSubstring<&'b str> for Input<'a> {
    fn find_substring(&self, substr: &str) -> Option<usize> {
        self.s.find(substr)
    }
}

impl<'a, 'b> Compare<&'b str> for Input<'a> {
    #[inline]
    fn compare(&self, t: &'b str) -> CompareResult {
        self.s.compare(t)
    }

    #[inline]
    fn compare_no_case(&self, t: &'b str) -> CompareResult {
        self.s.compare_no_case(t)
    }
}

impl<'a> NomInput for Input<'a> {
    type Item = char;
    type Iter = Chars<'a>;
    type IterIndices = CharIndices<'a>;

    #[inline]
    fn input_len(&self) -> usize {
        self.s.len()
    }

    #[inline]
    fn take(&self, index: usize) -> Self {
        self.of(self.s.take(index))
    }

    #[inline]
    fn take_from(&self, index: usize) -> Self {
        self.of(self.s.take_from(index))
    }

    #[inline]
    fn take_split(&self, index: usize) -> (Self, Self) {
        let (a, b) = self.s.take_split(index);
        (self.of(a), self.of(b))
    }

    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.s.position(predicate)
    }

    #[inline]
    fn iter_elements(&self) -> Self::Iter {
        self.s.chars()
    }

    #[inline]
    fn iter_indices(&self) -> Self::IterIndices {
        self.s.char_indices()
    }

    #[inline]
    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        self.s.slice_index(count)
    }
}

impl<'a> Offset for Input<'a> {
    fn offset(&self, second: &Self) -> usize {
        self.s.offset(second.s)
    }
}
