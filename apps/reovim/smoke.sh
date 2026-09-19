#!/usr/bin/env bash
# The editor role, checked the only way it can be: the real binary in a
# terminal, run by the programs that read $EDITOR. Needs tmux and git.
#
#   apps/reovim/smoke.sh [BINARY]     default target/debug/reovim
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
BIN="$(realpath "${1:-target/debug/reovim}")"
DIR="$(mktemp -d /tmp/reovim-smoke.XXXXXXXX)"
readonly BIN DIR
TMUX=(tmux -L "reovim-smoke-$$")
export XDG_RUNTIME_DIR="$DIR/run"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=smoke GIT_AUTHOR_EMAIL=smoke@reovim
export GIT_COMMITTER_NAME=smoke GIT_COMMITTER_EMAIL=smoke@reovim
export GIT_EDITOR="$BIN" GIT_SEQUENCE_EDITOR="$BIN"
# tmux kills the pane's shell, not the editor under it, so an editor
# that slept through its terminal going away would be left behind
# spinning. The socket names are per-pid but $BIN is not, so a stray
# editor is looked for by binary; check.sh runs the scripts serially.
#
# A run that failed is read from what it left: the panes are gone with
# the server, but $DIR keeps the repository, the files written and each
# pane's exit code, and the way out says where it is. A clean run
# leaves neither the server nor the directory.
cleanup() {
    local code=$? i
    "${TMUX[@]}" kill-server 2>/dev/null || true
    if [[ "$code" == 0 ]]; then
        rm -rf -- "$DIR"
    else
        printf 'smoke.sh: what the run left is under %s\n' "$DIR" >&2
    fi
    for ((i = 0; i < 100; i++)); do
        pgrep -f "^$BIN( |\$)" >/dev/null || exit "$code"
        sleep 0.01
    done
    printf 'smoke.sh: an editor outlived the run\n' >&2
    exit 1
}
trap cleanup EXIT
trap 'exit 1' INT TERM

fail() {
    printf 'smoke.sh: %s\n' "$1" >&2
    exit 1
}

# A pane running `command` in `dir`; its exit code lands in `$DIR/exit`.
start() {
    local name="$1" dir="$2" command="$3"
    "${TMUX[@]}" new-session -d -s "$name" -x 80 -y 24 -c "$dir" \
        "$command; echo \$? > '$DIR/exit-$name'; sleep 30"
}

# Wait up to two seconds for `pattern` on the pane of `name`.
shown() {
    local name="$1" pattern="$2" i
    for ((i = 0; i < 2000; i++)); do
        if "${TMUX[@]}" capture-pane -p -t "$name" | grep -q -- "$pattern"
        then
            return
        fi
        sleep 0.001
    done
    fail "$name never showed $pattern"
}

# Keys one at a time, as a person types them: an Esc that arrives in
# the same read as the next key would be an escape sequence.
keys() {
    local name="$1" key
    shift
    for key in "$@"; do
        "${TMUX[@]}" send-keys -t "$name" "$key"
        sleep 0.02
    done
}

# The exit code of the pane `name`, once it has ended, into `ENDED`. A
# `fail` inside a `$(ended ...)` would end the substitution alone and
# leave the empty string behind, which every caller read as a code, so
# the code is handed back in a variable and read from there.
ENDED=
ended() {
    local name="$1" i
    for ((i = 0; i < 200; i++)); do
        if [[ -f "$DIR/exit-$name" ]]; then
            ENDED="$(cat "$DIR/exit-$name")"
            return
        fi
        sleep 0.01
    done
    fail "$name did not end"
}

# The editor holds the pane; the status row says which file. A line, a
# column and a revision are each at least one digit: `[0-9]*` alone is
# a row with none of them.
editing() {
    shown "$1" "$2  [0-9][0-9]*:[0-9][0-9]*  r[0-9][0-9]*"
}

# How many sockets the runtime directory holds. A directory that is not
# there holds no answer at all, which is not none; the caller says what
# that is worth.
sockets() {
    local names
    names="$(ls -- "$XDG_RUNTIME_DIR")" || return 1
    grep -c '\.sock$' <<< "$names" || true
}

