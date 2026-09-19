//! `reovim view [--socket PATH] [--color[=WHEN]] DISPLAY`: a reader
//! that knows it shows a display, not a printer of its bytes.
//!
//! `DISPLAY` is a display's own directory, `.../display/0` today; the
//! viewer reads `text` (one frame, which holds `rows`) and `info/cols`,
//! its own size being `term::size()`. It paints each line with
//! `look::painted`, as `tui::draw` does, and fills every cell of its
//! own screen right of the source's `cols` or below its `rows` with
//! `·`, tmux style. A source larger than the viewer: a row whose own
//! content did not fit whole ends in a dim `>`; a row that did is not
//! marked, even under a narrower viewer; rows that do not fit at all
//! collapse the last row into a dim `+N rows`. The cursor stands
//! where the frame's does, hidden past the viewer's own edge.
//!
//! It draws again on a `Modify` of `text` and on its own resize; `q`
//! and Ctrl-C end it. It opens nothing for writing: it types nothing
//! into the display and leaves the tree exactly as it found it.
//!
//! The frame is another Client's bytes, so nothing here trusts its
//! shape: a huge or a zero `rows`, a `row` or a `col` past what is
//! there, are read without panicking on them; a run gone wrong is
//! `look::painted`'s own guard, `capture` and `tui::draw` reading the
//! same runs and needing it just as much.

use std::fmt::Write as _;

use reovim_client::{Calls, Fault};
use reovim_client_api::{column, display, width};
use reovim_host::term;
use reovim_kernel_interface::{Error, Event, Kind, Rd};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, look, ops};

use crate::Fail;
use crate::tui::{out, tupled};

/// Between one poll of the terminal and the next check of what changed
/// since the last one: the tui's own redraw tick.
const TICK: u64 = 20;

/// `·`, a cell past the source's own screen, tmux style.
const DOT: char = '·';

/// `target`, read and drawn until `q` or Ctrl-C: a Client of its own on
/// `socket`, as `cli` connects.
///
/// # Errors
/// `Client` for a target `DISPLAY` is not, or holds no `text` or
/// `info/cols`; otherwise as [`crate::cli::linked`] and `term::enter`.
pub fn run(socket: &str, target: &str, color: bool) -> Result<(), Fail> {
    let host = crate::cli::linked(socket)?;
    let ctx = host.own();
    let shown = cell::resolve(&ctx, ROOT, target).map_err(Fault::from)?;
    let text = cell::resolve(&ctx, shown, "text").map_err(Fault::from)?;
    let cols = cell::resolve(&ctx, shown, "info/cols").map_err(Fault::from)?;
    term::enter()?;
    let result = drive(&ctx, text, cols, color);
    let _ = term::leave();
    result
}

/// Watched before the first draw, so nothing written between the two
/// is missed; closed again whatever ends the loop.
fn drive(ctx: &dyn Calls, text: Rd, cols: Rd, color: bool) -> Result<(), Fail> {
    let h = ops::watch(ctx, text).map_err(Fault::from)?;
    let cursor = ops::since(ctx, 0).map_err(Fault::from)?.1;
    let result = redraws(ctx, text, cols, color, cursor);
    ops::shut(ctx, &[h]);
    result
}

/// The first frame, then one drawn again on a resize of this terminal
/// or a `Modify` of `text`; `q` and Ctrl-C are the only keys read.
fn redraws(
    ctx: &dyn Calls,
    text: Rd,
    cols: Rd,
    color: bool,
    mut cursor: u64,
) -> Result<(), Fail> {
    draw(ctx, text, cols, color)?;
    loop {
        if term::stopped() {
            return Err(Fail::Signal);
        }
        match term::poll(TICK)? {
            Some(term::Event::Key(
                term::Key::Char('q') | term::Key::Ctrl('c'),
            )) => {
                return Ok(());
            }
            Some(term::Event::Resize) => draw(ctx, text, cols, color)?,
            _ => {}
        }
        let (events, next) = ops::since(ctx, cursor).map_err(Fault::from)?;
        cursor = next;
        let redrawn =
            |event: &Event| event.kind == Kind::Modify && event.rd == text;
        if events.iter().any(redrawn) {
            draw(ctx, text, cols, color)?;
        }
    }
}

