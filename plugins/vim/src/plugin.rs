//! `impl Plugin for Vim`: what the keys asked for, done.
//!
//! The engine hands back an `Effect` over its own copy of the text;
//! here it becomes calls. An edit is a `replace`, or an `amend` when
//! it joins the insert under way; undo and redo move the Buffer's
//! head and the text is read back; a save publishes it to its source;
//! a put reads the clipboard rd and a `"+` yank writes it; a quit is
//! `process::exit`, and what all of it looks like is one `grid::show`.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    BAR, BLOCK, Caps, Ctx, Error, Event, Fault, Handle, Hash, Key, Kind, MATCH,
    Plugin, Rights, SELECTION, Start, Tree, grid, keys, process, under,
};
use reovim_text_api::{history, publish, text};

use crate::{Edit, Effect, Mode, Quit, Vim};

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says on a Buffer over nothing.
const E32: &str = "E32: No file name";
/// What undo and redo say at the ends of the history.
const OLDEST: &str = "Already at oldest change";
const NEWEST: &str = "Already at newest change";

impl Plugin for Vim {
    /// The Buffer opened to read and write, the clipboard found if
    /// there is one, the region asked how big it is, and the text read
    /// and shown.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let rights = Rights::READ.union(Rights::WRITE);
        self.h = Some(tree.open(at.buffer, rights)?);
        self.source = at.source;
        self.dir = at.dir;
        self.clip = tree.resolve("/module/clipboard/text").ok();
        self.region = caps.open(grid::hash());
        self.ending = caps.open(process::hash());
        if let Some(region) = self.region {
            self.rows = count(grid::Call(caps, region).size()?.rows);
        }
        let bytes = tree.read_whole(self.handle()?)?;
        self.loaded(&bytes);
        self.begin(count(at.at));
        self.show(caps)
    }

    /// A key run through the engine and its effect performed, or a
    /// resize taken for what the View is laid out over. An input of
    /// anything else is not Vim's.
    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        if from == grid::hash() {
            let resized: grid::Resized = reovim_sdk::decode(bytes)?;
            self.rows = count(resized.rows);
            return self.show(caps);
        }
        if from != keys::hash() {
            return Ok(());
        }
        let press: keys::Press = reovim_sdk::decode(bytes)?;
        let Some(key) = Key::of(press.key) else { return Ok(()) };
        let effect = self.key(key);
        let acted = self.act(tree, caps, effect)?;
        let said = self.write_clipped(tree)?;
        if acted || said {
            self.show(caps)?;
        }
        Ok(())
    }

    /// A Modify on the Buffer, or a ring that overflowed: the text is
    /// read again, which is how another Client's write arrives.
    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let moved = |e: &Event| matches!(e.kind, Kind::Modify | Kind::Overflow);
        if !events.iter().any(moved) {
            return Ok(());
        }
        let bytes = tree.read_whole(self.handle()?)?;
        if !self.loaded(&bytes) {
            return Ok(());
        }
        self.show(caps)
    }
}

impl Vim {
    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Fault> {
        self.h.ok_or(Fault::Kernel(Error::Closed))
    }

    /// What `effect` asks of the Client; true when something is to be
    /// shown again.
    fn act(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        effect: Effect,
    ) -> Result<bool, Fault> {
        match effect {
            Effect::Nothing => return Ok(false),
            Effect::Moved => {}
            Effect::Edit(edit) => self.send(tree, edit)?,
            Effect::Edits(edits) => self.send_each(tree, edits)?,
            Effect::Undo => self.walk(tree, true)?,
            Effect::Redo => self.walk(tree, false)?,
            Effect::Save => self.save(tree)?,
            Effect::SaveAs(name) => self.save_as(tree, &name)?,
            Effect::Clipboard => return self.pasting(tree, caps),
            Effect::Quit(how) => return self.quit(tree, caps, how),
        }
        Ok(true)
    }

    /// `edit` as a new revision, or into the head one when it joins
    /// the insert under way.
    fn send(&self, tree: &mut Tree, edit: Edit) -> Result<(), Fault> {
        let Edit { at, len, text, join } = edit;
        let edit = text::Edit { at: at as u64, len: len as u64, text };
        let mut call = text::Call(tree, self.handle()?);
        if join { call.amend(edit) } else { call.replace(edit) }
    }

    /// Those edits, in order, as a repeat replayed them.
    fn send_each(
        &self,
        tree: &mut Tree,
        edits: Vec<Edit>,
    ) -> Result<(), Fault> {
        for edit in edits {
            self.send(tree, edit)?;
        }
        Ok(())
    }

    /// Head one revision back or on, the text read again after it; the
    /// end of the history is a message.
    fn walk(&mut self, tree: &mut Tree, back: bool) -> Result<(), Fault> {
        let h = self.handle()?;
        let mut call = history::Call(tree, h);
        let (moved, end) =
            if back { (call.undo(), OLDEST) } else { (call.redo(), NEWEST) };
        match moved {
            Ok(()) => {}
            Err(Fault::Kernel(Error::Invalid)) => {
                self.says(end);
                return Ok(());
            }
            Err(fault) => return Err(fault),
        }
        let bytes = tree.read_whole(h)?;
        self.loaded(&bytes);
        Ok(())
    }

    /// The head published to its source; a Buffer over nothing has no
    /// file to write.
    fn save(&mut self, tree: &mut Tree) -> Result<(), Fault> {
        if self.source.is_none() {
            self.message = String::from(E32);
            return Ok(());
        }
        self.fixing(tree)?;
        publish::Call(tree, self.handle()?).save()
    }

