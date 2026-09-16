#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, keys to calls through a `Client`,
//! a frame from the draft. It sees neither a link nor a terminal; the
//! keys are Vim's.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{Client, Error, Fault, Handle, Key, Kind, Rd, Rights};
use reovim_client_vim::{Edit, Effect, Quit, Vim};
use reovim_text_api::{history, open, publish, text};

pub use reovim_client_vim::Frame;

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    /// Leave with this exit code: 0 says use the file.
    Quit(u8),
    /// A put wants the clipboard: read it and answer with `pasted`.
    Clipboard,
}

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";

/// One Buffer over `/module/fs/root/FILE`, watched, with its draft in
/// a `Vim` and its head and published revisions from `tree`. The
/// source is watched too: a save by anyone is a Modify there.
pub struct Tui {
    client: Client,
    vim: Vim,
    buffer: Rd,
    source: Rd,
    h: Handle,
    name: String,
    head: u64,
    published: u64,
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
        let mut vim = Vim::default();
        vim.loaded(&client.read_whole(h)?);
        vim.goto_line(1);
        client.since()?;
        let mut tui = Self {
            client,
            vim,
            buffer,
            source,
            h,
            name,
            head: 0,
            published: 0,
        };
        tui.refresh()?;
        Ok(tui)
    }

    /// The Buffer rd.
    #[must_use]
    pub const fn buffer(&self) -> Rd {
        self.buffer
    }

    /// The Vim behind the keys, for a test to judge its state.
    #[must_use]
    pub const fn vim(&self) -> &Vim {
        &self.vim
    }

    /// One key to Vim: an edit is a `replace`, or an `amend` when it
    /// joins the insert under way; `u` and Ctrl-R move the head; `:w`
    /// saves. `:q` is refused with E37 while there are unsaved
    /// changes, `:q!` leaves them, `:wq` saves first, `:cq` exits 1.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Fault> {
        let effect = self.vim.key(key);
        self.act(effect)
    }

    /// The clipboard's answer to `Step::Clipboard`: `None` when there
    /// is no clipboard.
    ///
    /// # Errors
    /// What the Client says.
    pub fn pasted(&mut self, text: Option<String>) -> Result<Step, Fault> {
        let effect = self.vim.pasted(text);
        self.act(effect)
    }

    /// Text a yank left for the clipboard, once.
    pub const fn take_clipboard(&mut self) -> Option<String> {
        self.vim.take_clipboard()
    }

    /// A message in the status row until the next key.
    pub fn notice(&mut self, message: &str) {
        self.vim.notice(message);
    }

    /// What an effect asks of the Client, and what to draw after.
    fn act(&mut self, effect: Effect) -> Result<Step, Fault> {
        match effect {
            Effect::Nothing => return Ok(Step::Same),
            Effect::Clipboard => return Ok(Step::Clipboard),
            Effect::Moved => return Ok(Step::Changed),
            Effect::Edit(edit) => self.edit(edit)?,
            Effect::Edits(edits) => self.edits(edits)?,
            Effect::Undo => {
                let moved = history::Call(&mut self.client, self.h).undo();
                self.moved(moved, "Already at oldest change")?;
            }
            Effect::Redo => {
                let moved = history::Call(&mut self.client, self.h).redo();
                self.moved(moved, "Already at newest change")?;
            }
            Effect::Save => self.save()?,
            Effect::Quit(Quit::Ask) if self.head != self.published => {
                self.vim.notice(E37);
                return Ok(Step::Changed);
            }
            Effect::Quit(Quit::Ask | Quit::Force) => return Ok(Step::Quit(0)),
            Effect::Quit(Quit::Write) => {
                self.save()?;
                return Ok(Step::Quit(0));
            }
            Effect::Quit(Quit::Abort) => return Ok(Step::Quit(1)),
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
        let text = self.vim.loaded(&bytes);
        let tree = self.refresh()?;
        Ok(if text || tree { Step::Changed } else { Step::Same })
    }

    /// The frame with the status row filled in: the name, the
    /// position, the revision, then Vim's word.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Frame {
        let mut frame = self.vim.frame(cols, rows);
        let (line, column) = self.vim.position();
        let mark = if self.head == self.published { "" } else { "[+]" };
        frame.status = format!(
            "{}  {}:{}  r{}{mark}   {}",
            self.name,
            line + 1,
            column + 1,
            self.head,
            frame.status
        );
        frame
    }

    /// The head written to the source, after the newline Vim adds to
    /// a last line without one.
    fn save(&mut self) -> Result<(), Fault> {
        if let Some(Effect::Edit(edit)) = self.vim.fix_eol() {
            self.edit(edit)?;
        }
        publish::Call(&mut self.client, self.h).save()
    }

    /// The cursor to line `n`, counted from 1: `+N` on the command
    /// line.
    pub fn goto_line(&mut self, n: usize) {
        self.vim.goto_line(n);
    }

    /// `edits` in order, as `.` replayed them.
    fn edits(&mut self, edits: Vec<Edit>) -> Result<(), Fault> {
        for edit in edits {
            self.edit(edit)?;
        }
        Ok(())
    }

    /// `edit` as a new revision, or into the head one when it joins
    /// the insert under way.
    fn edit(&mut self, edit: Edit) -> Result<(), Fault> {
        let Edit { at, len, text, join } = edit;
        let edit = text::Edit { at: at as u64, len: len as u64, text };
        let mut call = text::Call(&mut self.client, self.h);
        if join { call.amend(edit) } else { call.replace(edit) }
    }

    /// After a move of head: the draft read again, or `refused` shown
    /// when the Driver had nowhere to go.
    fn moved(
        &mut self,
        moved: Result<(), Fault>,
        refused: &str,
    ) -> Result<(), Fault> {
        match moved {
            Ok(()) => {}
            Err(Fault::Kernel(Error::Invalid)) => {
                self.vim.notice(refused);
                return Ok(());
            }
            Err(fault) => return Err(fault),
        }
        let bytes = self.client.read_whole(self.h)?;
        self.vim.loaded(&bytes);
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