/// One whole frame of `text`, painted at this terminal's own size, the
/// cursor placed or left hidden.
fn draw(ctx: &dyn Calls, text: Rd, cols: Rd, color: bool) -> Result<(), Fail> {
    let screen: display::Screen =
        decode(&ops::bytes(ctx, text).map_err(Fault::from)?)
            .map_err(Fault::from)?;
    let cols_bytes = ops::bytes(ctx, cols).map_err(Fault::from)?;
    let source_cols: u64 = String::from_utf8_lossy(&cols_bytes)
        .trim()
        .parse()
        .map_err(|_| Fault::from(Error::Invalid))?;
    let view = term::size()?;
    let (rows, cursor) = painted(&screen, source_cols, view, color);
    let mut shown = String::from("\x1b[?25l\x1b[H");
    shown.push_str(&rows.join("\r\n"));
    if let Some((row, col)) = cursor {
        let _ = write!(shown, "\x1b[{};{}H\x1b[?25h", row + 1, col + 1);
    }
    Ok(out(shown.as_bytes())?)
}

/// `screen` painted into a viewer of `view` cells, `source_cols` being
/// `info/cols` since a `Screen` no longer carries it: the pure part of
/// [`run`], no terminal needed to ask it anything, and safe over a
/// frame no Client here drew.
fn painted(
    screen: &display::Screen,
    source_cols: u64,
    view: (u16, u16),
    color: bool,
) -> (Vec<String>, Option<(u16, u16)>) {
    let source_cols = usize::try_from(source_cols).unwrap_or(usize::MAX);
    let source_rows = usize::try_from(screen.rows).unwrap_or(usize::MAX).max(1);
    let view_cols = usize::from(view.0);
    let view_rows = usize::from(view.1);
    // The one cell narrower reserves room for a `>` a cut row may
    // need; `fill` is the `·` past `source_cols` a wider viewer gets,
    // 0 either way once the viewer is the narrower of the two.
    let cols_cut = view_cols < source_cols;
    let usable = if cols_cut { view_cols - 1 } else { source_cols };
    let fill = view_cols.saturating_sub(source_cols);
    let rows_cut = view_rows < source_rows;
    let shown_rows =
        if rows_cut { view_rows.saturating_sub(1) } else { source_rows };
    // Rows before the status; a row at or past it is the status shown
    // inverse, as the app shows it, and cut the same way any row is.
    let text_rows = source_rows.saturating_sub(1);

    let mut rows = Vec::with_capacity(shown_rows);
    for at in 0..shown_rows {
        let (text, runs) = if at < text_rows {
            screen.lines.get(at).map_or_else(
                || (String::new(), Vec::new()),
                |line| (line.text.clone(), tupled(&line.runs)),
            )
        } else {
            let len = u64::try_from(screen.status.len()).unwrap_or(u64::MAX);
            (screen.status.clone(), vec![(0, len, look::INVERSE)])
        };
        rows.push(row(&text, &runs, usable, cols_cut, fill, color));
    }
    if rows_cut {
        rows.push(omitted(source_rows - shown_rows, view_cols, color));
    } else if view_rows > source_rows {
        let blank: String = std::iter::repeat_n(DOT, view_cols).collect();
        rows.extend(std::iter::repeat_n(blank, view_rows - source_rows));
    }
    (rows, cursor_at(screen, usable, shown_rows))
}

/// One source row painted for the viewer, in `usable` cells: `·` past
/// `source_cols` where the viewer has `fill` more of them (0 where it
/// does not), else a dim `>` where this row's own content did not fit
/// whole, a plain cell where it did — a narrower viewer alone marks
/// nothing; a row it did not have to cut is not a cut row.
fn row(
    text: &str,
    runs: &[(u64, u64, u64)],
    usable: usize,
    cols_cut: bool,
    fill: usize,
    color: bool,
) -> String {
    let (mut out, cut) = cells(text, runs, usable, color);
    if cols_cut {
        if cut {
            out.push_str(&dim(">", color));
        } else {
            out.push(' ');
        }
    } else {
        out.extend(std::iter::repeat_n(DOT, fill));
    }
    out
}

/// `text`'s own cells up to `cells`, cut at a char boundary — a wide
/// char that would cross it is left out whole — and the cells it
/// took: the same width `column` counts by.
fn clip(text: &str, cells: usize) -> (&str, usize) {
    let mut used = 0;
    let mut end = text.len();
    for (at, c) in text.char_indices() {
        let w = width(c);
        if used + w > cells {
            end = at;
            break;
        }
        used += w;
    }
    (&text[..end], used)
}

