#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, keys to calls through a `Client`,
//! a frame from the draft. It sees neither a link nor a terminal.

extern crate alloc;

mod editor;

use alloc::format;
use alloc::string::String;

use reovim_client::{Client, Error, Fault, Handle, Key, Kind, Rd, Rights};
use reovim_text_api::{history, open, publish, text};

pub use editor::{Editor, Effect, Frame};

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    /// Leave with this exit code: 0 says use the file.
    Quit(u8),
}

/// One Buffer over `/module/fs/root/FILE`, watched, with its draft in
/// an `Editor` and its head and published revisions from `tree`. The
/// source is watched too: a save by anyone is a Modify there.
pub struct Tui {
    client: Client,
    editor: Editor,
    buffer: Rd,
    source: Rd,
    h: Handle,
    name: String,
    head: u64,
    published: u64,
    /// A quit was refused on unsaved changes; the next one discards.
    guard: bool,
}

impl Tui {
    /// Open `file` as a Buffer, watch it and load the draft. The name
    /// shown is the source's, read through the `source` pointer.
    ///
    /// # Errors
    /// What the Client says; `Kernel(Invalid)` for a file that is not
    /// under the root.
    pub fn open(mut client: Client, file: &str) -> Result<Self, Fault> {
        client.capability(b"tui".to_vec())?;
        let source = client.resolve(&format!("/module/fs/root/{file}"))?;
        let opener = client.resolve("/module/text/open")?;
        let opening = client.open(opener, Rights::WRITE)?;
        let buffer = open::Call(&mut client, opening).open(source)?;
        client.close(opening)?;
        let pointer = client.lookup(buffer, "source")?;
        let source: Rd = reovim_sdk::decode(&client.read_all(pointer)?)?;
        let name = client.name(source)?;
        let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
        client.watch(h, true)?;
        let watched = client.open(source, Rights::READ)?;
        client.watch(watched, true)?;
        let mut editor = Editor::default();
        editor.loaded(&client.read_whole(h)?);
        client.since()?;
        let mut tui = Self {
            client,
            editor,
            buffer,
            source,
            h,
            name,
            head: 0,
            published: 0,
            guard: false,
        };
        tui.refresh()?;
        Ok(tui)
    }

    /// The Buffer rd.
    #[must_use]
    pub const fn buffer(&self) -> Rd {
        self.buffer
    }

    /// One key: an edit is a `replace`, Ctrl-Z and Ctrl-Y move the
    /// head, Ctrl-S saves. Ctrl-Q quits, refusing once while there is
    /// something unsaved; Ctrl-C aborts with 1.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Fault> {
        let guarded = core::mem::take(&mut self.guard);
        match self.editor.key(key) {
            Effect::Nothing if guarded => return Ok(Step::Changed),
            Effect::Nothing => return Ok(Step::Same),
            Effect::Moved => return Ok(Step::Changed),
            Effect::Edit { at, len, text } => {
                let edit = text::Edit { at: at as u64, len: len as u64, text };
                text::Call(&mut self.client, self.h).replace(edit)?;
            }
            Effect::Undo => {
                history::Call(&mut self.client, self.h).undo()?;
                self.reload()?;
            }
            Effect::Redo => {
                history::Call(&mut self.client, self.h).redo()?;
                self.reload()?;
            }
            Effect::Save => publish::Call(&mut self.client, self.h).save()?,
            Effect::Quit if self.head != self.published && !guarded => {
                self.guard = true;
                return Ok(Step::Changed);
            }
            Effect::Quit => return Ok(Step::Quit(0)),
            Effect::Abort => return Ok(Step::Quit(1)),
        }
        self.refresh()?;
        Ok(Step::Changed)
    }

    /// What happened since: a Modify on the Buffer or its source, or an
    /// Overflow, re-reads the draft and the tree.
    ///
    /// # Errors
    /// `Kernel(Gone)` once the Buffer is gone; otherwise what the
    /// Client says.
    pub fn tick(&mut self) -> Result<Step, Fault> {
        let events = self.client.since()?;
        let on = |rd: Rd, kind: Kind| {
            events.iter().any(|e| e.rd == rd && e.kind == kind)
        };
        if on(self.buffer, Kind::Gone) {
            return Err(Fault::Kernel(Error::Gone));
        }
        // Overflow: the ring moved past us, so everything is read again.
        let overflow = events.iter().any(|e| e.kind == Kind::Overflow);
        if !overflow
            && !on(self.buffer, Kind::Modify)
            && !on(self.source, Kind::Modify)
        {
            return Ok(Step::Same);
        }
        let bytes = self.client.read_whole(self.h)?;
        let text = self.editor.loaded(&bytes);
        let tree = self.refresh()?;
        Ok(if text || tree { Step::Changed } else { Step::Same })
    }

    /// The frame with the status row filled in.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Frame {
        let mut frame = self.editor.frame(cols, rows);
        let mark = if self.head == self.published { "" } else { "[+]" };
        let hint = if self.guard {
            "unsaved: Ctrl-Q again discards, Ctrl-S saves"
        } else {
            "Ctrl-S save  Ctrl-Z undo  Ctrl-Y redo  Ctrl-Q quit"
        };
        frame.status = format!(
            "{}  {}  r{}{mark}   {hint}",
            self.name, frame.status, self.head
        );
        frame
    }

    /// The draft read again after a move of head.
    fn reload(&mut self) -> Result<(), Fault> {
        let bytes = self.client.read_whole(self.h)?;
        self.editor.loaded(&bytes);
        Ok(())
    }

    /// Head and published from `tree`; true when either moved.
    fn refresh(&mut self) -> Result<bool, Fault> {
        let tree = history::Call(&mut self.client, self.h).tree()?;
        let moved = (tree.head, tree.published) != (self.head, self.published);
        (self.head, self.published) = (tree.head, tree.published);
        Ok(moved)
    }
}
