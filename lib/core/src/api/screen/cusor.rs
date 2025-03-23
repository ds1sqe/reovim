use reovim_sys::{cursor, queue};

use super::{Position, Screen};

pub struct Cusor;

impl Cusor {
    pub fn move_to(
        scr: &mut Screen,
        pos: &Position,
    ) -> std::result::Result<(), std::io::Error> {
        queue!(scr.out_stream, cursor::MoveTo(pos.x, pos.y))
    }
}
