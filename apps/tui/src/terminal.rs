use crossterm::{cursor, event, execute, terminal};
use reovim_client_tui::viewport;
use std::io::{self, Write};

pub struct Terminal {
    raw: bool,
    screen: bool,
}

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        let mut guard = Self { raw: false, screen: false };
        terminal::enable_raw_mode()?;
        guard.raw = true;
        // A failed write may have applied part of the terminal sequence.
        guard.screen = true;
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            event::EnableBracketedPaste,
            cursor::Hide
        )?;
        Ok(guard)
    }

    pub fn restore(&mut self) -> io::Result<()> {
        let screen = if self.screen {
            execute!(
                io::stdout(),
                event::DisableBracketedPaste,
                cursor::Show,
                terminal::LeaveAlternateScreen
            )
        } else {
            Ok(())
        };
        if screen.is_ok() {
            self.screen = false;
        }
        let raw = if self.raw { terminal::disable_raw_mode() } else { Ok(()) };
        if raw.is_ok() {
            self.raw = false;
        }
        screen.and(raw)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn paint(
    lines: &[String],
    selected: Option<(u16, u16)>,
    status: &str,
    columns: u16,
) -> io::Result<()> {
    let mut output = io::stdout();
    execute!(
        output,
        cursor::Hide,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All)
    )?;
    write!(output, "{}", viewport::status(status, columns))?;
    for (row, text) in lines.iter().enumerate() {
        execute!(
            output,
            cursor::MoveTo(
                0,
                u16::try_from(row + 1).map_err(io::Error::other)?
            )
        )?;
        write!(output, "{text}")?;
    }
    if let Some((column, row)) = selected {
        execute!(output, cursor::MoveTo(column, row + 1), cursor::Show)?;
    }
    output.flush()
}