/// `text`'s own `runs` fit into `usable` cells and padded to it with
/// spaces, and whether they did not fit whole: past `usable` itself,
/// or a run's own pad past `text` (for a newline it takes) landing
/// where there was no more room for it. `look::painted` of an empty
/// run list is `text` alone, so `color` false asks for nothing more
/// than the plain slice.
fn cells(
    text: &str,
    runs: &[(u64, u64, u64)],
    usable: usize,
    color: bool,
) -> (String, bool) {
    let (slice, used) = clip(text, usable);
    let truncated = slice.len() < text.len();
    let end = u64::try_from(slice.len()).unwrap_or(u64::MAX);
    let kept: Vec<_> = if truncated {
        runs.iter()
            .filter(|&&(from, _, _)| from < end)
            .map(|&(from, to, style)| (from, to.min(end), style))
            .collect()
    } else {
        runs.to_vec()
    };
    let mut shown =
        if color { look::painted(slice, &kept) } else { slice.to_string() };
    let taken = u64::try_from(text.len()).unwrap_or(u64::MAX);
    let wants_pad = !truncated && runs.iter().any(|&(_, to, _)| to > taken);
    let has_room = used < usable;
    let pad = usable
        .saturating_sub(used + usize::from(color && wants_pad && has_room));
    shown.extend(std::iter::repeat_n(' ', pad));
    (shown, truncated || (color && wants_pad && !has_room))
}

/// `text` in the look a marker draws in: dim, and only when `color`
/// asked for one at all.
fn dim(text: &str, color: bool) -> String {
    if !color {
        return text.to_string();
    }
    let len = u64::try_from(text.len()).unwrap_or(u64::MAX);
    look::painted(text, &[(0, len, look::DIM)])
}

/// The last row where `n` more of the source do not fit at all.
fn omitted(n: usize, view_cols: usize, color: bool) -> String {
    let label = format!("+{n} rows");
    let mut shown = dim(&label, color);
    let pad = view_cols.saturating_sub(label.chars().count());
    shown.extend(std::iter::repeat_n(' ', pad));
    shown
}

/// The frame's cursor as the viewer's own row and column, `None` past
/// either edge: `column` is asked for the cell, a hostile `row` or
/// `col` being its own to guard, not this one's to slice on again.
fn cursor_at(
    screen: &display::Screen,
    usable: usize,
    shown_rows: usize,
) -> Option<(u16, u16)> {
    let row = usize::try_from(screen.row).unwrap_or(usize::MAX);
    if row >= shown_rows {
        return None;
    }
    let cell = column(screen);
    if cell >= usable {
        return None;
    }
    Some((
        u16::try_from(row).unwrap_or(u16::MAX),
        u16::try_from(cell).unwrap_or(u16::MAX),
    ))
}

#[cfg(test)]
mod tests {
    use reovim_client_api::display;
    use reovim_sdk::look;

    use super::painted;

    fn screen(
        rows: u64,
        row: u64,
        col: u64,
        lines: Vec<display::Line>,
        status: &str,
    ) -> display::Screen {
        display::Screen {
            rows,
            row,
            col,
            shape: 0,
            lines,
            status: status.to_string(),
        }
    }

    fn line(text: &str, runs: Vec<display::Run>) -> display::Line {
        display::Line { text: text.to_string(), runs }
    }

    fn run(from: u64, to: u64, style: u64) -> display::Run {
        display::Run { from, to, style }
    }

    /// A viewer bigger than the source both ways fills right and below
    /// with `·`, tmux style.
    #[test]
    fn a_bigger_viewer_fills_right_and_below_with_dots() {
        let lines = vec![line("ab", vec![])];
        let src = screen(2, 0, 0, lines, "st");
        let (rows, _) = painted(&src, 5, (8, 4), false);
        assert_eq!(rows.len(), 4, "the viewer's own rows");
        assert_eq!(rows[0], "ab   ···");
        assert_eq!(rows[1], "st   ···");
        assert_eq!(rows[2], "········");
        assert_eq!(rows[3], "········");
    }

    /// A row too long for a narrower viewer ends in a dim `>`: it is
    /// the row's own content that was cut, not the viewer merely
    /// being the narrower of the two.
    #[test]
    fn a_row_cut_by_a_narrower_viewer_ends_in_a_dim_arrow() {
        let lines = vec![line(&"a".repeat(10), vec![])];
        let src = screen(2, 0, 0, lines, "");
        let (rows, _) = painted(&src, 10, (6, 2), false);
        assert_eq!(
            rows[0], "aaaaa>",
            "five a's, one cell short for the marker"
        );
    }

    /// The marker is dim only when asked to paint in color at all.
    #[test]
    fn the_cut_marker_is_dim_only_in_color() {
        let lines = vec![line(&"a".repeat(10), vec![])];
        let src = screen(2, 0, 0, lines, "");
        let (rows, _) = painted(&src, 10, (6, 2), true);
        let want = format!("aaaaa{}", look::painted(">", &[(0, 1, look::DIM)]));
        assert_eq!(rows[0], want);
    }