# `value` into the rd `target`. Bytes typed into an rd have one
# spelling in the shell, a run's output through `>`, so the value goes
# to a file on disk first and `cat` of that file writes it; the editor
# boots fs at `/`, so the file is under the run's directory there.
VALUE="/module/fs/root$DIR/value"
readonly VALUE
poured() {
    local target="$1" value="$2"
    printf '%s' "$value" > "$DIR/value"
    "$BIN" cli cat "$VALUE" '>' "$target"
}

# What `/session` holds, once it holds `wanted` Sessions or two seconds
# have gone. A link's end reaches the editor a moment after the cli
# process that held it is done, so the listing is asked for again.
sessions() {
    local wanted="$1" i listed
    for ((i = 0; i < 200; i++)); do
        listed="$("$BIN" cli ls /session)"
        [[ "$(wc -l <<< "$listed")" == "$wanted" ]] && break
        sleep 0.01
    done
    printf '%s\n' "$listed"
}

# --- a repository, a commit message written and one aborted ---------
git init -q "$DIR/repo"
cd "$DIR/repo"
echo one > a
git add a
git commit -q -m first
echo two > a
git add a

start commit "$DIR/repo" "git commit"
editing commit COMMIT_EDITMSG
# "the second" typed, `dw` takes "the ", `yyp` and `.` make three lines,
# `dd` takes one back: a two-line message.
keys commit i "the second" Escape 0 d w y y p . d d : w q Enter
ended commit
[[ "$ENDED" == 0 ]] || fail "git commit failed"
[[ "$(git log -1 --format=%B)" == $'second\nsecond' ]] \
    || fail "message not used: $(git log -1 --format=%B)"

echo three > a
git add a
head="$(git rev-parse HEAD)"
start abort "$DIR/repo" "git commit"
editing abort COMMIT_EDITMSG
keys abort i "third" Escape : c q Enter
ended abort
[[ "$ENDED" != 0 ]] || fail "git commit went on after :cq"
[[ "$(git rev-parse HEAD)" == "$head" ]] || fail "abort committed"

# --- a rebase todo: pick made reword, then the new message ----------
git commit -q -m third
start rebase "$DIR/repo" "git rebase -i HEAD~1"
editing rebase git-rebase-todo
keys rebase x x x x i "reword" Escape : w q Enter
editing rebase COMMIT_EDITMSG
keys rebase : s / t h i r d / r e w o r d e d " " t h i r d / Enter Z Z
ended rebase
[[ "$ENDED" == 0 ]] || fail "rebase failed"
[[ "$(git log -1 --format=%s)" == "reworded third" ]] \
    || fail "reword not used: $(git log -1 --format=%s)"

# --- a new file by absolute path, absent until saved; two at once ---
# A name that is not on disk is no rd: the editor opens a Buffer over
# nothing that keeps the name and says so on the first frame, as nvim
# does, and the first write makes the file.
mkdir "$DIR/new"
start new "$DIR" "$BIN $DIR/new/note"
editing new note
shown new '"new/note" \[New\]'

# dmesg reads the editor's log as the real one does: a stamp on every
# line and the line the editor wrote where it listens; nothing at err
# on a clean run. Then one follows in a pane, and sees what arrives
# while it runs: a connect of its own, and an entry a later cli writes
# with printk's `<4>` in front. SIGINT, which is what Ctrl-C makes on
# a cooked terminal, ends it with exit 0; the pane's own shell is not
# in a job of its own, so the signal is sent to the cli alone.
said="$("$BIN" cli dmesg)"
[[ "$said" == "["* ]] || fail "dmesg has no stamp: $said"
[[ "$said" == *"listen $XDG_RUNTIME_DIR/reovim.sock"* ]] \
    || fail "dmesg does not say where the editor listens: $said"
# An empty answer is only empty where the run that gave it went well:
# a cli that died says nothing either.
said="$("$BIN" cli dmesg -l err)" && code=0 || code=$?
[[ "$code" == 0 ]] || fail "dmesg -l err exits $code"
[[ -z "$said" ]] || fail "dmesg -l err is not empty: $said"
start follow "$DIR" "$BIN cli dmesg -w"
shown follow 'connect session='
poured /sys/log '<4>smoke-follows' >/dev/null
shown follow 'smoke-follows'
pkill -INT -f "^$BIN cli dmesg -w\$"
ended follow
[[ "$ENDED" == 0 ]] || fail "dmesg -w did not end on SIGINT"

