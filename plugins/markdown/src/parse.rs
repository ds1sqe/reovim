//! The constructs a Markdown line holds, and the looks they are.
//!
//! One pass over the lines, from the first, so a fence opened above
//! the range is still open inside it; a line the range does not meet
//! is counted and nothing else. A fence opens at a line beginning
//! with three backticks or tildes and closes at the next such line,
//! and inside one nothing is parsed. Outside, a line is a rule, a
//! heading, a quote, or text with inline constructs found left to
//! right, no nesting beyond a code span.

use alloc::vec::Vec;

use reovim_client_api::looks::{Look, Range};
use reovim_sdk::sgr;

/// What a construct's own look is painted at, under its markers.
const CONSTRUCT: u64 = 10;
/// What a marker's is, over the construct it conceals.
const CONCEALED: u64 = 20;

/// The most hashes a heading has.
const LEVELS: usize = 6;
/// What a fence line begins with, either of them.
const FENCES: [&str; 2] = ["```", "~~~"];

/// Every look the lines of `text` that meet `range` hold.
pub fn looks(text: &str, range: &Range) -> Vec<Look> {
    let mut found = Vec::new();
    let mut at = 0;
    let mut fenced = false;
    // The newline a file ends with closes its last line and opens no
    // other, as the drawer counts rows.
    for line in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        let end = at + line.len();
        let fence = FENCES.iter().any(|open| line.starts_with(open));
        if meets(range, at, end) {
            shown(&mut found, line, at, (fenced || fence, fence));
        }
        fenced ^= fence;
        at = end + 1;
    }
    found
}

/// One line the range meets: `held` says whether a fence holds the
/// line and whether the line is a fence's own.
fn shown(found: &mut Vec<Look>, line: &str, at: usize, held: (bool, bool)) {
    let (inside, fence) = held;
    if inside {
        tinted(found, line, at, fence);
    } else {
        lined(found, line, at);
    }
}

/// Whether the bytes `at..end` are any of `range`'s, a line of no
/// bytes at its edge being one of them.
fn meets(range: &Range, at: usize, end: usize) -> bool {
    as64(at) <= range.to && as64(end) >= range.from
}

/// A line of a fence: tinted whole, its marker concealed where it has
/// one, and nothing of it parsed.
fn tinted(found: &mut Vec<Look>, line: &str, at: usize, fence: bool) {
    found.push(look(at, 0, line.len(), sgr::FENCE));
    if fence {
        found.push(look(at, 0, marker(line), sgr::MARKER));
    }
}

/// How many bytes of the fence line `line` are its marker: the run of
/// whatever it begins with, three or more.
fn marker(line: &str) -> usize {
    let first = line.as_bytes().first().copied().unwrap_or(b'`');
    line.bytes().take_while(|&b| b == first).count()
}

/// One line outside a fence: a rule, a heading, a quote, or text.
fn lined(found: &mut Vec<Look>, line: &str, at: usize) {
    if ruled(line) {
        found.push(look(at, 0, line.len(), sgr::RULE));
    } else if let Some(hashes) = headed(line) {
        found.push(look(at, 0, hashes, sgr::MARKER));
        found.push(look(at, hashes, line.len(), sgr::HEADING));
    } else if line.starts_with("> ") {
        found.push(look(at, 0, 2, sgr::MARKER));
        found.push(look(at, 2, line.len(), sgr::QUOTE));
    } else {
        let head = listed(line);
        inline(found, &line[head..], at + head);
    }
}

/// Whether `line` is a rule: three or more of `-`, `*` or `_` and
/// nothing else.
fn ruled(line: &str) -> bool {
    let bare = line.trim();
    let mut rest = bare.chars();
    let Some(first) = rest.next() else { return false };
    matches!(first, '-' | '*' | '_')
        && bare.len() >= 3
        && rest.all(|c| c == first)
}

/// How many bytes of `line` are a heading's marker, its one space
/// included; `None` for a line that heads nothing.
fn headed(line: &str) -> Option<usize> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > LEVELS {
        return None;
    }
    match line.as_bytes().get(hashes) {
        None => Some(hashes),
        Some(b' ') => Some(hashes + 1),
        Some(_) => None,
    }
}

/// How many bytes of `line` a list's own marker takes, which stays as
/// it is and is no look.
///
/// A star's is the only one this reads: `- ` and `1. ` begin no
/// construct, so a line of either is text with or without the skip,
/// and a rule of stars is already answered above.
fn listed(line: &str) -> usize {
    if line.starts_with("* ") { 2 } else { 0 }
}

