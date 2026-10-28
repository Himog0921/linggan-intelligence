//! Text helpers for the checks. Byte ranges stay internal; anything handed to the browser is
//! converted to UTF-16 code units, which is how JavaScript indexes strings.

/// Characters that count towards a title or cover length: everything except whitespace.
pub fn char_count(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

pub fn utf16_index(s: &str, byte: usize) -> usize {
    s[..byte].encode_utf16().count()
}

/// UTF-16 start and end of a byte range.
pub fn utf16_range(s: &str, range: (usize, usize)) -> (usize, usize) {
    (utf16_index(s, range.0), utf16_index(s, range.1))
}

/// Leftmost-longest, non-overlapping byte ranges of every needle in `hay` (exact match).
pub fn find_any(hay: &str, needles: &[&str]) -> Vec<(usize, usize)> {
    let mut found: Vec<(usize, usize)> = needles
        .iter()
        .filter(|needle| !needle.is_empty())
        .flat_map(|needle| {
            hay.match_indices(needle)
                .map(|(start, hit)| (start, start + hit.len()))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut kept: Vec<(usize, usize)> = Vec::new();
    for range in found {
        if kept.last().is_none_or(|last| range.0 >= last.1) {
            kept.push(range);
        }
    }
    kept
}

/// Same as [`find_any`] for owned needles.
pub fn find_any_owned(hay: &str, needles: &[String]) -> Vec<(usize, usize)> {
    let refs: Vec<&str> = needles.iter().map(String::as_str).collect();
    find_any(hay, &refs)
}

pub fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// One step of a loose pattern: a literal (ASCII letters compare without case), an optional
/// literal, or any number of spaces between two literals.
#[derive(Clone, Copy)]
pub enum Tok {
    Lit(&'static str),
    Opt(&'static str),
    Gap,
}

fn eat(hay: &str, pos: usize, lit: &str) -> Option<usize> {
    let mut at = pos;
    let mut rest = hay[pos..].chars();
    for expected in lit.chars() {
        let got = rest.next()?;
        if !got.eq_ignore_ascii_case(&expected) {
            return None;
        }
        at += got.len_utf8();
    }
    Some(at)
}

fn match_at(hay: &str, start: usize, pattern: &[Tok]) -> Option<usize> {
    let mut pos = start;
    for tok in pattern {
        match tok {
            Tok::Lit(lit) => pos = eat(hay, pos, lit)?,
            Tok::Opt(lit) => {
                if let Some(next) = eat(hay, pos, lit) {
                    pos = next;
                }
            }
            Tok::Gap => {
                while let Some(c) = hay[pos..].chars().next() {
                    if matches!(c, ' ' | '\t' | '\u{3000}') {
                        pos += c.len_utf8();
                    } else {
                        break;
                    }
                }
            }
        }
    }
    Some(pos)
}

/// Leftmost, non-overlapping byte ranges where any of the patterns matches. Every pattern must
/// start with a literal so that no match is empty.
pub fn find_patterns(hay: &str, patterns: &[&[Tok]]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < hay.len() {
        let best = patterns
            .iter()
            .filter_map(|pattern| match_at(hay, i, pattern))
            .max();
        match best {
            Some(end) if end > i => {
                out.push((i, end));
                i = end;
            }
            _ => {
                i += hay[i..].chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    out
}

/// True when the byte offset lies inside a hashtag token such as `#a娃成长`.
pub fn inside_hashtag(hay: &str, byte: usize) -> bool {
    hay[..byte]
        .chars()
        .rev()
        .take_while(|c| !c.is_whitespace())
        .any(|c| c == '#' || c == '＃')
}

pub fn is_ascii_word(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_characters_without_whitespace() {
        assert_eq!(char_count("A娃 的\n日记"), 5);
        assert_eq!(char_count(""), 0);
        assert_eq!(char_count("😀😀"), 2);
    }

    #[test]
    fn utf16_offsets_follow_javascript_indexing() {
        let s = "😀好a";
        assert_eq!(utf16_index(s, 0), 0);
        assert_eq!(utf16_index(s, "😀".len()), 2);
        assert_eq!(utf16_range(s, ("😀".len(), s.len())), (2, 4));
    }

    #[test]
    fn find_any_prefers_the_longest_and_never_overlaps() {
        let hay = "彻底改掉了坏习惯，改掉了";
        let hits = find_any(hay, &["改掉了", "彻底改掉"]);
        let texts: Vec<&str> = hits.iter().map(|&(a, b)| &hay[a..b]).collect();
        assert_eq!(texts, vec!["彻底改掉", "改掉了"]);
        assert!(find_any("没有", &["", "治好"]).is_empty());
    }

    #[test]
    fn loose_patterns_ignore_spaces_and_ascii_case() {
        let pats: &[&[Tok]] = &[&[Tok::Lit("a"), Tok::Gap, Tok::Lit("娃"), Tok::Lit("都")]];
        for hay in ["A娃都好", "a 娃都好", "A　娃都好", "xxA娃都"] {
            assert_eq!(find_patterns(hay, pats).len(), 1, "{hay}");
        }
        assert!(find_patterns("A娃不都", pats).is_empty());
        let hay = "前A  娃都后";
        let hit = find_patterns(hay, pats)[0];
        assert_eq!(&hay[hit.0..hit.1], "A  娃都");
    }

    #[test]
    fn optional_literals_do_not_create_empty_matches() {
        let pats: &[&[Tok]] = &[&[Tok::Lit("所有"), Tok::Opt("的"), Tok::Gap, Tok::Lit("A娃")]];
        assert_eq!(find_patterns("所有的A娃", pats).len(), 1);
        assert_eq!(find_patterns("所有A娃", pats).len(), 1);
        assert!(find_patterns("所有孩子", pats).is_empty());
        assert!(find_patterns("", pats).is_empty());
    }

    #[test]
    fn hashtag_tokens_are_recognised_by_their_leading_mark() {
        let s = "写 #a娃成长 然后 a娃";
        let inside = s.find("a娃成长").unwrap();
        let outside = s.rfind("a娃").unwrap();
        assert!(inside_hashtag(s, inside));
        assert!(!inside_hashtag(s, outside));
    }
}