# The cli as a shell in a terminal of its own: the prompt says where
# the line stands, `cd` moves it, a listing comes from there, and
# Ctrl-D on an empty line ends the session.
start shell "$DIR" "$BIN cli"
shown shell '^/ >'
keys shell c d " " / m o d u l e Enter
shown shell '^/module >'
keys shell l s Enter
shown shell 'coreutils  fs  sysutils  text'
# `|` puts one run's output into the next run's input; every stage but
# the last writes for no screen, so `grep` matches a name and no escape.
keys shell l s " " "|" " " g r e p " " t e x t Enter
shown shell '^text$'
keys shell p w d Enter
shown shell '^/module$'
keys shell C-d
ended shell
[[ "$ENDED" == 0 ]] || fail "Ctrl-D did not end the cli"

start other "$DIR/new" "$BIN ../other"
editing other other
# The first editor has the default socket, the second one of its own;
# the cli reaches the first and finds the tui that holds its terminal.
open="$(sockets)" || fail "$XDG_RUNTIME_DIR is not there"
[[ "$open" == 2 ]] || fail "two editors have $open sockets"
[[ "$("$BIN" cli ls -a /session/1/client)" == *" 1" ]] \
    || fail "the running editor has no client rd 1"
# What this Client serves is `ls /self/dev` inside it, `/self` being
# on the wire since 05.1; the cell a second Client would read a
# capability from holds nothing, nothing writing it since the same
# minor deleted `Client::capability`.
serves="$("$BIN" cli cat /session/1/client/1/capability)"
[[ -z "$serves" ]] || fail "the running editor's client serves $serves"
# What the link has carried is the Client's own to know, and `link` is
# the five numbers a person reads it as, made at the read: labelled
# text, as `/sys/info/mem`'s three are.
link=/session/1/client/1/self/dev/link
labels="$("$BIN" cli cat $link | awk '{ print $1 }' | tr '\n' ' ')"
[[ "$labels" == "messages bytes_out bytes_in last_us worst_us " ]] \
    || fail "$link says $labels"
carried() {
    "$BIN" cli cat $link | awk '$1 == "messages" { print $2 }'
}
first="$(carried)"
[[ "$first" -gt 0 ]] || fail "the editor's link has carried $first messages"
# A second later it is higher, and by no more than a second of them:
# the editor is idle, so what moves it is the tick's own `Since` and
# the serving of these two reads.
sleep 1
second="$(carried)"
[[ "$second" -gt "$first" ]] \
    || fail "an idle editor's link stood still at $first"
[[ "$((second - first))" -lt 200 ]] \
    || fail "an idle second cost $((second - first)) messages"
# A line that cannot run: one line on stderr, naming the Driver and the
# target, and exit 1.
said="$("$BIN" cli cat /session 2>&1 >/dev/null)" && code=0 || code=$?
[[ "$said" == "reovim: cat /session: unsupported" ]] \
    || fail "a Container read says: $said"
[[ "$code" == 1 ]] || fail "a Container read exits $code"
# Every cli run above made a Session of its own and took it away with
# it; what `/session` holds is the editor's, 1, and the one the cli
# doing the listing stands in.
listed="$(sessions 2)"
[[ "$(wc -l <<< "$listed")" == 2 && "$listed" == $'1\n'* ]] \
    || fail "the cli runs left /session holding $(tr '\n' ' ' <<< "$listed")"

# The log level is the Kernel's mode, printk's console_loglevel made
# the ring's own: at `debug` it keeps a trace of every op and the pair
# it stamps around each `call`, with what the round trip took. `dmesg`
# says the rd as a path and the api and the method by name, from the
# tree and the declarations, since the Kernel writes only numbers.
[[ "$("$BIN" cli cat /sys/loglevel)" == info ]] \
    || fail "the level at boot is $("$BIN" cli cat /sys/loglevel)"
