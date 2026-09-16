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
    /// Where a relative name in `:w NAME` starts, under the root.
    dir: String,
    name: String,
    head: u64,
    published: u64,
    /// `/module/clipboard/text`, when that Module is there.
    clipboard: Option<Rd>,
}

impl<P: Plugin> Tui<P> {
    /// Open `file`, a name under the root, as a Buffer, or one over
    /// nothing when there is no file; watch it and load the draft
    /// into `plugin`. `dir`, under the root too, is where a relative
    /// `:w NAME` starts.
    ///
    /// # Errors
    /// What the Client says; `Kernel(Invalid)` for a file that is not
    /// under the root.
    pub fn open(
        mut client: Client,
        dir: &str,
        file: Option<&str>,
        mut plugin: P,
    ) -> Result<Self, Fault> {
        client.capability(b"tui".to_vec())?;
        let source = match file {
            Some(file) => Some(file_rd(&mut client, file)?),
            None => None,
        };
        let buffer = buffer_over(&mut client, source)?;
        let name = match source {
            Some(source) => client.name(source)?,
            None => String::from(UNNAMED),
        };
        let h = watched(&mut client, buffer, source)?;
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
            dir: String::from(dir),
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
            Effect::SaveAs(name) => self.save_as(&name)?,
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

    /// `:w NAME`: the draft written to the file `name` through a
    /// Buffer over it, replaced whole and saved; its own name is a
    /// save. A Buffer over nothing goes on as that file's Buffer, its
    /// history starting there.
    fn save_as(&mut self, name: &str) -> Result<(), Fault> {
        let target = file_rd(&mut self.client, &under(&self.dir, name))?;
        if Some(target) == self.source {
            return self.save();
        }
        if let Some(edit) = self.plugin.saving() {
            self.edit(edit)?;
        }
        let draft = self.client.read_whole(self.h)?;
        let buffer = buffer_over(&mut self.client, Some(target))?;
        let h = self.client.open(buffer, Rights::READ.union(Rights::WRITE))?;
        let len = self.client.read_whole(h)?.len() as u64;
        let text = String::from_utf8_lossy(&draft).into_owned();
        text::Call(&mut self.client, h).replace(text::Edit {
            at: 0,
            len,
            text,
        })?;
        publish::Call(&mut self.client, h).save()?;
        self.client.close(h)?;
        if self.source.is_some() {
            return Ok(());
        }
        self.client.close(self.h)?;
        self.h = watched(&mut self.client, buffer, Some(target))?;
        (self.buffer, self.source) = (buffer, Some(target));
        self.name = self.client.name(target)?;
        Ok(())
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

/// The rd of `file`, a name under the root.
fn file_rd(client: &mut Client, file: &str) -> Result<Rd, Fault> {
    client.resolve(&format!("/module/fs/root/{file}"))
}

/// A Buffer over `source`, or over nothing.
fn buffer_over(client: &mut Client, source: Option<Rd>) -> Result<Rd, Fault> {
    let opener = client.resolve("/module/text/open")?;
    let opening = client.open(opener, Rights::WRITE)?;
    let mut open = open::Call(&mut *client, opening);
    let buffer = match source {
        Some(source) => open.open(source),
        None => open.new(),
    };
    client.close(opening)?;
    buffer
}

/// A handle on `buffer`, watched, with the `source` watched too.
fn watched(
    client: &mut Client,
    buffer: Rd,
    source: Option<Rd>,
) -> Result<Handle, Fault> {
    let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
    client.watch(h, true)?;
    if let Some(source) = source {
        let watched = client.open(source, Rights::READ)?;
        client.watch(watched, true)?;
    }
    Ok(h)
}

/// `path` under `dir` unless it is absolute, `.` and `..` resolved
/// lexically, without the leading slash: its name under the root.
/// `dir` is a name under the root already.
#[must_use]
pub fn under(dir: &str, path: &str) -> String {
    let joined = if path.starts_with('/') {
        String::from(path)
    } else {
        format!("{dir}/{path}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::under;

    #[test]
    fn under_names_the_file_under_the_root() {
        assert_eq!(under("home/me", "note"), "home/me/note");
        assert_eq!(under("home/me", "./a/../b/"), "home/me/b");
        assert_eq!(under("home/me", "/tmp/x"), "tmp/x");
        assert_eq!(under("", "../../etc/hosts"), "etc/hosts");
        assert_eq!(under("", "/home/me"), "home/me");
        assert_eq!(under("home/me", "/"), "");
    }
}
