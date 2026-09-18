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
    BAR, BLOCK, Calls, Caps, Ctx, Error, Event, Flags, Handle, Hash, Key, Kind,
    MATCH, Plugin, Rd, Rights, SELECTION, Start, grid, keys, open, process,
};
use reovim_client_sh::{Ran, Shell, complete, run};
use reovim_sdk::path::under;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};
use reovim_text_api::{history, open as text_open, publish, text};

use crate::{Ask, Edit, Effect, Mode, Quit, Vim};

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says on a Buffer over nothing.
const E32: &str = "E32: No file name";
/// What a write says when the file it would make is there already.
const E13: &str = "E13: File exists (add ! to override)";
/// What a write says when the host would not make the file.
const E212: &str = "E212: Can't open file for writing";
/// What a `:` line says of a first token neither Vim's own list nor
/// PATH has.
const E492: &str = "E492: Not an editor command:";
/// What `:explore` says to a name that is not on disk: Vim's word for
/// a directory it cannot find, the name as the person typed it.
const E344: &str = "E344: Can't find directory";
/// What `:edit` says to one: the same, for a file.
const E345: &str = "E345: Can't find file";
/// Where the fs Module mirrors the directory the editor runs over.
const MIRROR: &str = "/module/fs/root";
/// What undo and redo say at the ends of the history.
const OLDEST: &str = "Already at oldest change";
const NEWEST: &str = "Already at newest change";

impl Plugin for Vim {
    /// A Buffer of its own over what the region was opened on, read
    /// and written through a handle it keeps, the clipboard found if
    /// there is one, the region asked how big it is, and the text read
    /// and shown from the line it was told to begin at.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Error> {
        let Ctx { ctx, caps } = ctx;
        let buffer = buffer_over(ctx, at.over)?;
        let rights = Rights::READ.union(Rights::WRITE);
        self.buffer = Some(buffer);
        self.h = Some(ctx.open(buffer, rights)?);
        self.source = at.over;
        self.name = at.name;
        self.dir = at.dir;
        self.shell = standing(ctx, &self.dir)?;
        self.clip = cell::resolve(ctx, ROOT, "/module/clipboard/text").ok();
        self.region = caps.open(grid::hash());
        self.opener = caps.open(open::hash());
        self.ending = caps.open(process::hash());
        if let Some(region) = self.region {
            self.rows = count(grid::Call(caps, region).size()?.rows);
        }
        let bytes = ops::whole(ctx, self.handle()?)?;
        self.loaded(&bytes);
        let at = at.line.map_or(0, |line| self.at_line(count(line)));
        self.begin(at);
        self.announce(ctx)?;
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
    ) -> Result<(), Error> {
        let Ctx { ctx, caps } = ctx;
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
        let acted = self.act(ctx, caps, effect)?;
        let said = self.write_clipped(ctx)?;
        if acted || said {
            self.show(caps)?;
        }
        Ok(())
    }

    /// A Modify on the Buffer, or a ring that overflowed: the text is
    /// read again, which is how another Client's write arrives.
    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Error> {
        let Ctx { ctx, caps } = ctx;
        let moved = |e: &Event| matches!(e.kind, Kind::Modify | Kind::Overflow);
        if !events.iter().any(moved) {
            return Ok(());
        }
        let bytes = ops::whole(ctx, self.handle()?)?;
        if !self.loaded(&bytes) {
            return Ok(());
        }
        self.show(caps)
    }
}

impl Vim {
    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Error> {
        self.h.ok_or(Error::Closed)
    }

    /// The byte line `n` starts at, counted from 1 and clamped to the
    /// last line: the tty's `+N`, which only whoever holds the text
    /// can find.
    fn at_line(&self, n: usize) -> usize {
        let line = n.max(1) - 1;
        self.line_at(line.min(self.line_count() - 1)).unwrap_or(0)
    }

    /// What `effect` asks of the Client; true when something is to be
    /// shown again.
    fn act(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        effect: Effect,
    ) -> Result<bool, Error> {
        match effect {
            Effect::Nothing => return Ok(false),
            Effect::Moved => {}
            Effect::Edit(edit) => self.send(ctx, edit)?,
            Effect::Edits(edits) => self.send_each(ctx, edits)?,
            Effect::Undo => self.walk(ctx, true)?,
            Effect::Redo => self.walk(ctx, false)?,
            Effect::Save(bang) => {
                self.save(ctx, bang)?;
            }
            Effect::SaveAs(name, bang) => {
                self.save_as(ctx, &name, bang)?;
            }
            Effect::Clipboard => return self.pasting(ctx, caps),
            Effect::Quit(how) => return self.quit(ctx, caps, how),
            Effect::Explore(name, bang) => {
                return self.explore(ctx, caps, name.as_deref(), bang);
            }
            Effect::Open(name, bang) => {
                return self.edited(ctx, caps, &name, bang);
            }
            Effect::Line(line) => return Ok(self.ran(ctx, &line)),
            Effect::Cli(bang) => return self.cli(ctx, caps, bang),
            Effect::Complete(ask) => return self.completing(ctx, ask),
        }
        Ok(true)
    }

