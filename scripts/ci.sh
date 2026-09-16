#!/usr/bin/env bash
# The local and GitHub contributor matrix share this entrypoint.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly ROOT
readonly -a TOOLCHAINS=(stable nightly)
readonly -a COMPONENTS=(clippy rustfmt)
readonly HOSTED_TARGET=x86_64-unknown-linux-gnu
readonly -a TARGETS=("$HOSTED_TARGET" x86_64-unknown-none)
declare -A TOOLCHAIN_PINS=()

fail() {
    printf 'ci.sh: %s: %s\n' "$1" "$2" >&2
    exit "${3:-1}"
}

usage() {
    cat <<'EOF'
Usage: ./scripts/ci.sh [--toolchain NAME | --matrix | --help]

No argument runs every CI lane, even if another lane fails. --toolchain runs
one declared stable/nightly role. ci.json owns their exact toolchain pins.
--matrix validates that manifest and emits the GitHub matrix without checks.
Each lane installs its declared toolchain/components/targets when needed,
fetches locked dependencies into a fresh Cargo home, then executes
scripts/check.sh --full. This uses the network and your Rustup home; Rustup
itself is not updated. The manifest is never changed by running checks.
--help and --matrix are read-only/network-free; --matrix requires Perl.

Prerequisites: x86_64 Linux, Rustup, Git, Perl, ripgrep, and the Node/TypeScript
versions declared in apps/web/package.json. Browser tools must be installed
before this runner; it does not install them locally. The exact resolved
compiler versions are printed. Local and GitHub runs share the testing window,
prerequisites, fetch, and checks; they do not share an identical OS image.
To change the window, edit ci.json and pass the full matrix before committing.
Exit: 1 matrix failure, 2 usage, 3 prerequisite, 4 isolation, 5 fetch, 6 check.
EOF
}

emit_matrix() {
    local lane separator=""
    printf '{"include":['
    for lane in "${TOOLCHAINS[@]}"; do
        printf '%s{"toolchain":"%s","version":"%s"}' \
            "$separator" "$lane" "${TOOLCHAIN_PINS[$lane]}"
        separator=,
    done
    printf ']}\n'
}

validate_lane() {
    local lane

    for lane in "${TOOLCHAINS[@]}"; do
        [[ "$1" != "$lane" ]] || return 0
    done
    fail CI_USAGE "unknown toolchain: $1" 2
}