    /// A row that fits a narrower viewer whole is not marked cut: a
    /// plain cell stands where the marker would, since nothing of
    /// this row was left out.
    #[test]
    fn a_row_that_fits_a_narrower_viewer_is_not_marked_cut() {
        let lines = vec![line("ab", vec![])];
        let src = screen(2, 0, 0, lines, "");
        let (rows, _) = painted(&src, 10, (6, 2), false);
        assert_eq!(rows[0], "ab    ", "no > on a row nothing was cut from");
    }

    /// A viewer shorter than the source's rows loses the rest of them
    /// to a dim `+N rows` on its own last row.
    #[test]
    fn a_shorter_viewer_collapses_the_rest_into_n_rows() {
        let lines: Vec<_> =
            (0..5).map(|n| line(&format!("row{n}"), vec![])).collect();
        let src = screen(6, 0, 0, lines, "st");
        let (rows, _) = painted(&src, 10, (10, 3), false);
        assert_eq!(rows.len(), 3);
        // Two content rows shown (view_rows - 1), four more not: six
        // source rows less the two shown.
        assert_eq!(rows[2], "+4 rows   ");
    }

    /// A wide char that would straddle the cut boundary is left out
    /// whole, its cell going to the pad and not to a half a glyph.
    #[test]
    fn a_wide_char_at_the_cut_edge_is_left_out_whole() {
        let text = format!("{}{}", "a".repeat(5), '한');
        let lines = vec![line(&text, vec![])];
        let src = screen(2, 0, 0, lines, "");
        // usable is view_cols - 1 = 5: the five a's fill it exactly, so
        // the two-cell char across the boundary is dropped whole.
        let (rows, _) = painted(&src, 20, (6, 2), false);
        assert_eq!(rows[0], "aaaaa>");
    }

    /// The cursor stands at the frame's own row and cell column when
    /// both are inside the viewer's own screen.
    #[test]
    fn the_cursor_is_placed_when_inside_the_viewer() {
        let lines = vec![line("hi", vec![])];
        let src = screen(2, 0, 1, lines, "");
        let (_, cursor) = painted(&src, 10, (10, 3), false);
        assert_eq!(cursor, Some((0, 1)));
    }

    /// A column past what the viewer can show hides the cursor.
    #[test]
    fn the_cursor_is_hidden_past_the_viewers_own_column() {
        let text = "a".repeat(10);
        let lines = vec![line(&text, vec![])];
        let src = screen(2, 0, 9, lines, "");
        let (_, cursor) = painted(&src, 10, (6, 2), false);
        assert_eq!(cursor, None, "column 9 is past the five usable cells");
    }

    /// A row past what the viewer can show (collapsed into `+N rows`)
    /// hides the cursor too.
    #[test]
    fn the_cursor_is_hidden_past_the_viewers_own_row() {
        let lines: Vec<_> =
            (0..5).map(|n| line(&format!("row{n}"), vec![])).collect();
        let src = screen(6, 4, 0, lines, "st");
        let (_, cursor) = painted(&src, 10, (10, 3), false);
        assert_eq!(
            cursor, None,
            "row 4 is past the two rows a 3-row viewer shows"
        );
    }

    /// A run that takes the newline paints one space past the text; a
    /// viewer with room left pads the rest itself, plain.
    #[test]
    fn a_run_taking_the_newline_pads_one_cell_in_its_own_look() {
        let lines = vec![line("ab", vec![run(0, 3, look::INVERSE)])];
        let src = screen(2, 0, 0, lines, "");
        let (rows, _) = painted(&src, 5, (5, 2), true);
        let want =
            format!("{}  ", look::painted("ab", &[(0, 3, look::INVERSE)]));
        assert_eq!(rows[0], want, "one space in the run's own look, two plain");
    }

    /// A hostile frame — huge or zero `rows`, a `row` or a `col` past
    /// everything, a run whose `from` is past its own `to`, one
    /// reaching far past `text`, one that cuts a multi-byte char, and
    /// a viewer of 0 or 1 cells either way — does not panic; that each
    /// call returns at all is what this asks.
    #[test]
    fn a_hostile_frame_does_not_panic() {
        let bad = vec![
            run(5, 2, look::INVERSE),
            run(0, u64::MAX, look::DIM),
            run(2, 5, look::BOLD), // from lands inside '한'
        ];
        let lines = vec![line("a한hostile", bad)];
        let ordinary = screen(2, 0, 3, lines, "status");
        let _ = painted(&ordinary, 4, (3, 2), true);
        let _ = painted(&ordinary, 20, (30, 5), true);

        let huge = screen(u64::MAX, u64::MAX, u64::MAX, Vec::new(), "s");
        let _ = painted(&huge, 0, (0, 0), true);
        let _ = painted(&huge, u64::MAX, (1, 1), true);

        let empty = screen(0, 0, 0, Vec::new(), "");
        let _ = painted(&empty, 5, (5, 5), false);
    }
}