    /// `edit` as a new revision, or into the head one when it joins
    /// the insert under way.
    fn send(&self, mut ctx: &dyn Calls, edit: Edit) -> Result<(), Error> {
        let Edit { at, len, text, join } = edit;
        let edit = text::Edit { at: at as u64, len: len as u64, text };
        let mut call = text::Call(&mut ctx, self.handle()?);
        if join { call.amend(edit) } else { call.replace(edit) }
    }

    /// Those edits, in order, as a repeat replayed them.
    fn send_each(
        &self,
        ctx: &dyn Calls,
        edits: Vec<Edit>,
    ) -> Result<(), Error> {
        for edit in edits {
            self.send(ctx, edit)?;
        }
        Ok(())
    }

    /// Head one revision back or on, the text read again after it; the
    /// end of the history is a message.
    fn walk(&mut self, mut ctx: &dyn Calls, back: bool) -> Result<(), Error> {
        let h = self.handle()?;
        let mut call = history::Call(&mut ctx, h);
        let (moved, end) =
            if back { (call.undo(), OLDEST) } else { (call.redo(), NEWEST) };
        match moved {
            Ok(()) => {}
            Err(Error::Invalid) => {
                self.says(end);
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        let bytes = ops::whole(ctx, h)?;
        self.loaded(&bytes);
        Ok(())
    }

    /// A Buffer over nothing that was opened for a file says so on
    /// the first frame, as nvim does; the directory it is to go in is
    /// not there either when it says `[New DIRECTORY]`.
    fn announce(&mut self, ctx: &dyn Calls) -> Result<(), Error> {
        let Some(name) = self.name.clone() else { return Ok(()) };
        let dir = name.rsplit_once('/').map_or("", |(dir, _)| dir);
        let word =
            if directory(ctx, dir)? { "[New]" } else { "[New DIRECTORY]" };
        let said = as_given(&self.dir, &name);
        self.message = format!("\"{said}\" {word}");
        Ok(())
    }

    /// The head published to its source; a Buffer over nothing writes
    /// the file it was opened for, and has no file name without one.
    /// False when it said why it did not write.
    fn save(&mut self, mut ctx: &dyn Calls, bang: bool) -> Result<bool, Error> {
        if self.source.is_some() {
            self.fixing(ctx)?;
            publish::Call(&mut ctx, self.handle()?).save()?;
            return Ok(true);
        }
        let Some(name) = self.name.clone() else {
            return Ok(self.says(E32));
        };
        self.written_to(ctx, &name, bang)
    }

    /// `:w NAME`: the head written to that file, `NAME` resolved under
    /// the directory the Client started in. Its own name is a save,
    /// and a name that is not on disk is made first. A Buffer over
    /// nothing goes on as that file's, its history starting there.
    fn save_as(
        &mut self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        let path = under(&self.dir, name);
        let Some(target) = resolved(ctx, &path)? else {
            return self.written_to(ctx, &path, bang);
        };
        if Some(target) == self.source {
            return self.save(ctx, bang);
        }
        self.published_to(ctx, target)
    }

    /// The file `path` names made under the root and the head written
    /// to it: every missing directory on the way by `mkdir`, then
    /// `create`. A file that appeared since is E13 unless `bang` says
    /// to write over it; anything that refuses to make the file, the
    /// host and a file where a directory is wanted alike, is E212.
    fn written_to(
        &mut self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        match making(ctx, path, bang) {
            Ok(Some(target)) => self.published_to(ctx, target),
            Ok(None) => Ok(self.says(E13)),
            // Refused, or asked of an rd that makes nothing: either
            // way there is no file to write.
            Err(_) => Ok(self.says(E212)),
        }
    }

    /// The head written to `target`, which a Buffer over nothing goes
    /// on as, watched by its Client from then on.
    fn published_to(
        &mut self,
        mut ctx: &dyn Calls,
        target: Rd,
    ) -> Result<bool, Error> {
        self.fixing(ctx)?;
        publish::Call(&mut ctx, self.handle()?).save_as(target)?;
        self.source = self.source.or(Some(target));
        Ok(true)
    }

    /// The newline `fixendofline` writes at the end of a last line
    /// that has none, before the write itself.
    fn fixing(&mut self, ctx: &dyn Calls) -> Result<(), Error> {
        self.fixed_eol().map_or(Ok(()), |edit| self.send(ctx, edit))
    }

    /// A put waiting for the clipboard: what it holds, then the put
    /// itself, which is an effect like any other.
    fn pasting(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
    ) -> Result<bool, Error> {
        let text = self.clipboard(ctx)?;
        let effect = self.pasted(text);
        self.act(ctx, caps, effect)
    }

    /// What the clipboard holds, through its rd; `None` when there is
    /// no Module or no tool.
    fn clipboard(&self, ctx: &dyn Calls) -> Result<Option<String>, Error> {
        let Some(rd) = self.clip else { return Ok(None) };
        match ops::bytes(ctx, rd) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(Error::Unsupported | Error::Busy) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// What a `"+` yank left for the clipboard, written to its rd; no
    /// Module or no tool is a message, which is true when it is one.
    fn write_clipped(&mut self, ctx: &dyn Calls) -> Result<bool, Error> {
        let Some(text) = self.clipped() else { return Ok(false) };
        let unsupported = Err(Error::Unsupported);
        let written = self
            .clip
            .map_or(unsupported, |rd| ops::put(ctx, rd, text.into_bytes()));
        match written {
            Ok(_) => Ok(false),
            Err(Error::Unsupported) => {
                Ok(self.says("clipboard: no tool on PATH"))
            }
            Err(Error::Busy) => Ok(self.says("clipboard: the tool failed")),
            Err(error) => Err(error),
        }
    }

    /// `:q` while the Buffer holds what its source has not; `:q!` and
    /// `ZQ` whatever it holds; `:wq`, `:x` and `ZZ` written first,
    /// which over nothing leaves an untouched Buffer and makes the
    /// file of a changed one, or is E32 without a name; `:cq` with 1,
    /// so the caller uses nothing.
    fn quit(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        how: Quit,
    ) -> Result<bool, Error> {
        let unsaved = self.unsaved(ctx)?;
        if how == Quit::Ask && unsaved {
            return Ok(self.says(E37));
        }
        let code = match how {
            Quit::Abort => Some(1),
            Quit::Write => self.written(ctx, unsaved)?,
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
        ctx: &dyn Calls,
        unsaved: bool,
    ) -> Result<Option<u8>, Error> {
        // Over nothing and untouched: `:x` writes nothing, as Vim has
        // it, and there is no file to make.
        if self.source.is_none() && !unsaved {
            return Ok(Some(0));
        }
        Ok(self.save(ctx, false)?.then_some(0))
    }

    /// `:exp[lore][!] [NAME]`: the Client asked to show an rd in this
    /// region, which is the end of this Vim. `NAME` is resolved under
    /// the directory the Client started in, as `:w NAME` is, and with
    /// no name it is the directory the file is in. Whatever is there
    /// goes to the Client's table, a Container to a listing and a cell
    /// to another Vim; a name that is not there is E344. The swap
    /// takes the draft with it, so unsaved changes are E37 without the
    /// bang, as `:q` has it.
    fn explore(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        name: Option<&str>,
        bang: bool,
    ) -> Result<bool, Error> {
        let target = match name {
            Some(name) => resolved(ctx, &under(&self.dir, name))?,
            None => self.here(ctx)?,
        };
        // Nothing is being left for a name that leads nowhere, so
        // there is nothing for the bang to override yet.
        let Some(target) = target else {
            let said = name.unwrap_or(&self.dir);
            return Ok(self.says(&format!("{E344} \"{said}\" in cdpath")));
        };
        self.shown(ctx, caps, target, "", bang)
    }

    /// `:e[dit][!] PATH`: the file that name holds in this region,
    /// which the Client's table gives another Vim. The name is
    /// resolved as `:explore`'s is; one that is not there is E345.
    fn edited(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        name: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        let Some(target) = resolved(ctx, &under(&self.dir, name))? else {
            return Ok(self.says(&format!("{E345} \"{name}\" in path")));
        };
        self.shown(ctx, caps, target, "", bang)
    }

    /// `:cli[!]`: a shell in this region, which the Client's table
    /// gives by name and no shape gives at all. It opens where the `:`
    /// line's own lines stand, the Shell's cwd, so `:cd` moves what
    /// `:cli` opens over as it moves what `:ls` lists.
    fn cli(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        bang: bool,
    ) -> Result<bool, Error> {
        self.shown(ctx, caps, self.shell.cwd(), "cli", bang)
    }

    /// The Client asked to show `target` in this region, in the Plugin
    /// `plugin` names or, with an empty name, in the one its table
    /// gives that rd's shape. This is the end of this Vim: the swap
    /// takes the draft with it, so unsaved changes are E37 without the
    /// bang, as `:q` has it. Nothing is shown again here; the Plugin
    /// that comes shows itself.
    fn shown(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        target: Rd,
        plugin: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        if !bang && self.unsaved(ctx)? {
            return Ok(self.says(E37));
        }
        let Some(opener) = self.opener else { return Ok(false) };
        let ask = open::Ask { target: target.0, plugin: String::from(plugin) };
        open::Call(caps, opener).open(ask)?;
        Ok(false)
    }

    /// A line Vim's list has none of, typed on the `:` line: run
    /// through clients/sh over this tree, from where the Shell stands,
    /// and its first line of output said in the status row, which is
    /// all one row shows. A first token PATH has not is E492, which is
    /// Vim's own; every other `Miss` is said as the shell says it.
    ///
    /// A run that came back open is a Driver that follows: what its
    /// cell holds at this one read is what is said, and the cell is
    /// closed after it, a Plugin having no tick to read it again.
    fn ran(&mut self, ctx: &dyn Calls, line: &str) -> bool {
        let out = match run(ctx, &mut self.shell, line) {
            Ok(ran) => printed(ctx, ran),
            Err(miss) if miss.unknown => format!("{E492} {line}"),
            Err(miss) => format!("{miss}"),
        };
        self.rebased();
        self.says(out.lines().next().unwrap_or_default())
    }

    /// Where `:cd` left the Shell, taken as the directory Vim works
    /// in too, which is what nvim's `:cd` does: `:w NAME`, `:edit
    /// NAME` and `:explore NAME` resolve there from now on, and Tab
    /// completes there. A cwd outside the mirror leaves it where it
    /// was, no file of the editor's being under one.
    fn rebased(&mut self) {
        let path = self.shell.path();
        let Some(rest) = path.strip_prefix(MIRROR) else { return };
        if !(rest.is_empty() || rest.starts_with('/')) {
            return;
        }
        self.dir = rest.trim_start_matches('/').into();
    }

    /// Tab on the `:` line: what may stand where the last word does,
    /// found over the tree, which the line and the wildmenu then take.
    /// The first token is Vim's own names and then PATH's; the words
    /// Vim runs itself take a name under the directory it works in;
    /// every other line is the shell's, which completes it from where
    /// the Shell stands.
    fn completing(&mut self, ctx: &dyn Calls, ask: Ask) -> Result<bool, Error> {
        let (from, found) = match ask {
            Ask::Names { word } => {
                let found = complete(ctx, &self.shell, &word);
                self.words(&word, found.candidates);
                return Ok(true);
            }
            Ask::Path { from, word } => (from, self.names(ctx, &word)?),
            Ask::Line { line } => {
                let found = complete(ctx, &self.shell, &line);
                (found.from, found.candidates)
            }
        };
        self.offer(from, found);
        Ok(true)
    }

    /// The names that may stand where `word` does: the children of
    /// the mirror of the directory Vim works in, or of what `word`
    /// names under it.
    ///
    /// A leading `/` is the mirror's own root, where `:w /NAME`
    /// writes, and not the tree's, so it comes off the word and goes
    /// back on what is found.
    fn names(&self, ctx: &dyn Calls, word: &str) -> Result<Vec<String>, Error> {
        let Some(root) = resolved(ctx, "")? else {
            return Ok(Vec::new());
        };
        let Some(rest) = word.strip_prefix('/') else {
            let under = resolved(ctx, &self.dir)?.unwrap_or(root);
            return Ok(ops::paths(ctx, under, word));
        };
        let found = ops::paths(ctx, root, rest);
        Ok(found.into_iter().map(|name| format!("/{name}")).collect())
    }

    /// Where `:explore` with no name looks: the directory the Buffer's
    /// source is in, or, over nothing, the one the Client started in.
    fn here(&self, ctx: &dyn Calls) -> Result<Option<Rd>, Error> {
        let here = |source| ops::parent(ctx, source).map(Some);
        self.source.map_or_else(|| resolved(ctx, &self.dir), here)
    }

    /// Whether the Buffer holds what its source has not.
    fn unsaved(&self, mut ctx: &dyn Calls) -> Result<bool, Error> {
        let revisions = history::Call(&mut ctx, self.handle()?).tree()?;
        Ok(revisions.head != revisions.published)
    }

    /// `message` until the next key; true, as an effect that has
    /// something to show answers.
    fn says(&mut self, message: &str) -> bool {
        self.message = String::from(message);
        true
    }

    /// The View to the region, if the Client has one.
    fn show(&mut self, caps: &mut Caps) -> Result<(), Error> {
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
            over: self.buffer.map_or(0, |buffer| buffer.0),
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
fn directory(ctx: &dyn Calls, path: &str) -> Result<bool, Error> {
    if path.is_empty() {
        return Ok(true);
    }
    let Some(rd) = resolved(ctx, path)? else { return Ok(false) };
    Ok(ops::info(ctx, rd)?.flags.contains(Flags::CONTAINER))
}

/// The rd `path` names under the root, or `None` when the tree has
/// nothing there: the walk stops at the first name that is not.
fn resolved(ctx: &dyn Calls, path: &str) -> Result<Option<Rd>, Error> {
    match cell::resolve(ctx, ROOT, &format!("{MIRROR}/{path}")) {
        Err(_) => Ok(None),
        found => found.map(Some),
    }
}

/// The rd of the file `path` names under the root, made: every
/// missing directory on the way by `mkdir`, then `create` for the
/// file itself. `None` when a file is there already and `bang` does
/// not say to write over it.
fn making(
    ctx: &dyn Calls,
    path: &str,
    bang: bool,
) -> Result<Option<Rd>, Error> {
    let names: Vec<&str> = path.split('/').filter(|n| !n.is_empty()).collect();
    let Some((name, dirs)) = names.split_last() else {
        return Err(Error::Invalid);
    };
    let mut dir = cell::resolve(ctx, ROOT, MIRROR)?;
    for step in dirs {
        dir = below(ctx, dir, step)?;
    }
    match made(ctx, dir, "create", name) {
        Err(Error::Stale) if bang => ops::lookup(ctx, dir, name).map(Some),
        Err(Error::Stale) => Ok(None),
        made => made.map(Some),
    }
}

/// The directory `name` under `dir`, made when it is not there yet.
fn below(ctx: &dyn Calls, dir: Rd, name: &str) -> Result<Rd, Error> {
    match ops::lookup(ctx, dir, name) {
        Err(Error::Gone) => made(ctx, dir, "mkdir", name),
        found => found,
    }
}

/// `create` or `mkdir` of the fs api on the directory `dir`: the rd
/// it made.
fn made(
    ctx: &dyn Calls,
    dir: Rd,
    method: &str,
    name: &str,
) -> Result<Rd, Error> {
    let input = reovim_sdk::encode(&String::from(name));
    let out = ops::call_by_name(ctx, dir, method, input)?;
    reovim_sdk::decode(&out)
}

/// What a run printed, its cell closed where the run came back open: a
/// Driver that follows is read this once and no more, a Plugin having
/// no tick to read it again.
fn printed(ctx: &dyn Calls, ran: Ran) -> String {
    let Ran { out, open } = ran;
    if let Some(mut open) = open {
        let _ = open.close(ctx);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where the `:` line's shell lines stand: the mirror of the directory
/// the Client started Vim in, so `:ls` lists the buffer's own
/// directory and `:pwd` says where it is. The root of the tree where
/// there is no mirror of that directory, which is a place to stand
/// although no file of the editor's is under it.
fn standing(ctx: &dyn Calls, dir: &str) -> Result<Shell, Error> {
    let path = if dir.is_empty() {
        String::from(MIRROR)
    } else {
        format!("{MIRROR}/{dir}")
    };
    let standing = resolved(ctx, dir)?;
    Ok(standing.map_or_else(Shell::default, |cwd| Shell::at(cwd, path)))
}

/// A Buffer over `source`, or over nothing: the one the Plugin shows,
/// made through `/module/text/open` and kept by the handle `start`
/// opens on it.
fn buffer_over(mut ctx: &dyn Calls, source: Option<Rd>) -> Result<Rd, Error> {
    let opener = cell::resolve(ctx, ROOT, "/module/text/open")?;
    let opening = ctx.open(opener, Rights::WRITE)?;
    let mut call = text_open::Call(&mut ctx, opening);
    let buffer = match source {
        Some(source) => call.open(source),
        None => call.new(),
    };
    ops::close(ctx, opening)?;
    buffer
}

/// A byte range of the text as a span of `style`.
fn span(style: u64) -> impl Fn((usize, usize)) -> grid::Span {
    move |(from, to)| grid::Span { from: from as u64, to: to as u64, style }
}

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
