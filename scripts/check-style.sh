#!/usr/bin/env bash
# Repository-wide checks for governed human-authored source.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly LIMIT=80
readonly MAX_ENTRIES=16384
readonly MAX_FILES=8192
readonly MAX_FILE_BYTES=4194304
readonly MAX_TOTAL_BYTES=67108864
readonly MARKER_KEY="reovim-"'style:'
readonly EXACT_REASON="exact-token"
readonly SEARCH_REASON="searchable-string"

usage() {
    cat <<'EOF'
Usage: ./scripts/check-style.sh

Checks line width, local line-width exemptions, and trailing whitespace in
active human-authored Rust, shell, TOML, and YAML source. It also runs its
closed scanner, Rustfmt, and Clippy mutation fixtures.
EOF
}

case "${1:-}" in
    "") ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        printf 'check-style: unexpected argument: %s\n' "$1" >&2
        usage >&2
        exit 2
        ;;
esac

source_comment() {
    case "$1" in
        *.rs) printf '%s\n' '//' ;;
        *.sh | *.bash | *.zsh | *.toml | *.yml | *.yaml)
            printf '%s\n' '#'
            ;;
        *) return 1 ;;
    esac
}

is_governed_path() {
    local path="$1"

    case "$path" in
        .git/* | target/* | tmp/*) return 1 ;;
        Documentation/heritage/*) return 1 ;;
        changelog/*) return 1 ;;
        Cargo.lock) return 1 ;;
    esac
    source_comment "$path" >/dev/null
}

style_issue() {
    local display="$1"
    local line="$2"
    local code="$3"
    local message="$4"

    printf '%s:%s: %s: %s\n' \
        "$display" "$line" "$code" "$message" >&2
}

measured_size=0
measure_file() {
    local file="$1"
    local display="$2"
    local size

    if ! size="$(stat -c '%s' -- "$file" 2>/dev/null)"; then
        style_issue "$display" 0 STYLE_INPUT_STAT \
            'governed file size is unavailable'
        return 1
    fi
    case "$size" in
        '' | *[!0-9]*)
            style_issue "$display" 0 STYLE_INPUT_STAT \
                'governed file size is not an unsigned decimal'
            return 1
            ;;
    esac
    measured_size="$size"
}

decimal_exceeds() {
    local value="$1"
    local limit="$2"

    if [[ ${#value} -ne ${#limit} ]]; then
        (( ${#value} > ${#limit} ))
        return
    fi
    [[ "$value" > "$limit" ]]
}

admitted_total=0
check_entry_bound() {
    local display="$1"
    local next_entries="$2"

    if (( next_entries > MAX_ENTRIES )); then
        style_issue "$display" 0 STYLE_ENTRY_COUNT_LIMIT \
            "repository traversal exceeds $MAX_ENTRIES entries"
        return 1
    fi
}

check_inventory_bounds() {
    local display="$1"
    local next_files="$2"
    local file_bytes="$3"
    local current_total="$4"
    local next_total

    if (( next_files > MAX_FILES )); then
        style_issue "$display" 0 STYLE_FILE_COUNT_LIMIT \
            "governed file count exceeds $MAX_FILES"
        return 1
    fi
    if decimal_exceeds "$file_bytes" "$MAX_FILE_BYTES"; then
        style_issue "$display" 0 STYLE_FILE_BYTES_LIMIT \
            "governed file exceeds $MAX_FILE_BYTES bytes"
        return 1
    fi
    next_total=$((current_total + file_bytes))
    if (( next_total > MAX_TOTAL_BYTES )); then
        style_issue "$display" 0 STYLE_TOTAL_BYTES_LIMIT \
            "governed files exceed $MAX_TOTAL_BYTES bytes"
        return 1
    fi
    admitted_total="$next_total"
}

check_input_type() {
    local file="$1"
    local display="$2"

    if [[ -L "$file" || ! -f "$file" ]]; then
        style_issue "$display" 0 STYLE_INPUT_TYPE \
            'governed path must be one regular non-symlink file'
        return 1
    fi
}

scan_file() {
    local file="$1"
    local display="$2"
    local expected_size="$3"
    local remaining_bytes="$4"
    local comment
    comment="$(source_comment "$display")"

    perl - "$file" "$display" "$comment" "$LIMIT" \
        "$MARKER_KEY" "$EXACT_REASON" "$SEARCH_REASON" \
        "$MAX_FILE_BYTES" "$remaining_bytes" "$expected_size" <<'PERL'
use strict;
use warnings;

my (
    $file,
    $display,
    $comment,
    $limit,
    $key,
    $exact_reason,
    $search_reason,
    $max_file_bytes,
    $remaining_bytes,
    $expected_size,
) = @ARGV;
my $failed = 0;

sub issue {
    my ($line, $code, $message) = @_;
    print STDERR "$display:$line: $code: $message\n";
    $failed = 1;
}

open my $handle, '<:raw', $file or do {
    issue(0, 'STYLE_INPUT_READ', 'governed file cannot be opened');
    exit 1;
};
my $raw = '';
while (1) {
    my $chunk = '';
    my $count = read $handle, $chunk, 65_536;
    if (!defined $count) {
        issue(0, 'STYLE_INPUT_READ', 'governed file read failed');
        exit 1;
    }
    last if $count == 0;
    $raw .= $chunk;
    if (length($raw) > $max_file_bytes) {
        issue(0, 'STYLE_FILE_BYTES_LIMIT',
            "governed file exceeds $max_file_bytes bytes");
        exit 1;
    }
    if (length($raw) > $remaining_bytes) {
        issue(0, 'STYLE_TOTAL_BYTES_LIMIT',
            'governed files exceed the aggregate byte limit');
        exit 1;
    }
}
close $handle or do {
    issue(0, 'STYLE_INPUT_READ', 'governed file close failed');
    exit 1;
};
if (length($raw) != $expected_size) {
    issue(0, 'STYLE_INPUT_CHANGED',
        'governed file size changed while it was scanned');
    exit 1;
}
if (!utf8::decode($raw)) {
    issue(0, 'STYLE_INVALID_UTF8',
        'governed source is not well-formed UTF-8');
    exit 1;
}

my @lines = split /\n/, $raw, -1;
pop @lines if @lines && $lines[-1] eq '';
my $exact_marker = "$comment $key allow-long-line $exact_reason";
my $search_marker = "$comment $key allow-long-line $search_reason";
my $pending_line = 0;
my $line_number = 0;

for my $text (@lines) {
    ++$line_number;
    my $trimmed = $text;
    $trimmed =~ s/\A[ \t]*//;
    my $width = 0;
    for my $character (split //, $text) {
        my $value = ord $character;
        if ($character eq "\t") {
            $width += 8 - ($width % 8);
        } elsif ($value >= 0x20 && $value <= 0x7e) {
            ++$width;
        } else {
            $width += 2;
        }
    }

    if ($text =~ /\s\z/) {
        issue($line_number, 'STYLE_TRAILING_WHITESPACE',
            'trailing whitespace is forbidden');
    }

    if ($trimmed eq $exact_marker || $trimmed eq $search_marker) {
        if ($display =~ /\.rs\z/) {
            issue($line_number, 'STYLE_EXEMPTION_UNSUPPORTED',
                'Rust line-width markers are not admitted');
            next;
        }
        if ($width > $limit) {
            issue($line_number, 'STYLE_LINE_WIDTH',
                'marker line exceeds the 80-column maximum');
        }
        if ($pending_line != 0) {
            issue($pending_line, 'STYLE_EXEMPTION_DUPLICATE',
                'a second marker cannot replace an unused marker');
        }
        $pending_line = $line_number;
        next;
    }

    if (index($text, $key) != -1) {
        if ($pending_line != 0) {
            issue($pending_line, 'STYLE_EXEMPTION_NONADJACENT',
                'the next line is not the marked source line');
            $pending_line = 0;
        }
        issue($line_number, 'STYLE_EXEMPTION_UNKNOWN',
            'marker syntax or reason is not admitted');
        if ($width > $limit) {
            issue($line_number, 'STYLE_LINE_WIDTH',
                'line exceeds the 80-column maximum');
        }
        next;
    }

    if ($pending_line != 0) {
        if ($width > $limit) {
            $pending_line = 0;
            next;
        }
        if ($trimmed eq '') {
            issue($pending_line, 'STYLE_EXEMPTION_NONADJACENT',
                'a blank line separates marker and source');
        } else {
            issue($pending_line, 'STYLE_EXEMPTION_STALE',
                'the marked source line does not exceed 80 columns');
        }
        $pending_line = 0;
    }

    if ($width > $limit) {
        issue($line_number, 'STYLE_LINE_WIDTH',
            'line exceeds the 80-column maximum');
    }
}

