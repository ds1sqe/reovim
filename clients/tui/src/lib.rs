#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, keys to calls through a `Client`,
//! a frame from the draft, the clipboard an rd like any other. It
//! sees neither a link nor a terminal; the keys are a `Plugin`'s to
//! read.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    Client, Edit, Effect, Error, Fault, Handle, Key, Kind, Plugin, Quit, Rd,
    Rights,
};
use reovim_text_api::{history, open, publish, text};

pub use reovim_client::Frame;

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    /// Leave with this exit code: 0 says use the file.
    Quit(u8),
}

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says on a Buffer over nothing.
const E32: &str = "E32: No file name";
/// The name of a Buffer over nothing, as nvim shows it.
const UNNAMED: &str = "[No Name]";

/// One Buffer over `/module/fs/root/FILE`, or over nothing, watched.
///
/// Its draft is in a `Plugin`, its head and published revisions from
/// `tree`. The source is watched too: a save by anyone is a Modify
/// there.
pub struct Tui<P: Plugin> {
    client: Client,
    plugin: P,
    buffer: Rd,
    source: Option<Rd>,
    h: Handle,
    name: String,
    head: u64,
    published: u64,
    /// `/module/clipboard/text`, when that Module is there.
    clipboard: Option<Rd>,
}

impl<P: Plugin> Tui<P> {
    /// Open `file` as a Buffer, or one over nothing when there is no
    /// file, watch it and load the draft into `plugin`.
    ///
    /// # Errors
    /// What the Client says; `Kernel(Invalid)` for a file that is not
    /// under the root.
    pub fn open(
        mut client: Client,
        file: Option<&str>,
        mut plugin: P,
    ) -> Result<Self, Fault> {
        client.capability(b"tui".to_vec())?;
        let source = match file {
            Some(file) => {
                Some(client.resolve(&format!("/module/fs/root/{file}"))?)
            }
            None => None,
        };
        let opener = client.resolve("/module/text/open")?;
        let opening = client.open(opener, Rights::WRITE)?;
        let mut open = open::Call(&mut client, opening);
        let buffer = match source {
            Some(source) => open.open(source)?,
            None => open.new()?,
        };
        client.close(opening)?;
        let name = match source {
            Some(source) => client.name(source)?,
            None => String::from(UNNAMED),
        };
        let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
        client.watch(h, true)?;
        if let Some(source) = source {
            let watched = client.open(source, Rights::READ)?;
            client.watch(watched, true)?;
        }
        plugin.loaded(&client.read_whole(h)?);
        plugin.goto_line(1);
        let clipboard = client.resolve("/module/clipboard/text").ok();
        client.since()?;
        let mut tui = Self {
            client,
            plugin,
            buffer,
            source,
            h,
            name,
            head: 0,
            published: 0,
            clipboard,
        };
        tui.refresh()?;
        Ok(tui)
    }

    /// The Buffer rd.
    #[must_use]
    pub const fn buffer(&self) -> Rd {
        self.buffer
    }

    /// The Plugin behind the keys, for a test to judge its state.
    #[must_use]
    pub const fn plugin(&self) -> &P {
        &self.plugin
    }

    /// One key to the Plugin: an edit is a `replace`, or an `amend`
    /// when it joins the insert under way; undo and redo move the
    /// head; a save writes; a put from the clipboard reads its rd and
    /// a yank into it writes there. `Quit::Ask` is refused with E37
    /// while there are unsaved changes, `Force` leaves them, `Write`
    /// saves first, `Abort` exits 1.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Fault> {
        let effect = self.plugin.key(key);
        let step = self.act(effect)?;
        self.yanked()?;
        Ok(step)
    }

    /// A message in the status row until the next key.
    pub fn notice(&mut self, message: &str) {
        self.plugin.notice(message);
    }

    /// What an effect asks of the Client, and what to draw after.
    fn act(&mut self, effect: Effect) -> Result<Step, Fault> {
        match effect {
            Effect::Nothing => return Ok(Step::Same),
            Effect::Clipboard => {
                let text = self.clipboard()?;
                let effect = self.plugin.pasted(text);
                return self.act(effect);
            }
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
                self.plugin.notice(E37);
                return Ok(Step::Changed);
            }
            Effect::Quit(Quit::Ask | Quit::Force) => return Ok(Step::Quit(0)),
            // Over nothing: `:x` on an untouched Buffer leaves, a changed
            // one has no file to write.
            Effect::Quit(Quit::Write)
                if self.source.is_none() && self.head == self.published =>
            {
                return Ok(Step::Quit(0));
            }
            Effect::Quit(Quit::Write) if self.source.is_none() => {
                self.plugin.notice(E32);
                return Ok(Step::Changed);
            }
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
        let source = self.source.is_some_and(|s| on(s, Kind::Modify));
        if !overflow && !on(self.buffer, Kind::Modify) && !source {
            return Ok(Step::Same);
        }
        let bytes = self.client.read_whole(self.h)?;
        let text = self.plugin.loaded(&bytes);
        let tree = self.refresh()?;
        Ok(if text || tree { Step::Changed } else { Step::Same })
    }

    /// The frame with the status row filled in: the name, the
    /// position, the revision, then the Plugin's word.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Frame {
        let mut frame = self.plugin.frame(cols, rows);
        let (line, column) = self.plugin.position();
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

    /// What the clipboard holds, through its rd; `None` when there is
    /// no Module or no tool.
    fn clipboard(&mut self) -> Result<Option<String>, Fault> {
        let Some(rd) = self.clipboard else { return Ok(None) };
        match self.client.read_all(rd) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(Fault::Kernel(Error::Unsupported | Error::Busy)) => Ok(None),
            Err(fault) => Err(fault),
        }
    }

    /// What a yank left for the clipboard, written to its rd; no
    /// Module or no tool is a message.
    fn yanked(&mut self) -> Result<(), Fault> {
        let Some(text) = self.plugin.take_clipboard() else { return Ok(()) };
        let unsupported = Err(Fault::Kernel(Error::Unsupported));
        let written = self.clipboard.map_or(unsupported, |rd| {
            self.client.write_all(rd, text.into_bytes())
        });
        match written {
            Ok(_) => Ok(()),
            Err(Fault::Kernel(Error::Unsupported)) => {
                self.plugin.notice("clipboard: no tool on PATH");
                Ok(())
            }
            Err(Fault::Kernel(Error::Busy)) => {
                self.plugin.notice("clipboard: the tool failed");
                Ok(())
            }
            Err(fault) => Err(fault),
        }
    }

    /// The head written to the source, after the Plugin's last word;
    /// E32 over nothing.
    fn save(&mut self) -> Result<(), Fault> {
        if self.source.is_none() {
            self.plugin.notice(E32);
            return Ok(());
        }
        if let Some(edit) = self.plugin.saving() {
            self.edit(edit)?;
        }
        publish::Call(&mut self.client, self.h).save()
    }

    /// The cursor to line `n`, counted from 1: `+N` on the command
    /// line.
    pub fn goto_line(&mut self, n: usize) {
        self.plugin.goto_line(n);
    }

    /// `edits` in order, as a repeat replayed them.
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
                self.plugin.notice(refused);
                return Ok(());
            }
            Err(fault) => return Err(fault),
        }
        let bytes = self.client.read_whole(self.h)?;
        self.plugin.loaded(&bytes);
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
