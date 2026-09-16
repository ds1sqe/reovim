#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, keys to calls through a `Client`,
//! a frame from the draft. It sees neither a link nor a terminal.

extern crate alloc;

mod editor;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::value::{self, Value};
use reovim_client::{
    Client, Decl, Error, Fault, Handle, Hash, Key, Kind, Method, Rd, Rights,
};

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
    /// What the Buffer offers, read once.
    decls: Vec<(Hash, Decl)>,
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
        let open = client.resolve("/module/text/open")?;
        let input = source.0.to_le_bytes().to_vec();
        let out = client.call_by_name(open, "open", input)?;
        let buffer = Rd(rd_of(&out)?);
        let pointer = client.lookup(buffer, "source")?;
        let source = Rd(rd_of(&client.read_all(pointer)?)?);
        let name = client.name(source)?;
        let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
        client.watch(h, true)?;
        let watched = client.open(source, Rights::READ)?;
        client.watch(watched, true)?;
        let decls = client.decls(buffer)?;
        let mut editor = Editor::default();
        editor.loaded(&client.read_whole(h)?);
        client.since()?;
        let mut tui = Self {
            client,
            editor,
            buffer,
            source,
            h,
            decls,
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
                self.call("replace", edit(at, len, &text)?)?;
            }
            Effect::Undo => self.moved("undo")?,
            Effect::Redo => self.moved("redo")?,
            Effect::Save => {
                self.call("save", Vec::new())?;
            }
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

    /// The method `name` among what the Buffer offers.
    fn method(&self, name: &str) -> Result<(Hash, Method), Fault> {
        let found = self.decls.iter().find_map(|(hash, decl)| {
            decl.methods
                .iter()
                .find(|m| m.name == name)
                .map(|m| (*hash, m.clone()))
        });
        found.ok_or_else(|| Fault::Usage(format!("no method `{name}`")))
    }

    /// `name` on the Buffer's own handle.
    fn call(&mut self, name: &str, input: Vec<u8>) -> Result<Vec<u8>, Fault> {
        let (hash, method) = self.method(name)?;
        self.client.call(self.h, hash, method.number, input)
    }

    /// A move of head; the draft is read again.
    fn moved(&mut self, name: &str) -> Result<(), Fault> {
        self.call(name, Vec::new())?;
        let bytes = self.client.read_whole(self.h)?;
        self.editor.loaded(&bytes);
        Ok(())
    }

    /// Head and published from `tree`; true when either moved.
    fn refresh(&mut self) -> Result<bool, Fault> {
        let (_, method) = self.method("tree")?;
        let tree =
            value::decode(&method.output, &self.call("tree", Vec::new())?)?;
        let number = |name| tree.field(name).and_then(Value::number);
        let (head, published) = (
            number("head").ok_or(Fault::Wire)?,
            number("published").ok_or(Fault::Wire)?,
        );
        let moved = (head, published) != (self.head, self.published);
        (self.head, self.published) = (head, published);
        Ok(moved)
    }
}

/// The `Edit` record of `replace`.
fn edit(at: usize, len: usize, text: &str) -> Result<Vec<u8>, Fault> {
    let mut input = (at as u64).to_le_bytes().to_vec();
    input.extend_from_slice(&(len as u64).to_le_bytes());
    let n = u32::try_from(text.len()).map_err(|_| Fault::Wire)?;
    input.extend_from_slice(&n.to_le_bytes());
    input.extend_from_slice(text.as_bytes());
    Ok(input)
}

/// An `Rd` answer.
fn rd_of(bytes: &[u8]) -> Result<u64, Fault> {
    let id: [u8; 8] = bytes.try_into().map_err(|_| Fault::Wire)?;
    Ok(u64::from_le_bytes(id))
}