run_matrix() {
    local lane observed
    local -a failed=()

    for lane in "${TOOLCHAINS[@]}"; do
        # A new shell preserves errexit inside the lane despite this test.
        if CI_EXPECTED_WINDOW="$WINDOW" \
            "$ROOT/scripts/ci.sh" --toolchain "$lane"; then
            printf 'ci.sh: lane %s passed\n' "$lane"
        else
            failed+=("$lane")
        fi
    done
    observed="$("$ROOT/scripts/ci.sh" --matrix)" || \
        fail CI_MANIFEST 'cannot revalidate testing window' 2
    [[ "$observed" == "$WINDOW" ]] || \
        fail CI_WINDOW_CHANGED 'manifest window changed during the matrix' 2
    [[ ${#failed[@]} -eq 0 ]] || \
        fail CI_MATRIX_FAILED "failed lanes: ${failed[*]}"
}

mode=all
case "${1-}" in
    "") [[ $# -eq 0 ]] || fail CI_USAGE 'unexpected empty argument' 2 ;;
    --toolchain)
        [[ $# -eq 2 ]] || fail CI_USAGE '--toolchain requires one name' 2
        validate_lane "$2"
        lane="$2"
        mode=lane
        ;;
    --matrix | -h | --help)
        [[ $# -eq 1 ]] || fail CI_USAGE "$1 accepts no other arguments" 2
        if [[ "$1" != --matrix ]]; then
            usage
            exit 0
        fi
        mode=discover
        ;;
    *) fail CI_USAGE "unexpected argument: $1" 2 ;;
esac

normalize_environment() {
    local variable

    while IFS= read -r variable; do
        case "$variable" in
            CARGO | CARGO_* | RUSTC* | RUSTDOC* | RUSTFMT* | RUSTFLAGS \
                | RUSTUP_TOOLCHAIN | RUSTUP_DIST_SERVER | RUSTUP_UPDATE_ROOT \
                | GIT_* | BASH_ENV | ENV \
                | PERL5OPT | PERL5LIB | RIPGREP_CONFIG_PATH)
                unset "$variable"
                ;;
        esac
    done < <(compgen -e)
    export LC_ALL=C CARGO_TERM_COLOR=always
    export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
}

load_manifest() {
    local records role pin

    command -v perl >/dev/null || fail CI_TOOL_MISSING 'required: perl' 3
    [[ -f "$ROOT/ci.json" && ! -L "$ROOT/ci.json" ]] || \
        fail CI_MANIFEST 'ci.json must be a regular non-symlink file' 2
    records="$(perl -MJSON::PP - "$ROOT/ci.json" <<'PERL'
use strict;
use warnings;

sub invalid { die "ci.sh: CI_MANIFEST: $_[0]\n"; }
open my $file, '<', $ARGV[0] or invalid("cannot read ci.json: $!");
my $input = do { local $/; <$file> };
my $manifest = eval { JSON::PP->new->decode($input) };
invalid("invalid JSON: $@") if $@;
invalid('expected a JSON object') unless ref($manifest) eq 'HASH';
my %formats = (
    'rust-stable' => qr/\A[1-9][0-9]*\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\z/,
    'rust-nightly' =>
        qr/\Anightly-[0-9]{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12][0-9]|3[01])\z/,
);
for my $key (keys %$manifest) {
    invalid("unknown key: $key") unless exists $formats{$key};
}
for my $role (qw(stable nightly)) {
    my $key = "rust-$role";
    my $pin = $manifest->{$key};
    invalid("missing or invalid exact pin: $key")
        unless defined($pin) && !ref($pin) && $pin =~ $formats{$key};
    print "$role\t$pin\n";
}
PERL
)" || fail CI_MANIFEST 'cannot load declared testing window' 2
    while IFS=$'\t' read -r role pin; do
        TOOLCHAIN_PINS["$role"]="$pin"
    done <<<"$records"
}

check_ancestor_configuration() {
    local directory="${ROOT%/*}" config

    while [[ -n "$directory" ]]; do
        for config in config config.toml; do
            if [[ -e "$directory/.cargo/$config" \
                || -L "$directory/.cargo/$config" ]]; then
                fail CI_CARGO_CONFIG \
                    "ancestor configuration: $directory/.cargo/$config" 3
            fi
        done
        [[ "$directory" != / ]] || break
        directory="${directory%/*}"
        [[ -n "$directory" ]] || directory=/
    done
}

contains_line() {
    local line

    while IFS= read -r line; do
        [[ "$line" != "$2" ]] || return 0
    done <<<"$1"
    return 1
}

require_programs() {
    local program

    for program in rustup git perl rg mktemp uname; do
        command -v "$program" >/dev/null || \
            fail CI_TOOL_MISSING "required program: $program" 3
    done
    [[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || \
        fail CI_HOST_UNSUPPORTED 'requires x86_64 Linux' 3
}

sync_toolchain() {
    local IFS=,

    printf 'ci.sh: %s: ensuring declared toolchain %s\n' \
        "$lane" "${TOOLCHAIN_PINS[$lane]}"
    rustup toolchain install "$TOOLCHAIN" --profile minimal \
        --component "${COMPONENTS[*]}" --target "${TARGETS[*]}" \
        --no-self-update || \
        fail CI_TOOLCHAIN_SYNC "cannot install declared $lane toolchain" 3
}

check_prerequisites() {
    local version components targets item

    version="$(rustup run "$TOOLCHAIN" rustc -vV)" || \
        fail CI_TOOLCHAIN_MISSING "cannot execute $TOOLCHAIN" 3
    contains_line "$version" "host: $HOSTED_TARGET" || \
        fail CI_HOST_UNSUPPORTED "requires Rust host $HOSTED_TARGET" 3
    printf 'ci.sh: installed channel %s\n%s\n' "$lane" "$version"
    components="$(rustup component list --installed \
        --toolchain "$TOOLCHAIN")" || \
        fail CI_COMPONENT_QUERY "cannot inspect $lane components" 3
    for item in "${COMPONENTS[@]}"; do
        contains_line "$components" "$item-$HOSTED_TARGET" || \
            fail CI_COMPONENT_MISSING "$lane requires $item" 3
    done
    targets="$(rustup target list --installed --toolchain "$TOOLCHAIN")" || \
        fail CI_TARGET_QUERY "cannot inspect $lane targets" 3
    for item in "${TARGETS[@]}"; do
        contains_line "$targets" "$item" || \
            fail CI_TARGET_MISSING "$lane requires $item" 3
    done
}

normalize_environment
load_manifest
readonly WINDOW="$(emit_matrix)"
if [[ -n "${CI_EXPECTED_WINDOW-}" \
    && "$CI_EXPECTED_WINDOW" != "$WINDOW" ]]; then
    fail CI_WINDOW_CHANGED 'manifest window differs from the matrix start' 2
fi
if [[ "$mode" == discover ]]; then
    printf '%s\n' "$WINDOW"
    exit 0
fi
if [[ "$mode" == all ]]; then
    run_matrix
    exit 0
fi
readonly TOOLCHAIN="${TOOLCHAIN_PINS[$lane]}-$HOSTED_TARGET"
require_programs
check_ancestor_configuration
sync_toolchain
check_prerequisites
cd "$ROOT"

CI_CARGO_HOME="$(mktemp -d /tmp/reovim-ci.XXXXXXXX)" || \
    fail CI_ISOLATION 'cannot create temporary Cargo home' 4
readonly CI_CARGO_HOME
if [[ "$CI_CARGO_HOME" != /tmp/reovim-ci.* || ! -d "$CI_CARGO_HOME" \
    || -L "$CI_CARGO_HOME" || ! -O "$CI_CARGO_HOME" ]]; then
    fail CI_ISOLATION 'mktemp returned an unsafe Cargo home' 4
fi
cleanup() {
    if [[ "$CI_CARGO_HOME" == /tmp/reovim-ci.* && -d "$CI_CARGO_HOME" \
        && ! -L "$CI_CARGO_HOME" && -O "$CI_CARGO_HOME" ]]; then
        rm -rf -- "$CI_CARGO_HOME"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export CARGO_HOME="$CI_CARGO_HOME" RUSTUP_TOOLCHAIN="$TOOLCHAIN"

printf 'ci.sh: %s: fetching locked dependencies\n' "$lane"
rustup run "$TOOLCHAIN" cargo fetch \
    --manifest-path "$ROOT/Cargo.toml" --locked || \
    fail CI_FETCH_FAILED "$lane locked dependency fetch failed" 5
printf 'ci.sh: %s: uncached contributor checks\n' "$lane"
rustup run "$TOOLCHAIN" bash "$ROOT/scripts/check.sh" --full || \
    fail CI_CHECK_FAILED "$lane contributor checks failed" 6
