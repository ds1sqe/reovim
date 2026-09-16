#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, keys to calls through a `Client`,
//! a frame from the draft. It sees neither a link nor a terminal.

extern crate alloc;

mod editor;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::{Client, Error, Fault, Handle, Key, Kind, Rd, Rights};

pub use editor::{Editor, Effect, Frame};

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    Quit,
}

/// One Buffer over `/module/fs/root/FILE`, watched, with its draft in
/// an `Editor`.
pub struct Tui {
    client: Client,
    editor: Editor,
    buffer: Rd,
    h: Handle,
    name: String,
    dirty: bool,
}

impl Tui {
    /// Attach `file`, open the Buffer, watch it and load the draft.
    ///
    /// # Errors
    /// What the Client says; `Kernel(Invalid)` for a file that is not
    /// under the root.
    pub fn open(mut client: Client, file: &str) -> Result<Self, Fault> {
        let source = client.resolve(&format!("/module/fs/root/{file}"))?;
        let attach = client.resolve("/module/buffer/attach")?;
        let input = source.0.to_le_bytes().to_vec();
        let out = client.call_by_name(attach, "attach", input)?;
        let id: [u8; 8] = out[..].try_into().map_err(|_| Fault::Wire)?;
        let buffer = Rd(u64::from_le_bytes(id));
        let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
        client.watch(h, true)?;
        let mut editor = Editor::default();
        editor.loaded(&client.read_whole(h)?);
        client.since()?;
        let name = file.to_string();
        Ok(Self { client, editor, buffer, h, name, dirty: false })
    }

    /// The Buffer rd.
    #[must_use]
    pub const fn buffer(&self) -> Rd {
        self.buffer
    }

    /// One key: a change writes the whole draft, Ctrl-S saves.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Fault> {
        match self.editor.key(key) {
            Effect::Nothing => Ok(Step::Same),
            Effect::Moved => Ok(Step::Changed),
            Effect::Changed => {
                let text = self.editor.text().as_bytes().to_vec();
                self.client.write(self.h, 0, text)?;
                self.dirty = true;
                Ok(Step::Changed)
            }
            Effect::Save => {
                self.client.call_by_name(self.buffer, "save", Vec::new())?;
                self.dirty = false;
                Ok(Step::Changed)
            }
            Effect::Quit => Ok(Step::Quit),
        }
    }

    /// What happened to the Buffer since: a Modify re-reads the draft.
    ///
    /// # Errors
    /// `Kernel(Gone)` once the Buffer is gone; otherwise what the
    /// Client says.
    pub fn tick(&mut self) -> Result<Step, Fault> {
        let events = self.client.since()?;
        let on = |kind: Kind| {
            events.iter().any(|e| e.rd == self.buffer && e.kind == kind)
        };
        if on(Kind::Gone) {
            return Err(Fault::Kernel(Error::Gone));
        }
        if !on(Kind::Modify) {
            return Ok(Step::Same);
        }
        let bytes = self.client.read_whole(self.h)?;
        if !self.editor.loaded(&bytes) {
            return Ok(Step::Same);
        }
        self.dirty = true;
        Ok(Step::Changed)
    }

    /// The frame with the status row filled in.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Frame {
        let mut frame = self.editor.frame(cols, rows);
        let mark = if self.dirty { "  [+]" } else { "" };
        frame.status = format!(
            "{}  {}{mark}   Ctrl-S save  Ctrl-Q quit",
            self.name, frame.status
        );
        frame
    }
}