said="$(poured /sys/loglevel loud 2>&1 >/dev/null)" && code=0 || code=$?
[[ "$said" == "reovim: cat $VALUE: invalid" ]] \
    || fail "a token that is not a level says: $said"
[[ "$code" == 1 ]] || fail "a token that is not a level exits $code"
poured /sys/loglevel debug > /dev/null

# The keys below go into the draft this Client holds and cost no call
# at all; the Esc flushes them as one `replace_many`, and the trace
# holds that one.
keys new i "fresh" Escape : q Enter
shown new "E37: No write since last change"
said="$("$BIN" cli dmesg -d -l debug)"
poured /sys/loglevel 6 > /dev/null
[[ "$("$BIN" cli cat /sys/loglevel)" == info ]] \
    || fail "the level written as 6 does not read back as info"
grep -qE '^\[ *[0-9]+\.[0-9]{6} < *[0-9]+\.[0-9]{6}>\] ' <<< "$said" \
    || fail "dmesg -d says no delta: $(head -1 <<< "$said")"
grep -q 'call rd=/module/text/.* api=text method=replace_many dispatch' \
    <<< "$said" \
    || fail "the call pair is not in words: $(grep -m1 'call ' <<< "$said")"
round='api=text method=replace_many reply took=[0-9.]+[^ ]*s heap='
grep -qE "$round[-+][0-9]+$" <<< "$said" \
    || fail "the reply says no round trip: $(grep -m1 reply <<< "$said")"
# No read of the Buffer crosses any more, the draft being the Client's:
# the op the trace holds is the flush's own call.
grep -q 'op=call caller=' <<< "$said" || fail "no op is traced at debug"
[[ "$said" != *"rd=/sys/log"* ]] || fail "the log traced its own reading"
# Nothing crosses while nobody reads: the keys above are a Client's own
# editing, and with no remote watching, no op was asked of it at all.
[[ "$said" != *"serve token="* ]] || fail "the editor served an op unasked"
# Back at `info`, an op that would have been traced adds nothing.
count="$("$BIN" cli dmesg -l debug | wc -l)"
"$BIN" cli ls /module > /dev/null
[[ "$("$BIN" cli dmesg -l debug | wc -l)" == "$count" ]] \
    || fail "the trace goes on after the level says info"

# --- a remote watch on a cell of this Client's own ------------------
# The running editor is the Client on the default socket, so a `watch`
# from outside stands on a mirrored rd of its `/self`. What counts the
# watch is the Kernel's own table: the handle the proxy opened at the
# Client is the Client's and is counted nowhere, so the number moves by
# one and comes back. The pane ends with C-c, as the `dmesg -w` one
# above does.
watched=/session/1/client/1/self/dev/buffers/0
watchers="$("$BIN" cli cat /sys/info/watchers)"
counted() {
    local i
    for ((i = 0; i < 400; i++)); do
        [[ "$("$BIN" cli cat /sys/info/watchers)" == "$1" ]] && return
        sleep 0.01
    done
    fail "/sys/info/watchers is $("$BIN" cli cat /sys/info/watchers), not $1"
}
start told "$DIR" "$BIN cli watch $watched"
counted "$((watchers + 1))"
# One `Modify` a change of the draft; the change is taken back, so the
# file this editor writes below is what it was.
keys new x u
shown told 'Modify'
pkill -INT -f "^$BIN cli watch $watched\$"
ended told
[[ "$ENDED" == 0 ]] || fail "the watcher did not end on SIGINT"
counted "$watchers"

# --- a screen read and a keyboard typed from another terminal -------
# The tui writes what it drew into `display`, so a cli in another
# terminal prints that Client's screen: the pane itself, less the
# cursor line the capture ends with. `keys` types a whole notation into
# that Client's keyboard in one write, which is one tick and not seven.
screen=/session/1/client/1/self/dev/display
board=/session/1/client/1/self/dev/input/keyboard
captured() {
    "$BIN" cli capture "$screen" | head -n -1
}
[[ "$(captured)" == "$("${TMUX[@]}" capture-pane -p -t new)" ]] \
    || fail "the capture is not the pane: $(captured)"