/// The inline constructs of `body`, whose first byte is byte `at` of
/// the draft: a code span, a link, strong or emphasis, found left to
/// right and none inside another.
fn inline(found: &mut Vec<Look>, body: &str, at: usize) {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let took = match bytes[i] {
            b'`' => spanned(found, bytes, at, i),
            b'[' => linked(found, bytes, at, i),
            b'*' | b'_' => stressed(found, bytes, at, i),
            _ => 0,
        };
        // A byte that begins no construct is one byte of text, and a
        // marker is always ASCII, so no boundary lands inside a char.
        i += took.max(1);
    }
}

/// A code span at `i`, and how many bytes it took; 0 for a backtick
/// that closes nowhere.
fn spanned(found: &mut Vec<Look>, bytes: &[u8], at: usize, i: usize) -> usize {
    let Some(end) = next(bytes, b'`', i + 1) else { return 0 };
    found.push(look(at, i, i + 1, sgr::MARKER));
    found.push(look(at, i + 1, end, sgr::CODE));
    found.push(look(at, end, end + 1, sgr::MARKER));
    end + 1 - i
}

/// A link at `i`, and how many bytes it took; 0 for a `[` that is no
/// link's.
fn linked(found: &mut Vec<Look>, bytes: &[u8], at: usize, i: usize) -> usize {
    let Some(shut) = next(bytes, b']', i + 1) else { return 0 };
    if bytes.get(shut + 1) != Some(&b'(') {
        return 0;
    }
    let Some(url) = next(bytes, b')', shut + 2) else { return 0 };
    found.push(look(at, i, i + 1, sgr::MARKER));
    found.push(look(at, i + 1, shut, sgr::LINK));
    found.push(look(at, shut, url + 1, sgr::MARKER));
    url + 1 - i
}

/// Strong or emphasis at `i`, and how many bytes it took; 0 for a
/// star or an underscore that closes nowhere.
fn stressed(found: &mut Vec<Look>, bytes: &[u8], at: usize, i: usize) -> usize {
    let mark = bytes[i];
    let wide = bytes.get(i + 1) == Some(&mark);
    let width = if wide { 2 } else { 1 };
    let Some(end) = closes(bytes, mark, i + width, width) else { return 0 };
    let style = if wide { sgr::STRONG } else { sgr::EMPHASIS };
    found.push(look(at, i, i + width, sgr::MARKER));
    found.push(look(at, i + width, end, style));
    found.push(look(at, end, end + width, sgr::MARKER));
    end + width - i
}

/// Where a run of `width` copies of `mark` closes what opened before
/// `from`, the text between being no fewer than one byte.
fn closes(bytes: &[u8], mark: u8, from: usize, width: usize) -> Option<usize> {
    let last = bytes.len().checked_sub(width)?;
    (from + 1..=last).find(|&j| bytes[j..j + width].iter().all(|&b| b == mark))
}

/// The next `mark` in `bytes` from `from`.
fn next(bytes: &[u8], mark: u8, from: usize) -> Option<usize> {
    bytes.get(from..)?.iter().position(|&b| b == mark).map(|at| at + from)
}

/// One look over the bytes `from..to` of the line beginning at `at`:
/// a marker's stands over the construct it conceals, and every other
/// under it.
fn look(at: usize, from: usize, to: usize, style: u64) -> Look {
    let priority = if style == sgr::MARKER { CONCEALED } else { CONSTRUCT };
    Look { from: as64(at + from), to: as64(at + to), style, priority }
}

