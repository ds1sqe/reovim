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
cleanup() {
    "${TMUX[@]}" kill-server 2>/dev/null || true
    rm -rf -- "$DIR"
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
keys rebase i "reworded " Escape Z Z
[[ "$(ended rebase)" == 0 ]] || fail "rebase failed"
[[ "$(git log -1 --format=%s)" == "reworded third" ]] \
    || fail "reword not used: $(git log -1 --format=%s)"

# --- a new file by absolute path, absent until saved; two at once ---
mkdir "$DIR/new"
start new "$DIR" "$BIN $DIR/new/note"
start other "$DIR/new" "$BIN ../other"
editing new note
editing other other
keys new i "fresh" Escape : q Enter
shown new "E37: No write since last change"
[[ ! -e "$DIR/new/note" ]] || fail "a new file exists before its save"
keys new : w q Enter
keys other i "x" Escape : w Enter : q ! Enter
[[ "$(ended new)" == 0 && "$(ended other)" == 0 ]] || fail "quit failed"
[[ "$(cat "$DIR/new/note")" == fresh ]] || fail "new file not written"
[[ "$(cat "$DIR/other")" == x ]] || fail "second editor not written"
[[ ! -e "$XDG_RUNTIME_DIR" ]] || fail "the runtime directory was touched"

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

# --- start to first frame -------------------------------------------
before="$EPOCHREALTIME"
start time "$DIR" "$BIN other"
editing time other
after="$EPOCHREALTIME"
keys time C-q
millis="$(printf '%s %s\n' "$after" "$before" \
    | awk '{ printf "%d", ($1 - $2) * 1000 }')"
printf 'smoke.sh: start to first frame %s ms (tmux included)\n' "$millis"
if [[ "$BIN" == */release/* && "$millis" -ge 50 ]]; then
    fail "first frame took $millis ms"
fi
echo "smoke.sh: the editor role holds"