"$BIN" cli keys "$board" 'ihello<Esc>'
shown new hello
[[ "$(captured)" == "$("${TMUX[@]}" capture-pane -p -t new)" ]] \
    || fail "the capture after keys is not the pane: $(captured)"
# One `u` takes the whole insert back, an insert being one revision, so
# the file this editor writes below is what it was.
"$BIN" cli keys "$board" u
for ((i = 0; i < 200; i++)); do
    [[ "$(captured)" == *hello* ]] || break
    sleep 0.01
done
[[ "$(captured)" != *hello* ]] || fail "the undo did not reach the tui"
"$BIN" cli stat /bin/keys \
    | grep -q '0 run({cwd: Rd, target: utf8(256), notation: utf8(1024)})' \
    || fail "keys' run takes: $("$BIN" cli stat /bin/keys)"

# --- /sys as sysfs: the parameters, the facts, the core -------------
# A file per parameter under /sys/limit, each in decimal and each
# settable; a file per fact under /sys/info, each made at the read and
# written by nobody; /sys/core is everything the Kernel holds, saved in
# one read as a core is.
rights="$("$BIN" cli ls -l /sys)"
[[ "$(grep -c . <<< "$rights")" == 6 ]] || fail "/sys holds: $rights"
grep -qE '^- r-  .*  core$' <<< "$rights" || fail "/sys/core is not r"
grep -qE '^c r-  .*  info$' <<< "$rights" || fail "/sys/info is not a c r"
facts="$("$BIN" cli ls -l /sys/info)"
[[ "$(grep -c '^- r-  ' <<< "$facts")" == 5 ]] \
    || fail "the five facts are not read-only: $facts"
limits="$("$BIN" cli ls -l /sys/limit)"
[[ "$(grep -c '^- rw  ' <<< "$limits")" == 5 ]] \
    || fail "the five parameters are not rw: $limits"
[[ "$("$BIN" cli cat /sys/limit/log_ring)" == 4096 ]] \
    || fail "log_ring at boot is $("$BIN" cli cat /sys/limit/log_ring)"
# A ring told to keep eight keeps eight from that write on.
poured /sys/limit/log_ring 8 > /dev/null
[[ "$("$BIN" cli dmesg | wc -l)" == 8 ]] \
    || fail "a ring of eight kept $("$BIN" cli dmesg | wc -l) lines"
"$BIN" cli dmesg -t -l notice \
    | grep -q 'limit name=log_ring was=4096 now=8 caller=' \
    || fail "the parameter write is not in the log it sized"
said="$(poured /sys/limit/reentry x 2>&1 >/dev/null)" && code=0 || code=$?
[[ "$said" == "reovim: cat $VALUE: invalid" ]] \
    || fail "a parameter that is no number says: $said"
[[ "$code" == 1 ]] || fail "a refused parameter exits $code"
poured /sys/limit/log_ring 4096 > /dev/null

[[ "$("$BIN" cli cat /sys/info/mem)" == live=* ]] \
    || fail "/sys/info/mem says $("$BIN" cli cat /sys/info/mem)"
# One fact, one path: what stood loose under /sys is not there.
"$BIN" cli cat /sys/mem > /dev/null 2>&1 && code=0 || code=$?
[[ "$code" == 1 ]] || fail "/sys/mem answered a read and exited $code"
# The clock runs while the server does.
up="$("$BIN" cli cat /sys/info/uptime)"
[[ "$("$BIN" cli cat /sys/info/uptime)" -gt "$up" ]] \
    || fail "/sys/info/uptime stood still at $up"
# The two numbers a person cannot read have a reader under /bin, each
# holding nothing and taking nothing.
said="$("$BIN" cli date)"
grep -qE '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}$' \
    <<< "$said" || fail "date says $said"
said="$("$BIN" cli uptime)"
grep -qE '^up ([0-9]+d )?[0-9]+h [0-9]+m [0-9]+s$' <<< "$said" \
    || fail "uptime says $said"
[[ "$("$BIN" cli ls /bin | wc -w)" == 13 ]] \
    || fail "/bin holds $("$BIN" cli ls /bin)"
