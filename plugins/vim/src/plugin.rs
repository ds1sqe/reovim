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
    BAR, BLOCK, Caps, Ctx, Error, Event, Fault, Flags, Handle, Hash, Key, Kind,
    MATCH, Plugin, Rd, Rights, SELECTION, Start, Tree, grid, keys, process,
    under,
};
use reovim_text_api::{history, publish, text};

use crate::{Edit, Effect, Mode, Quit, Vim};

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says on a Buffer over nothing.
const E32: &str = "E32: No file name";
/// What a write says when the file it would make is there already.
const E13: &str = "E13: File exists (add ! to override)";
/// What a write says when the host would not make the file.
const E212: &str = "E212: Can't open file for writing";
/// Where the fs Module mirrors the directory the editor runs over.
const MIRROR: &str = "/module/fs/root";
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
        self.name = at.name;
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
        self.announce(tree)?;
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
            Effect::Save(bang) => {
                self.save(tree, bang)?;
            }
            Effect::SaveAs(name, bang) => {
                self.save_as(tree, &name, bang)?;
            }
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

    /// A Buffer over nothing that was opened for a file says so on
    /// the first frame, as nvim does; the directory it is to go in is
    /// not there either when it says `[New DIRECTORY]`.
    fn announce(&mut self, tree: &mut Tree) -> Result<(), Fault> {
        let Some(name) = self.name.clone() else { return Ok(()) };
        let dir = name.rsplit_once('/').map_or("", |(dir, _)| dir);
        let word =
            if directory(tree, dir)? { "[New]" } else { "[New DIRECTORY]" };
        let said = as_given(&self.dir, &name);
        self.message = format!("\"{said}\" {word}");
        Ok(())
    }

    /// The head published to its source; a Buffer over nothing writes
    /// the file it was opened for, and has no file name without one.
    /// False when it said why it did not write.
    fn save(&mut self, tree: &mut Tree, bang: bool) -> Result<bool, Fault> {
        if self.source.is_some() {
            self.fixing(tree)?;
            publish::Call(tree, self.handle()?).save()?;
            return Ok(true);
        }
        let Some(name) = self.name.clone() else {
            return Ok(self.says(E32));
        };
        self.written_to(tree, &name, bang)
    }

    /// `:w NAME`: the head written to that file, `NAME` resolved under
    /// the directory the Client started in. Its own name is a save,
    /// and a name that is not on disk is made first. A Buffer over
    /// nothing goes on as that file's, its history starting there.
    fn save_as(
        &mut self,
        tree: &mut Tree,
        name: &str,
        bang: bool,
    ) -> Result<bool, Fault> {
        let path = under(&self.dir, name);
        let Some(target) = resolved(tree, &path)? else {
            return self.written_to(tree, &path, bang);
        };
        if Some(target) == self.source {
            return self.save(tree, bang);
        }
        self.published_to(tree, target)
    }

    /// The file `path` names made under the root and the head written
    /// to it: every missing directory on the way by `mkdir`, then
    /// `create`. A file that appeared since is E13 unless `bang` says
    /// to write over it; anything that refuses to make the file, the
    /// host and a file where a directory is wanted alike, is E212.
    fn written_to(
        &mut self,
        tree: &mut Tree,
        path: &str,
        bang: bool,
    ) -> Result<bool, Fault> {
        match making(tree, path, bang) {
            Ok(Some(target)) => self.published_to(tree, target),
            Ok(None) => Ok(self.says(E13)),
            // Refused, or asked of an rd that makes nothing: either
            // way there is no file to write. Only the link is fatal.
            Err(Fault::Kernel(_) | Fault::Usage(_)) => Ok(self.says(E212)),
            Err(fault) => Err(fault),
        }
    }

    /// The head written to `target`, which a Buffer over nothing goes
    /// on as, watched by its Client from then on.
    fn published_to(
        &mut self,
        tree: &mut Tree,
        target: Rd,
    ) -> Result<bool, Fault> {
        self.fixing(tree)?;
        publish::Call(tree, self.handle()?).save_as(target)?;
        self.source = self.source.or(Some(target));
        Ok(true)
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
    /// which over nothing leaves an untouched Buffer and makes the
    /// file of a changed one, or is E32 without a name; `:cq` with 1,
    /// so the caller uses nothing.
    fn quit(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        how: Quit,
    ) -> Result<bool, Fault> {
        let unsaved = self.unsaved(tree)?;
        if how == Quit::Ask && unsaved {
            return Ok(self.says(E37));
        }
        let code = match how {
            Quit::Abort => Some(1),
            Quit::Write => self.written(tree, unsaved)?,
            _ => Some(0),
        };
        // The write said why it did not happen; the editor stays.
        let Some(code) = code else { return Ok(true) };
        let Some(ending) = self.ending else { return Ok(false) };
        process::Call(caps, ending).exit(u64::from(code))?;
        Ok(false)
    }

    /// What `:wq`, `:x` and `ZZ` write before they leave: the source
    /// of a Buffer that has one, and the file a changed Buffer over
    /// nothing was opened for. `None` when that write did not happen,
    /// and the editor stays where it is having said why.
    fn written(
        &mut self,
        tree: &mut Tree,
        unsaved: bool,
    ) -> Result<Option<u8>, Fault> {
        // Over nothing and untouched: `:x` writes nothing, as Vim has
        // it, and there is no file to make.
        if self.source.is_none() && !unsaved {
            return Ok(Some(0));
        }
        Ok(self.save(tree, false)?.then_some(0))
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

/// The file as the person named it: `name`, a path under the root,
/// read from `dir`, the directory the Client started in, when it is
/// below it, as nvim says the path it was given.
fn as_given<'a>(dir: &str, name: &'a str) -> &'a str {
    name.strip_prefix(dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(name)
}

/// Whether `path` under the root is a directory there; the root
/// itself, which the empty path names, always is.
fn directory(tree: &mut Tree, path: &str) -> Result<bool, Fault> {
    if path.is_empty() {
        return Ok(true);
    }
    let Some(rd) = resolved(tree, path)? else { return Ok(false) };
    Ok(tree.info(rd)?.flags.contains(Flags::CONTAINER))
}

/// The rd `path` names under the root, or `None` when the tree has
/// nothing there: the walk stops at the first name that is not.
fn resolved(tree: &mut Tree, path: &str) -> Result<Option<Rd>, Fault> {
    match tree.resolve(&format!("{MIRROR}/{path}")) {
        Err(Fault::Kernel(_)) => Ok(None),
        found => found.map(Some),
    }
}

/// The rd of the file `path` names under the root, made: every
/// missing directory on the way by `mkdir`, then `create` for the
/// file itself. `None` when a file is there already and `bang` does
/// not say to write over it.
fn making(
    tree: &mut Tree,
    path: &str,
    bang: bool,
) -> Result<Option<Rd>, Fault> {
    let names: Vec<&str> = path.split('/').filter(|n| !n.is_empty()).collect();
    let Some((name, dirs)) = names.split_last() else {
        return Err(Fault::Kernel(Error::Invalid));
    };
    let mut dir = tree.resolve(MIRROR)?;
    for step in dirs {
        dir = below(tree, dir, step)?;
    }
    match made(tree, dir, "create", name) {
        Err(Fault::Kernel(Error::Stale)) if bang => {
            tree.lookup(dir, name).map(Some)
        }
        Err(Fault::Kernel(Error::Stale)) => Ok(None),
        made => made.map(Some),
    }
}

/// The directory `name` under `dir`, made when it is not there yet.
fn below(tree: &mut Tree, dir: Rd, name: &str) -> Result<Rd, Fault> {
    match tree.lookup(dir, name) {
        Err(Fault::Kernel(Error::Gone)) => made(tree, dir, "mkdir", name),
        found => found,
    }
}

/// `create` or `mkdir` of the fs api on the directory `dir`: the rd
/// it made.
fn made(
    tree: &mut Tree,
    dir: Rd,
    method: &str,
    name: &str,
) -> Result<Rd, Fault> {
    let input = reovim_sdk::encode(&String::from(name));
    let out = tree.call_by_name(dir, method, input)?;
    Ok(reovim_sdk::decode(&out)?)
}

/// A byte range of the text as a span of `style`.
fn span(style: u64) -> impl Fn((usize, usize)) -> grid::Span {
    move |(from, to)| grid::Span { from: from as u64, to: to as u64, style }
}

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
