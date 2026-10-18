//! Cached grapheme-safe word boundaries for read-only Vim navigation.
//!
//! A word groups Unicode letters/digits/underscore or contiguous punctuation.
//! A WORD groups non-whitespace graphemes. Unicode Segmentation owns cluster
//! boundaries; this is a fixed reader policy, not configurable Vim iskeyword.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// Direction and destination of a word movement.
#[derive(Clone, Copy)]
pub enum WordMotion {
    /// Start of the next word.
    Next,
    /// Start of the preceding word.
    Previous,
    /// End of the next/current word, advancing when already at its end.
    End,
    /// End of the preceding word.
    PreviousEnd,
}

#[derive(Clone, Copy)]
struct Word {
    start: usize,
    last: usize,
    end: usize,
}

#[derive(PartialEq, Eq)]
enum WordClass {
    Whitespace,
    Keyword,
    Punctuation,
}

/// Width-independent word indexes, rebuilt only when readable text changes.
#[derive(Default)]
pub struct WordIndex {
    words: Vec<Word>,
    wide: Vec<Word>,
}

impl WordIndex {
    /// Build word and whitespace-delimited WORD boundaries in one scan each.
    pub fn new(text: &str) -> Self {
        Self {
            words: words(text, false),
            wide: words(text, true),
        }
    }

    /// Resolve a count with binary searches rather than rescanning the document.
    pub fn movement(
        &self,
        offset: usize,
        motion: WordMotion,
        wide: bool,
        count: usize,
        last: usize,
    ) -> usize {
        let words = if wide { &self.wide } else { &self.words };
        let fallback = match motion {
            WordMotion::Previous | WordMotion::PreviousEnd => 0,
            WordMotion::Next | WordMotion::End => last,
        };
        let Some((index, end)) =
            destination(words, offset, motion, count.max(1))
        else {
            return fallback;
        };
        words
            .get(index)
            .map_or(fallback, |word| if end { word.last } else { word.start })
    }

    /// Select a word/WORD or intervening whitespace, optionally including space.
    pub fn object(
        &self,
        text: &str,
        offset: usize,
        wide: bool,
        around: bool,
    ) -> Option<Range<usize>> {
        let words = if wide { &self.wide } else { &self.words };
        let index = words.partition_point(|word| word.end <= offset);
        let next = words.get(index);
        let previous = index.checked_sub(1).and_then(|index| words.get(index));
        if around && next.is_none() {
            return previous.map(|word| {
                let end = text[word.end..]
                    .find('\n')
                    .map_or(text.len(), |end| word.end + end);
                word.start..end
            });
        }
        if around && let Some(word) = next.filter(|word| offset < word.start) {
            return Some(previous.map_or(0, |word| word.end)..word.end);
        }
        let mut range =
            if let Some(word) = next.filter(|word| word.start <= offset) {
                word.start..word.end
            } else {
                previous.map_or(0, |word| word.end)
                    ..next.map_or(text.len(), |word| word.start)
            };
        if around {
            let suffix = text
                .get(range.end..)?
                .chars()
                .take_while(|character| character.is_whitespace())
                .map(char::len_utf8)
                .sum::<usize>();
            if suffix > 0 {
                range.end += suffix;
            } else {
                range.start = previous.map_or(0, |word| word.end);
            }
        }
        (!range.is_empty()).then_some(range)
    }
}

fn destination(
    words: &[Word],
    offset: usize,
    motion: WordMotion,
    count: usize,
) -> Option<(usize, bool)> {
    let result = match motion {
        WordMotion::Next => (
            words
                .partition_point(|word| word.start <= offset)
                .saturating_add(count - 1),
            false,
        ),
        WordMotion::Previous => (
            words
                .partition_point(|word| word.start < offset)
                .checked_sub(count)?,
            false,
        ),
        WordMotion::End => (
            words
                .partition_point(|word| word.last <= offset)
                .saturating_add(count - 1),
            true,
        ),
        WordMotion::PreviousEnd => (
            words
                .partition_point(|word| word.last < offset)
                .checked_sub(count)?,
            true,
        ),
    };
    Some(result)
}

fn words(text: &str, wide: bool) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut previous = WordClass::Whitespace;
    for (start, grapheme) in text.grapheme_indices(true) {
        let class = class(grapheme, wide);
        if class == WordClass::Whitespace {
            previous = class;
            continue;
        }
        if class == previous
            && let Some(word) = words.last_mut()
        {
            word.last = start;
            word.end = start + grapheme.len();
        } else {
            words.push(Word {
                start,
                last: start,
                end: start + grapheme.len(),
            });
        }
        previous = class;
    }
    words
}

fn class(grapheme: &str, wide: bool) -> WordClass {
    let character = grapheme.chars().next().unwrap_or(' ');
    if character.is_whitespace() {
        return WordClass::Whitespace;
    }
    if wide || character.is_alphanumeric() || character == '_' {
        return WordClass::Keyword;
    }
    WordClass::Punctuation
}