"$BIN" cli stat /bin/uptime | grep -q '0 run({}) -> Rd READ' \
    || fail "uptime's run takes a field: $("$BIN" cli stat /bin/uptime)"
"$BIN" cli cat /sys/core > "$DIR/core"
[[ "$(head -1 "$DIR/core")" == limit\ * ]] \
    || fail "a core begins $(head -1 "$DIR/core")"
[[ "$(grep -c '^rd ' "$DIR/core")" -gt 20 ]] \
    || fail "a core holds $(grep -c '^rd ' "$DIR/core") rds"
grep -q '^live=' <<< "$(tail -1 "$DIR/core")" \
    || fail "a core does not end at the heap"

# A boot parameter: an editor started at `debug` is already there.
start sysed "$DIR" "$BIN --socket $DIR/sys.sock --sys loglevel=debug sysfile"
editing sysed sysfile
[[ "$("$BIN" cli --socket "$DIR/sys.sock" cat /sys/loglevel)" == debug ]] \
    || fail "--sys did not set the level before the modules loaded"
keys sysed : q ! Enter
ended sysed
[[ "$ENDED" == 0 ]] || fail "the --sys editor did not quit"

[[ ! -e "$DIR/new/note" ]] || fail "a new file exists before its save"
keys new : w q Enter
keys other i "x" Escape : w Enter : q ! Enter
ended new
[[ "$ENDED" == 0 ]] || fail "the first editor did not quit"
ended other
[[ "$ENDED" == 0 ]] || fail "the second editor did not quit"
[[ "$(cat "$DIR/new/note")" == fresh ]] || fail "new file not written"
[[ "$(cat "$DIR/other")" == x ]] || fail "second editor not written"
open="$(sockets)" || fail "$XDG_RUNTIME_DIR is not there"
[[ "$open" == 0 ]] || fail "a socket was left behind"

# --- a new file left unwritten leaves nothing behind -----------------
start gone "$DIR" "$BIN $DIR/new/gone"
editing gone gone
shown gone '"new/gone" \[New\]'
keys gone : q Enter
ended gone
[[ "$ENDED" == 0 ]] || fail ":q on a new file did not quit"
[[ ! -e "$DIR/new/gone" ]] || fail ":q on a new file left one behind"

# --- a new file below a directory that is not there ------------------
start deep "$DIR" "$BIN $DIR/new/a/b/x"
editing deep x
shown deep '"new/a/b/x" \[New DIRECTORY\]'
keys deep i "deep" Escape : w q Enter
ended deep
[[ "$ENDED" == 0 ]] || fail "the deep new file did not quit"
[[ -d "$DIR/new/a/b" ]] || fail "the directories on the way were not made"
[[ "$(cat "$DIR/new/a/b/x")" == deep ]] || fail "the deep file not written"

# --- a directory: the listing, Enter into a file, :q -----------------
# `reovim .` is a region over a Container, which is the explorer's
# shape and not Vim's: one entry a line, `../` first, then the
# directories with a `/`, then the files. Enter hands the file under
# the cursor to Vim, and the status row is named after it.
mkdir -p "$DIR/tree/sub"
echo one > "$DIR/tree/note"
start explore "$DIR/tree" "$BIN ."
editing explore tree
shown explore '^\.\./'
keys explore j j Enter
editing explore note
shown explore '^one$'
keys explore : q Enter
ended explore
[[ "$ENDED" == 0 ]] || fail ":q left the file the listing opened"

# A directory named on the command line is listed the same way, and
# `:q` in a listing leaves with 0, there being nothing to lose.
start subdir "$DIR" "$BIN tree/sub"
editing subdir sub
shown subdir '^\.\./'
keys subdir : q Enter
ended subdir
[[ "$ENDED" == 0 ]] || fail ":q in the listing did not quit"

