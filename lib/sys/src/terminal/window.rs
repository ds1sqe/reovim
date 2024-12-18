use rustix::termios::Winsize;

#[derive(Debug)]
pub struct WindowSize {
    pub rows: u16,
    pub columns: u16,
    pub width: u16,
    pub height: u16,
}

impl From<Winsize> for WindowSize {
    fn from(size: Winsize) -> WindowSize {
        WindowSize {
            columns: size.ws_col,
            rows: size.ws_row,
            width: size.ws_xpixel,
            height: size.ws_ypixel,
        }
    }
}