    /// `:w NAME`: the head written to that file, `NAME` resolved under
    /// the directory the Client started in. Its own name is a save. A
    /// Buffer over nothing goes on as that file's, its history
    /// starting there.
    fn save_as(&mut self, tree: &mut Tree, name: &str) -> Result<(), Fault> {
        let path = under(&self.dir, name);
        let target = tree.resolve(&format!("/module/fs/root/{path}"))?;
        if Some(target) == self.source {
            return self.save(tree);
        }
        self.fixing(tree)?;
        publish::Call(tree, self.handle()?).save_as(target)?;
        self.source = self.source.or(Some(target));
        Ok(())
    }

    /// The newline `fixendofline` writes at the end of a last line
    /// that has none, before the write itself.
    fn fixing(&mut self, tree: &mut Tree) -> Result<(), Fault> {
        self.fixed_eol().map_or(Ok(()), |edit| self.send(tree, edit))
    }

    /// A put waiting for the clipboard: what it holds, then the put
    /// itself, which is an effect like any other.
    fn pasting(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
    ) -> Result<bool, Fault> {
        let text = self.clipboard(tree)?;
        let effect = self.pasted(text);
        self.act(tree, caps, effect)
    }

    /// What the clipboard holds, through its rd; `None` when there is
    /// no Module or no tool.
    fn clipboard(&self, tree: &mut Tree) -> Result<Option<String>, Fault> {
        let Some(rd) = self.clip else { return Ok(None) };
        match tree.read_all(rd) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(Fault::Kernel(Error::Unsupported | Error::Busy)) => Ok(None),
            Err(fault) => Err(fault),
        }
    }

    /// What a `"+` yank left for the clipboard, written to its rd; no
    /// Module or no tool is a message, which is true when it is one.
    fn write_clipped(&mut self, tree: &mut Tree) -> Result<bool, Fault> {
        let Some(text) = self.clipped() else { return Ok(false) };
        let unsupported = Err(Fault::Kernel(Error::Unsupported));
        let written = self
            .clip
            .map_or(unsupported, |rd| tree.write_all(rd, text.into_bytes()));
        match written {
            Ok(_) => Ok(false),
            Err(Fault::Kernel(Error::Unsupported)) => {
                Ok(self.says("clipboard: no tool on PATH"))
            }
            Err(Fault::Kernel(Error::Busy)) => {
                Ok(self.says("clipboard: the tool failed"))
            }
            Err(fault) => Err(fault),
        }
    }

    /// `:q` while the Buffer holds what its source has not; `:q!` and
    /// `ZQ` whatever it holds; `:wq`, `:x` and `ZZ` written first,
    /// which over nothing leaves an untouched Buffer and is E32 for a
    /// changed one; `:cq` with 1, so the caller uses nothing.
    fn quit(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        how: Quit,
    ) -> Result<bool, Fault> {
        let unsaved = self.unsaved(tree)?;
        let code: u8 = match how {
            Quit::Abort => 1,
            Quit::Ask if unsaved => return Ok(self.says(E37)),
            Quit::Write if self.source.is_some() => {
                self.save(tree)?;
                0
            }
            // Over nothing: `:x` on a changed Buffer has no file.
            Quit::Write if unsaved => return Ok(self.says(E32)),
            _ => 0,
        };
        let Some(ending) = self.ending else { return Ok(false) };
        process::Call(caps, ending).exit(u64::from(code))?;
        Ok(false)
    }

    /// Whether the Buffer holds what its source has not.
    fn unsaved(&self, tree: &mut Tree) -> Result<bool, Fault> {
        let revisions = history::Call(tree, self.handle()?).tree()?;
        Ok(revisions.head != revisions.published)
    }

    /// `message` until the next key; true, as an effect that has
    /// something to show answers.
    fn says(&mut self, message: &str) -> bool {
        self.message = String::from(message);
        true
    }

    /// The View to the region, if the Client has one.
    fn show(&mut self, caps: &mut Caps) -> Result<(), Fault> {
        let Some(region) = self.region else { return Ok(()) };
        let view = self.view(self.rows);
        grid::Call(caps, region).show(view)
    }

    /// What to show in a region of `rows` rows: the first byte of the
    /// first line shown, which moves to keep the cursor in it, the
    /// cursor itself and its shape, the selection and the search
    /// matches as byte spans, and the status word.
    pub fn view(&mut self, rows: usize) -> grid::View {
        let height = rows.max(1);
        let line = self.position().0;
        if line < self.top {
            self.top = line;
        } else if line >= self.top + height {
            self.top = line + 1 - height;
        }
        let selected = match self.mode {
            Mode::Visual(kind) => self.selection_spans(kind, self.top, height),
            _ => Vec::new(),
        };
        let marks = self.search_spans(self.top, height);
        let mut spans: Vec<grid::Span> =
            selected.into_iter().map(span(SELECTION)).collect();
        spans.extend(marks.into_iter().map(span(MATCH)));
        grid::View {
            top: self.line_at(self.top).unwrap_or(0) as u64,
            cursor: self.cursor as u64,
            // Insert puts the cursor between chars, as nvim draws it;
            // every other mode is on one.
            shape: if self.mode == Mode::Insert { BAR } else { BLOCK },
            spans,
            status: self.status(),
        }
    }
}

/// A byte range of the text as a span of `style`.
fn span(style: u64) -> impl Fn((usize, usize)) -> grid::Span {
    move |(from, to)| grid::Span { from: from as u64, to: to as u64, style }
}

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