# --- :explore from a file: its directory, and back into the file -----
# `:exp[lore]` is Vim asking its Client for another rd in this region.
# With no name it is the directory the file is in, which the table
# gives the listing; Enter on the file comes back to Vim. The swap
# takes the draft with it, so it waits for the bang.
start explored "$DIR/tree" "$BIN note"
editing explored note
keys explored : e x p Enter
editing explored tree
shown explored '^\.\./'
keys explored j j Enter
editing explored note
shown explored '^one$'
keys explored i x Escape : e x p l o r e Enter
shown explored "E37: No write since last change"
keys explored : e x p l o r e ! Enter
editing explored tree
shown explored '^\.\./'
keys explored : q Enter
ended explored
[[ "$ENDED" == 0 ]] || fail ":q left the listing :explore made"
[[ "$(cat "$DIR/tree/note")" == one ]] || fail ":explore! wrote the draft"

# --- :cli: a shell in a region of its own ----------------------------
# `:cli` is Vim asking its Client for this region in the Plugin the
# name `cli` stands for, over where the `:` line's own lines stand:
# the directory the file is in. A line is typed at the prompt, run and
# answered above the next one; Ctrl-D gives the region back to the
# file it came from, which is a fresh Vim over it.
mkdir -p "$DIR/shell"
echo one > "$DIR/shell/note"
echo a > "$DIR/shell/alpha"
start clibuf "$DIR/shell" "$BIN note"
editing clibuf note
keys clibuf : c l i Enter
# The pane keeps no trailing space, so the prompt ends at the `>`.
shown clibuf "^/module/fs/root$DIR/shell >$"
keys clibuf l s Enter
shown clibuf '^alpha  note$'
# The one host that runs with color: what it paints on the way to its
# own region it does not put in a file, a `>` writing for no screen.
: > "$DIR/shell/f"
keys clibuf l s " " / m o d u l e " " ">" " " f Enter
keys clibuf C-d
editing clibuf note
shown clibuf '^one$'
keys clibuf : q Enter
ended clibuf
[[ "$ENDED" == 0 ]] || fail ":q after the cli buffer did not quit"

# --- the : line runs a Driver under /bin too -------------------------
# A first token no name under `/self/vim/bin` begins is the shell's
# line, run over the tree from the directory the file is in: `:ls`
# lists that directory and `:pwd` says where it stands, the first line
# of what it printed in the status row.
mkdir -p "$DIR/words"
echo a > "$DIR/words/alpha"
echo one > "$DIR/words/note"
start words "$DIR/words" "$BIN note"
editing words note
keys words : l s Enter
shown words "note  1:1  r1   alpha"
keys words : p w d Enter
shown words "note  1:1  r1   /module/fs/root$DIR/words"
: > "$DIR/words/f"
keys words : l s " " / m o d u l e " " ">" " " f Enter
shown words "note  1:1  r1 *$"
# The process cli's turn at the same line, while this editor holds the
# socket it reaches.
: > "$DIR/f"
"$BIN" cli ls /module '>' "/module/fs/root$DIR/f"
keys words : q Enter
ended words
[[ "$ENDED" == 0 ]] || fail ":q after a shell line did not quit"

# --- one line, three hosts, one file --------------------------------
# `ls /module > f` from `reovim cli`, from the cli buffer and from
# Vim's `:` line: a redirect runs every stage of its line for no screen
# at all, so neither the buffer's color nor anyone's width reaches the
# target, where 01.8 left the escapes in.
for host in "$DIR/f" "$DIR/shell/f" "$DIR/words/f"; do
    [[ "$(cat "$host")" == $'coreutils\nfs\nsysutils\ntext' ]] \
        || fail "$host holds $(tr '\n' ' ' < "$host")"
done

# --- Tab on the : line: the name expanded, its usage line, the menu --
# A token only one name under PATH begins is expanded to that name and
# the usage line its declaration gives stands in the status row. `:w`
# is three names — Vim's own `write` and `wq`, then `watch` under
# `/bin` — so it is the wildmenu instead, `write` current and the line
# holding it; Tab walks to the next. Enter on the expanded line runs
# it, which writes.
start tab "$DIR/words" "$BIN note"
editing tab note
keys tab : w r Tab
shown tab ":write   write \[path\]"
keys tab Escape : q Tab
shown tab ":quit   quit"
keys tab Escape i x Escape : w Tab
shown tab ":write   \[write\]  wq  watch"
keys tab Tab
shown tab ":wq   write  \[wq\]  watch"
keys tab Escape : w Tab Enter
editing tab note
keys tab : q Enter
ended tab
[[ "$ENDED" == 0 ]] || fail ":q after Tab did not quit"
[[ "$(cat "$DIR/words/note")" == xone ]] || fail "Tab then Enter did not write"