if ($pending_line != 0) {
    issue($pending_line, 'STYLE_EXEMPTION_STALE',
        'marker has no following source line');
}
exit($failed ? 1 : 0);
PERL
}

fixture_root=""
fixture_root_proven=0
cleanup_fixtures() {
    if (( fixture_root_proven == 0 )); then
        return 0
    fi
    if [[ -z "$fixture_root" || "$fixture_root" != /* || \
        ! -d "$fixture_root" || -L "$fixture_root" || \
        ! -O "$fixture_root" ]]; then
        style_issue check-style 0 STYLE_FIXTURE_ROOT \
            'proven fixture root became ineligible'
        return 1
    fi
    if ! rm -rf -- "$fixture_root"; then
        style_issue check-style 0 STYLE_FIXTURE_CLEANUP \
            'proven fixture root could not be removed'
        return 1
    fi
    fixture_root=""
    fixture_root_proven=0
}
trap 'cleanup_fixtures || true' EXIT

create_fixture_root() {
    local created

    if ! created="$(mktemp -d)"; then
        style_issue check-style 0 STYLE_FIXTURE_ROOT \
            'mktemp could not create the fixture root'
        return 1
    fi
    if [[ -z "$created" || "$created" != /* || \
        ! -d "$created" || -L "$created" || ! -O "$created" ]]; then
        style_issue check-style 0 STYLE_FIXTURE_ROOT \
            'mktemp did not return an owned absolute directory'
        return 1
    fi
    fixture_root="$created"
    fixture_root_proven=1
}

fixture_path=""
write_fixture() {
    local name="$1"
    local content="$2"
    local destination

    if (( fixture_root_proven == 0 )); then
        style_issue check-style 0 STYLE_FIXTURE_ROOT \
            'fixture write was attempted before root proof'
        return 1
    fi
    case "$name" in
        '' | *[!A-Za-z0-9._-]*)
            style_issue check-style 0 STYLE_FIXTURE_PATH \
                'fixture name is outside the closed local form'
            return 1
            ;;
    esac
    destination="$fixture_root/$name"
    if ! printf '%s' "$content" >"$destination"; then
        style_issue "$name" 0 STYLE_FIXTURE_WRITE \
            'fixture content could not be written'
        return 1
    fi
    fixture_path="$destination"
}

mutation_failures=0
expect_scan() {
    local expectation="$1"
    local expected_code="$2"
    local name="$3"
    local content="$4"
    local path
    local output
    local size

    if ! write_fixture "$name" "$content"; then
        mutation_failures=1
        return
    fi
    path="$fixture_path"
    if ! measure_file "$path" "$name"; then
        mutation_failures=1
        return
    fi
    size="$measured_size"

    if output="$(
        scan_file "$path" "$name" "$size" "$MAX_TOTAL_BYTES" 2>&1
    )"; then
        if [[ "$expectation" != pass ]]; then
            printf 'fixture %s unexpectedly passed\n' "$name" >&2
            mutation_failures=1
        fi
    else
        if [[ "$expectation" != fail ]]; then
            printf 'fixture %s unexpectedly failed: %s\n' \
                "$name" "$output" >&2
            mutation_failures=1
        elif [[ "$output" != *"$expected_code"* ]]; then
            printf 'fixture %s missed %s: %s\n' \
                "$name" "$expected_code" "$output" >&2
            mutation_failures=1
        fi
    fi
}

expect_bound() {
    local expectation="$1"
    local expected_code="$2"
    local name="$3"
    local next_files="$4"
    local file_bytes="$5"
    local current_total="$6"
    local output

    if output="$(
        check_inventory_bounds \
            "$name" "$next_files" "$file_bytes" "$current_total" 2>&1
    )"; then
        if [[ "$expectation" != pass ]]; then
            printf 'bound fixture %s unexpectedly passed\n' "$name" >&2
            mutation_failures=1
        fi
    else
        if [[ "$expectation" != fail ]]; then
            printf 'bound fixture %s unexpectedly failed: %s\n' \
                "$name" "$output" >&2
            mutation_failures=1
        elif [[ "$output" != *"$expected_code"* ]]; then
            printf 'bound fixture %s missed %s: %s\n' \
                "$name" "$expected_code" "$output" >&2
            mutation_failures=1
        fi
    fi
}

expect_entry_bound() {
    local expectation="$1"
    local expected_code="$2"
    local name="$3"
    local next_entries="$4"
    local output

    if output="$(
        check_entry_bound "$name" "$next_entries" 2>&1
    )"; then
        if [[ "$expectation" != pass ]]; then
            printf 'entry fixture %s unexpectedly passed\n' "$name" >&2
            mutation_failures=1
        fi
    else
        if [[ "$expectation" != fail ]]; then
            printf 'entry fixture %s unexpectedly failed: %s\n' \
                "$name" "$output" >&2
            mutation_failures=1
        elif [[ "$output" != *"$expected_code"* ]]; then
            printf 'entry fixture %s missed %s: %s\n' \
                "$name" "$expected_code" "$output" >&2
            mutation_failures=1
        fi
    fi
}

expect_path_scope() {
    local expectation="$1"
    local path="$2"
    local actual=excluded
    if is_governed_path "$path"; then
        actual=governed
    fi
    if [[ "$actual" != "$expectation" ]]; then
        printf 'scope fixture %s was %s, expected %s\n' \
            "$path" "$actual" "$expectation" >&2
        mutation_failures=1
    fi
}

expect_input_type_failure() {
    local name="$1"
    local path="$2"
    local output

    if output="$(check_input_type "$path" "$name" 2>&1)"; then
        printf 'type fixture %s unexpectedly passed\n' "$name" >&2
        mutation_failures=1
    elif [[ "$output" != *STYLE_INPUT_TYPE* ]]; then
        printf 'type fixture %s missed STYLE_INPUT_TYPE: %s\n' \
            "$name" "$output" >&2
        mutation_failures=1
    fi
}

run_rust_tool_fixtures() {
    local output
    local nesting_source formatting_source nesting_output

    if ! command -v clippy-driver >/dev/null; then
        style_issue check-style 0 STYLE_TOOL_UNAVAILABLE \
            'clippy-driver is required for the nesting mutant'
        return 1
    fi
    if ! command -v rustfmt >/dev/null; then
        style_issue check-style 0 STYLE_TOOL_UNAVAILABLE \
            'rustfmt is required for the formatting mutant'
        return 1
    fi

    nesting_source=$'mod first {\n    struct Probe;\n\n'
    nesting_source+=$'    impl Probe {\n        fn run(&self) {\n'
    nesting_source+=$'            if true {\n'
    nesting_source+=$'                let _ = self;\n'
    nesting_source+=$'            }\n        }\n    }\n}\n'
    if ! write_fixture nesting.rs "$nesting_source"; then
        return 1
    fi
    nesting_output="$fixture_root/nesting.rmeta"
    if output="$(
        CLIPPY_CONF_DIR="$ROOT" clippy-driver "$fixture_path" \
            --crate-name style_nesting_fixture \
            --edition=2024 --crate-type=lib --emit=metadata \
            -Adead_code -Fclippy::excessive_nesting \
            -o "$nesting_output" 2>&1
    )"; then
        style_issue nesting.rs 0 STYLE_NESTING_FIXTURE \
            'four-level nesting mutant unexpectedly passed'
        return 1
    fi
    if [[ "$output" != *'this block is too nested'* ]]; then
        style_issue nesting.rs 0 STYLE_NESTING_FIXTURE \
            'nesting mutant failed without the expected Clippy lint'
        return 1
    fi

    formatting_source=$'fn format_probe( ) {let _value=1;}\n'
    if ! write_fixture formatting.rs "$formatting_source"; then
        return 1
    fi
    if output="$(
        rustfmt --check --config-path "$ROOT/rustfmt.toml" \
            "$fixture_path" 2>&1
    )"; then
        style_issue formatting.rs 0 STYLE_RUSTFMT_FIXTURE \
            'unformatted Rust mutant unexpectedly passed'
        return 1
    fi
    if [[ "$output" != *'Diff in '* ]]; then
        style_issue formatting.rs 0 STYLE_RUSTFMT_FIXTURE \
            'Rustfmt mutant failed without the expected difference'
        return 1
    fi
}

run_mutation_fixtures() {
    local long exact_marker search_marker unknown_marker overlong_marker
    local tabs unicode wide_character item

    if ! create_fixture_root; then
        return 1
    fi
    printf -v long '%081d' 0
    printf -v tabs '\t\t\t\t\t\t\t\t\t'
    unicode=""
    wide_character=""
    for ((item = 0; item < 40; ++item)); do
        unicode+='é'
        wide_character+='界'
    done
    exact_marker="# $MARKER_KEY allow-long-line $EXACT_REASON"
    search_marker="// $MARKER_KEY allow-long-line $SEARCH_REASON"
    unknown_marker="# $MARKER_KEY allow-long-line unknown"
    printf -v overlong_marker '%*s%s' 81 '' "$exact_marker"

    expect_scan pass '' valid.sh $'value=short\n'
    expect_scan fail STYLE_LINE_WIDTH long.sh "$long"$'\n'
    expect_scan pass '' exact.sh "$exact_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_EXEMPTION_UNSUPPORTED search.rs \
        "$search_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_EXEMPTION_UNKNOWN unknown.sh \
        "$unknown_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_EXEMPTION_STALE stale.sh \
        "$exact_marker"$'\nshort\n'
    expect_scan fail STYLE_EXEMPTION_DUPLICATE duplicate.sh \
        "$exact_marker"$'\n'"$exact_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_EXEMPTION_NONADJACENT nonadjacent.sh \
        "$exact_marker"$'\n\n'"$long"$'\n'
    expect_scan fail STYLE_LINE_WIDTH one-use.sh \
        "$exact_marker"$'\n'"$long"$'\n'"$long"$'\n'
    expect_scan fail STYLE_EXEMPTION_UNKNOWN wrong-comment.rs \
        "$exact_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_LINE_WIDTH overlong-marker.sh \
        "$overlong_marker"$'\n'"$long"$'\n'
    expect_scan fail STYLE_TRAILING_WHITESPACE trailing.toml \
        $'value = true \n'
    expect_scan pass '' tabs.yml "$tabs"$'12345678\n'
    expect_scan fail STYLE_LINE_WIDTH wide-tabs.yml \
        "$tabs"$'\tx\n'
    expect_scan pass '' unicode.rs "$unicode"$'\n'
    expect_scan fail STYLE_LINE_WIDTH wide-unicode.rs \
        "$unicode"$'é\n'
    expect_scan pass '' wide-character.rs "$wide_character"$'\n'
    expect_scan fail STYLE_LINE_WIDTH wider-character.rs \
        "$wide_character"$'界\n'
    expect_scan fail STYLE_INVALID_UTF8 invalid-utf8.rs $'\xff\n'
    expect_entry_bound pass '' exact-entries "$MAX_ENTRIES"
    expect_entry_bound fail STYLE_ENTRY_COUNT_LIMIT entry-count \
        "$((MAX_ENTRIES + 1))"
    expect_bound pass '' exact-bounds \
        "$MAX_FILES" "$MAX_FILE_BYTES" \
        "$((MAX_TOTAL_BYTES - MAX_FILE_BYTES))"
    expect_bound fail STYLE_FILE_COUNT_LIMIT file-count \
        "$((MAX_FILES + 1))" 0 0
    expect_bound fail STYLE_FILE_BYTES_LIMIT file-bytes \
        1 "$((MAX_FILE_BYTES + 1))" 0
    expect_bound fail STYLE_TOTAL_BYTES_LIMIT total-bytes \
        1 1 "$MAX_TOTAL_BYTES"
    expect_path_scope governed source/lib.rs
    expect_path_scope governed scripts/release.sh
    expect_path_scope excluded scripts/release
    expect_path_scope excluded .git/hooks/check.sh
    expect_path_scope excluded target/generated.rs
    expect_path_scope excluded tmp/generated.rs
    expect_path_scope excluded changelog/generated.yml
    expect_path_scope governed generated/bindings.rs
    expect_path_scope governed tools/conformance/src/generated/bindings.rs
    expect_path_scope excluded Documentation/heritage/example.sh
    expect_path_scope excluded Cargo.lock
    if ! mkdir -- "$fixture_root/nonregular.rs"; then
        style_issue nonregular.rs 0 STYLE_FIXTURE_WRITE \
            'nonregular fixture could not be created'
        mutation_failures=1
    else
        expect_input_type_failure \
            nonregular.rs "$fixture_root/nonregular.rs"
    fi
    if ! ln -s -- valid.sh "$fixture_root/symlink.rs"; then
        style_issue symlink.rs 0 STYLE_FIXTURE_WRITE \
            'symlink fixture could not be created'
        mutation_failures=1
    else
        expect_input_type_failure symlink.rs "$fixture_root/symlink.rs"
    fi

    if ! run_rust_tool_fixtures; then
        mutation_failures=1
    fi
    if ! cleanup_fixtures; then
        mutation_failures=1
    fi
    ((mutation_failures == 0))
}

run_mutation_fixtures || {
    echo 'check-style: embedded mutation fixture failed' >&2
    exit 1
}

failures=0
entries=0
files=0
total_bytes=0
inventory_pid=""
if ! exec 7< <(
    find "$ROOT" \
        \( -path "$ROOT/.git" -o -path "$ROOT/target" \
        -o -path "$ROOT/tmp" \
        -o -path "$ROOT/Documentation/heritage" \
        -o -path "$ROOT/changelog" \) -prune \
        -o -print0
); then
    style_issue check-style 0 STYLE_INVENTORY_READ \
        'repository traversal could not start'
    exit 1
fi
inventory_pid="$!"
while IFS= read -r -d '' file <&7; do
    relative="${file#"$ROOT/"}"
    next_entries=$((entries + 1))
    if ! check_entry_bound "$relative" "$next_entries"; then
        exit 1
    fi
    entries="$next_entries"
    is_governed_path "$relative" || continue
    if ! check_input_type "$file" "$relative"; then
        exit 1
    fi
    if ! measure_file "$file" "$relative"; then
        exit 1
    fi
    size="$measured_size"
    next_files=$((files + 1))
    if ! check_inventory_bounds \
        "$relative" "$next_files" "$size" "$total_bytes"; then
        exit 1
    fi
    remaining_bytes=$((MAX_TOTAL_BYTES - total_bytes))
    if ! scan_file \
        "$file" "$relative" "$size" "$remaining_bytes"; then
        failures=1
    fi
    files="$next_files"
    total_bytes="$admitted_total"
done
exec 7<&-
if ! wait "$inventory_pid"; then
    style_issue check-style 0 STYLE_INVENTORY_READ \
        'repository traversal failed'
    exit 1
fi

((failures == 0)) || {
    echo 'check-style: governed files do not conform' >&2
    exit 1
}

printf 'check-style: all checks passed (%d entries, %d files, %d bytes)\n' \
    "$entries" "$files" "$total_bytes"