/// A count as the bytes of a draft are numbered.
fn as64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use reovim_client_api::looks::{Look, Range};
    use reovim_sdk::sgr;

    use super::looks;

    /// A construct's own look, over the text it is the meaning of.
    fn over(from: u64, to: u64, style: u64) -> Look {
        Look { from, to, style, priority: 10 }
    }

    /// A marker's, which the drawer conceals over the construct.
    fn hidden(from: u64, to: u64) -> Look {
        Look { from, to, style: sgr::MARKER, priority: 20 }
    }

    /// Every look of `text`, the range being the whole of it.
    fn whole(text: &str) -> Vec<Look> {
        looks(text, &Range { from: 0, to: text.len() as u64 })
    }

    #[test]
    fn a_heading_is_its_text_and_its_hashes_are_markers() {
        let found = whole("# Heading\n");
        assert_eq!(found, [hidden(0, 2), over(2, 9, sgr::HEADING)]);
    }

    #[test]
    fn six_hashes_head_and_seven_do_not() {
        assert_eq!(whole("###### h\n").len(), 2);
        assert_eq!(whole("####### h\n"), Vec::new(), "no heading of seven");
        assert_eq!(whole("#nospace\n"), Vec::new(), "a space or nothing");
    }

    #[test]
    fn stars_are_strong_and_emphasis_with_their_markers() {
        let found = whole("**bold** and *soft*\n");
        let want = [
            hidden(0, 2),
            over(2, 6, sgr::STRONG),
            hidden(6, 8),
            hidden(13, 14),
            over(14, 18, sgr::EMPHASIS),
            hidden(18, 19),
        ];
        assert_eq!(found, want);
    }

    #[test]
    fn underscores_say_the_same_as_stars() {
        let found = whole("__b__ and _s_\n");
        let want = [
            hidden(0, 2),
            over(2, 3, sgr::STRONG),
            hidden(3, 5),
            hidden(10, 11),
            over(11, 12, sgr::EMPHASIS),
            hidden(12, 13),
        ];
        assert_eq!(found, want);
    }

    #[test]
    fn a_code_span_is_its_text_and_its_backticks_are_markers() {
        let found = whole("use `cargo test` now\n");
        let want = [hidden(4, 5), over(5, 15, sgr::CODE), hidden(15, 16)];
        assert_eq!(found, want);
    }

    #[test]
    fn a_star_inside_a_code_span_is_text() {
        let found = whole("`*x*` here\n");
        let want = [hidden(0, 1), over(1, 4, sgr::CODE), hidden(4, 5)];
        assert_eq!(found, want, "no nesting beyond a code span");
    }

    #[test]
    fn a_link_is_its_text_and_its_url_is_a_marker() {
        let found = whole("see [the docs](http://x) now\n");
        let want = [hidden(4, 5), over(5, 13, sgr::LINK), hidden(13, 24)];
        assert_eq!(found, want);
    }

    #[test]
    fn a_quote_is_the_text_after_its_marker() {
        let found = whole("> quoted\n");
        assert_eq!(found, [hidden(0, 2), over(2, 8, sgr::QUOTE)]);
    }

    #[test]
    fn a_rule_line_is_all_rule_and_no_marker() {
        assert_eq!(whole("---\n"), [over(0, 3, sgr::RULE)]);
        assert_eq!(whole("***\n"), [over(0, 3, sgr::RULE)]);
        assert_eq!(whole("___\n"), [over(0, 3, sgr::RULE)]);
    }

    #[test]
    fn a_fence_covers_its_lines_and_parses_nothing_inside() {
        let found = whole("```rust\nlet *x* = 1;\n```\n");
        let want = [
            over(0, 7, sgr::FENCE),
            hidden(0, 3),
            over(8, 20, sgr::FENCE),
            over(21, 24, sgr::FENCE),
            hidden(21, 24),
        ];
        assert_eq!(found, want);
    }

    #[test]
    fn a_fence_nobody_closed_runs_to_the_end() {
        let found = whole("```\nstill\n");
        let want =
            [over(0, 3, sgr::FENCE), hidden(0, 3), over(4, 9, sgr::FENCE)];
        assert_eq!(found, want);
    }

    #[test]
    fn a_fence_of_more_than_three_hides_all_of_them() {
        let found = whole("````rust\nin\n````\n");
        let want = [
            over(0, 8, sgr::FENCE),
            hidden(0, 4),
            over(9, 11, sgr::FENCE),
            over(12, 16, sgr::FENCE),
            hidden(12, 16),
        ];
        assert_eq!(found, want);
    }

    #[test]
    fn a_tilde_fence_is_a_fence() {
        let found = whole("~~~\nin\n~~~\n");
        let want = [
            over(0, 3, sgr::FENCE),
            hidden(0, 3),
            over(4, 6, sgr::FENCE),
            over(7, 10, sgr::FENCE),
            hidden(7, 10),
        ];
        assert_eq!(found, want);
    }

    #[test]
    fn a_list_marker_stays_as_it_is() {
        let found = whole("- *item*\n");
        let want = [hidden(2, 3), over(3, 7, sgr::EMPHASIS), hidden(7, 8)];
        assert_eq!(found, want, "the dash is no look of its own");
        assert_eq!(whole("* plain star\n"), Vec::new());
        assert_eq!(whole("1. a\n"), Vec::new());
        let starred = whole("* a * b\n");
        assert_eq!(starred, Vec::new(), "a list's own star opens nothing");
    }

    #[test]
    fn a_marker_that_closes_nowhere_is_text() {
        assert_eq!(whole("a ` b\n"), Vec::new(), "one backtick");
        assert_eq!(whole("a [b c\n"), Vec::new(), "a bracket, no url");
        assert_eq!(whole("a [b](c\n"), Vec::new(), "a url, no close");
        assert_eq!(whole("a [b] (c)\n"), Vec::new(), "a url not its own");
        assert_eq!(whole("a * b\n"), Vec::new(), "one star");
        assert_eq!(whole("a **** b\n"), Vec::new(), "and no text between");
    }

    #[test]
    fn only_the_lines_the_range_meets_are_answered() {
        let text = "# A\n# B\n# C\n";
        let found = looks(text, &Range { from: 4, to: 7 });
        assert_eq!(found, [hidden(4, 6), over(6, 7, sgr::HEADING)]);
    }
}