# --- Tab past the name: the argument from the tree -------------------
# A path half typed after a name is completed from the tree the editor
# works over, and the name is written out with it. The space a
# completed file gets is where the next token begins, which is where
# the `x`
# lands; tmux would not show it at the end of the row.
mkdir -p "$DIR/wild"
: > "$DIR/wild/Cargo.toml"
echo one > "$DIR/wild/note"
start wild "$DIR/wild" "$BIN note"
editing wild note
keys wild : w " " C a Tab
shown wild ":write Cargo.toml"
keys wild x
shown wild ":write Cargo.toml x"
keys wild Escape : q Enter
ended wild
[[ "$ENDED" == 0 ]] || fail ":q after the argument Tab did not quit"

# --- no file: an unnamed buffer, E32 on :w, nothing written ---------
start unnamed "$DIR" "$BIN"
editing unnamed '\[No Name\]'
keys unnamed i "hi" Escape : w Enter
shown unnamed "E32: No file name"
keys unnamed : q ! Enter
ended unnamed
[[ "$ENDED" == 0 ]] || fail "the unnamed buffer did not quit"
[[ ! -e "$DIR/[No Name]" && ! -e "$DIR/No Name" ]] \
    || fail "the unnamed buffer wrote a file"

# --- :w NAME from an unnamed buffer names it -------------------------
start named "$DIR" "$BIN"
editing named '\[No Name\]'
keys named i "hi" Escape : w " " n o t e Enter
editing named note
keys named : q Enter
ended named
[[ "$ENDED" == 0 ]] || fail ":w NAME then :q failed"
[[ "$(cat "$DIR/note")" == hi ]] || fail ":w NAME did not write"

# --- SIGTERM: the terminal cooked, the file untouched, exit 1 -------
# The tty is read once the editor has let it go and before the pane's
# own shell ends, so this pane has a line of its own; `start` writes
# the code and sleeps, so the line hands the editor's code on and
# neither is written twice.
start term "$DIR" "$BIN other; code=\$?; \
    stty -a > '$DIR/stty'; (exit \$code)"
editing term other
keys term i "zz" Escape
sleep 0.05
pkill -TERM -f "^$BIN other\$"
ended term
[[ "$ENDED" == 1 ]] || fail "SIGTERM did not exit 1"
grep -q ' icanon' "$DIR/stty" || fail "tty left raw after SIGTERM"
alternate="$("${TMUX[@]}" display -p -t term '#{alternate_on}')"
[[ "$alternate" == 0 ]] || fail "alternate screen left on after SIGTERM"
[[ "$(cat "$DIR/other")" == x ]] || fail "SIGTERM wrote the file"

# --- the terminal gone: the editor goes with it ---------------------
start hangup "$DIR" "$BIN other"
editing hangup other
"${TMUX[@]}" kill-session -t hangup
for ((i = 0; i < 100; i++)); do
    pgrep -f "^$BIN other\$" >/dev/null || break
    sleep 0.01
done
if pgrep -f "^$BIN other\$" >/dev/null; then
    fail "the editor outlived its terminal"
fi

# --- start to first frame -------------------------------------------
before="$EPOCHREALTIME"
start time "$DIR" "$BIN other"
editing time other
after="$EPOCHREALTIME"
keys time : q ! Enter
ended time
[[ "$ENDED" == 0 ]] || fail "the timed editor did not quit"
millis="$(printf '%s %s\n' "$after" "$before" \
    | awk '{ printf "%d", ($1 - $2) * 1000 }')"
printf 'smoke.sh: start to first frame %s ms (tmux included)\n' "$millis"
if [[ "$BIN" == */release/* && "$millis" -ge 50 ]]; then
    fail "first frame took $millis ms"
fi
echo "smoke.sh: the editor role holds"
