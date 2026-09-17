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
cleanup() {
    local code=$? i
    "${TMUX[@]}" kill-server 2>/dev/null || true
    rm -rf -- "$DIR"
    for ((i = 0; i < 100; i++)); do
        pgrep -f "^$BIN( |\$)" >/dev/null || exit "$code"
        sleep 0.01
    done
    printf 'smoke.sh: an editor outlived the run\n' >&2
    exit 1
}
trap cleanup EXIT

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

# The exit code of the pane `name`, once it has ended.
ended() {
    local name="$1" i
    for ((i = 0; i < 200; i++)); do
        [[ -f "$DIR/exit-$name" ]] && { cat "$DIR/exit-$name"; return; }
        sleep 0.01
    done
    fail "$name did not end"
}

# The editor holds the pane; the status row says which file.
editing() {
    shown "$1" "$2  [0-9]*:[0-9]*  r[0-9]*"
}

# How many sockets the runtime directory holds.
sockets() {
    ls "$XDG_RUNTIME_DIR" | grep -c '\.sock$' || true
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
[[ "$(ended commit)" == 0 ]] || fail "git commit failed"
[[ "$(git log -1 --format=%B)" == $'second\nsecond' ]] \
    || fail "message not used: $(git log -1 --format=%B)"

echo three > a
git add a
head="$(git rev-parse HEAD)"
start abort "$DIR/repo" "git commit"
editing abort COMMIT_EDITMSG
keys abort i "third" Escape : c q Enter
[[ "$(ended abort)" != 0 ]] || fail "git commit went on after :cq"
[[ "$(git rev-parse HEAD)" == "$head" ]] || fail "abort committed"

# --- a rebase todo: pick made reword, then the new message ----------
git commit -q -m third
start rebase "$DIR/repo" "git rebase -i HEAD~1"
editing rebase git-rebase-todo
keys rebase x x x x i "reword" Escape : w q Enter
editing rebase COMMIT_EDITMSG
keys rebase : s / t h i r d / r e w o r d e d " " t h i r d / Enter Z Z
[[ "$(ended rebase)" == 0 ]] || fail "rebase failed"
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
[[ -z "$("$BIN" cli dmesg -l err)" ]] || fail "dmesg -l err is not empty"
start follow "$DIR" "$BIN cli dmesg -w"
shown follow 'connect session='
"$BIN" cli write /sys/log '<4>smoke-follows' >/dev/null
shown follow 'smoke-follows'
pkill -INT -f "^$BIN cli dmesg -w\$"
[[ "$(ended follow)" == 0 ]] || fail "dmesg -w did not end on SIGINT"

# The cli as a shell in a terminal of its own: the prompt says where
# the line stands, `cd` moves it, a listing comes from there, and
# Ctrl-D on an empty line ends the session.
start shell "$DIR" "$BIN cli"
shown shell '^/ >'
keys shell c d " " / m o d u l e Enter
shown shell '^/module >'
keys shell l s Enter
shown shell 'clipboard  fs  text'
keys shell p w d Enter
shown shell '^/module$'
keys shell C-d
[[ "$(ended shell)" == 0 ]] || fail "Ctrl-D did not end the cli"

start other "$DIR/new" "$BIN ../other"
editing other other
# The first editor has the default socket, the second one of its own;
# the cli reaches the first and finds the tui that holds its terminal.
[[ "$(sockets)" == 2 ]] || fail "two editors have $(sockets) sockets"
[[ "$("$BIN" cli children /session/1/client)" == *" 1" ]] \
    || fail "the running editor has no client rd 1"
serves="$("$BIN" cli read /session/1/client/1/capability)"
[[ "$serves" == $'keys\ngrid\nopen\nprocess' ]] \
    || fail "the running editor's client serves $serves"
# A wrong command: one line on stderr, naming the word and the target,
# and exit 1.
said="$("$BIN" cli read /session 2>&1 >/dev/null)" && code=0 || code=$?
[[ "$said" == "reovim: read /session: a Container has no bytes; ls it" ]] \
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
level=("$BIN" cli write /sys/loglevel)
[[ "$("$BIN" cli cat /sys/loglevel)" == info ]] \
    || fail "the level at boot is $("$BIN" cli cat /sys/loglevel)"
said="$("${level[@]}" loud 2>&1 >/dev/null)" && code=0 || code=$?
[[ "$said" == "reovim: write /sys/loglevel: invalid" ]] \
    || fail "a word that is not a level says: $said"
[[ "$code" == 1 ]] || fail "a word that is not a level exits $code"
"${level[@]}" debug > /dev/null

# The insert below is one `text` call, and the trace holds it.
keys new i "fresh" Escape : q Enter
shown new "E37: No write since last change"
said="$("$BIN" cli dmesg -d -l debug)"
"${level[@]}" 6 > /dev/null
[[ "$("$BIN" cli cat /sys/loglevel)" == info ]] \
    || fail "the level written as 6 does not read back as info"
grep -qE '^\[ *[0-9]+\.[0-9]{6} < *[0-9]+\.[0-9]{6}>\] ' <<< "$said" \
    || fail "dmesg -d says no delta: $(head -1 <<< "$said")"
grep -q 'call rd=/module/text/.* api=text method=replace dispatch' <<< "$said" \
    || fail "the call pair is not in words: $(grep -m1 'call ' <<< "$said")"
grep -qE 'api=text method=replace reply took=[0-9.]+[^ ]*s heap=[-+][0-9]+$' \
    <<< "$said" \
    || fail "the reply says no round trip: $(grep -m1 reply <<< "$said")"
grep -q 'op=read caller=' <<< "$said" || fail "no op is traced at debug"
[[ "$said" != *"rd=/sys/log"* ]] || fail "the log traced its own reading"
# Back at `info`, an op that would have been traced adds nothing.
count="$("$BIN" cli dmesg -l debug | wc -l)"
"$BIN" cli ls /module > /dev/null
[[ "$("$BIN" cli dmesg -l debug | wc -l)" == "$count" ]] \
    || fail "the trace goes on after the level says info"

# --- /sys as sysfs: the parameters, the heap, the core --------------
# A file per parameter under /sys/limit, each in decimal and each
# settable; /sys/mem is what the host's allocator counts; /sys/core is
# everything the Kernel holds, saved in one read as a core is.
rights="$("$BIN" cli ls -l /sys)"
[[ "$(grep -c . <<< "$rights")" == 6 ]] || fail "/sys holds: $rights"
grep -qE '^- r-  .*  core$' <<< "$rights" || fail "/sys/core is not r"
grep -qE '^- r-  .*  mem$' <<< "$rights" || fail "/sys/mem is not r"
limits="$("$BIN" cli ls -l /sys/limit)"
[[ "$(grep -c '^- rw  ' <<< "$limits")" == 5 ]] \
    || fail "the five parameters are not rw: $limits"
[[ "$("$BIN" cli cat /sys/limit/log_ring)" == 4096 ]] \
    || fail "log_ring at boot is $("$BIN" cli cat /sys/limit/log_ring)"
# A ring told to keep eight keeps eight from that write on.
"$BIN" cli write /sys/limit/log_ring 8 > /dev/null
[[ "$("$BIN" cli dmesg | wc -l)" == 8 ]] \
    || fail "a ring of eight kept $("$BIN" cli dmesg | wc -l) lines"
"$BIN" cli dmesg -t -l notice \
    | grep -q 'limit name=log_ring was=4096 now=8 caller=' \
    || fail "the parameter write is not in the log it sized"
said="$("$BIN" cli write /sys/limit/reentry x 2>&1 >/dev/null)" \
    && code=0 || code=$?
[[ "$said" == "reovim: write /sys/limit/reentry: invalid" ]] \
    || fail "a parameter that is no number says: $said"
[[ "$code" == 1 ]] || fail "a refused parameter exits $code"
"$BIN" cli write /sys/limit/log_ring 4096 > /dev/null

[[ "$("$BIN" cli cat /sys/mem)" == live=* ]] \
    || fail "/sys/mem says $("$BIN" cli cat /sys/mem)"
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
[[ "$(ended sysed)" == 0 ]] || fail "the --sys editor did not quit"

[[ ! -e "$DIR/new/note" ]] || fail "a new file exists before its save"
keys new : w q Enter
keys other i "x" Escape : w Enter : q ! Enter
[[ "$(ended new)" == 0 && "$(ended other)" == 0 ]] || fail "quit failed"
[[ "$(cat "$DIR/new/note")" == fresh ]] || fail "new file not written"
[[ "$(cat "$DIR/other")" == x ]] || fail "second editor not written"
[[ "$(sockets)" == 0 ]] || fail "a socket was left behind"

# --- a new file left unwritten leaves nothing behind -----------------
start gone "$DIR" "$BIN $DIR/new/gone"
editing gone gone
shown gone '"new/gone" \[New\]'
keys gone : q Enter
[[ "$(ended gone)" == 0 ]] || fail ":q on a new file did not quit"
[[ ! -e "$DIR/new/gone" ]] || fail ":q on a new file left one behind"

# --- a new file below a directory that is not there ------------------
start deep "$DIR" "$BIN $DIR/new/a/b/x"
editing deep x
shown deep '"new/a/b/x" \[New DIRECTORY\]'
keys deep i "deep" Escape : w q Enter
[[ "$(ended deep)" == 0 ]] || fail "the deep new file did not quit"
[[ -d "$DIR/new/a/b" ]] || fail "the directories on the way were not made"
[[ "$(cat "$DIR/new/a/b/x")" == deep ]] || fail "the deep file not written"

# --- no file: an unnamed buffer, E32 on :w, nothing written ---------
start unnamed "$DIR" "$BIN"
editing unnamed '\[No Name\]'
keys unnamed i "hi" Escape : w Enter
shown unnamed "E32: No file name"
keys unnamed : q ! Enter
[[ "$(ended unnamed)" == 0 ]] || fail "the unnamed buffer did not quit"
[[ ! -e "$DIR/[No Name]" && ! -e "$DIR/No Name" ]] \
    || fail "the unnamed buffer wrote a file"

# --- :w NAME from an unnamed buffer names it -------------------------
start named "$DIR" "$BIN"
editing named '\[No Name\]'
keys named i "hi" Escape : w " " n o t e Enter
editing named note
keys named : q Enter
[[ "$(ended named)" == 0 ]] || fail ":w NAME then :q failed"
[[ "$(cat "$DIR/note")" == hi ]] || fail ":w NAME did not write"

# --- SIGTERM: the terminal cooked, the file untouched, exit 1 -------
start term "$DIR" "$BIN other; echo \$? > '$DIR/exit-term'; \
    stty -a > '$DIR/stty'; sleep 30"
editing term other
keys term i "zz" Escape
sleep 0.05
pkill -TERM -f "^$BIN other\$"
[[ "$(ended term)" == 1 ]] || fail "SIGTERM did not exit 1"
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
[[ "$(ended time)" == 0 ]] || fail "the timed editor did not quit"
millis="$(printf '%s %s\n' "$after" "$before" \
    | awk '{ printf "%d", ($1 - $2) * 1000 }')"
printf 'smoke.sh: start to first frame %s ms (tmux included)\n' "$millis"
if [[ "$BIN" == */release/* && "$millis" -ge 50 ]]; then
    fail "first frame took $millis ms"
fi
echo "smoke.sh: the editor role holds"
