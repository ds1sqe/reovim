#!/usr/bin/env bash
# Component-aware input selection for scripts/check.sh.

set -euo pipefail

export LC_ALL=C
unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CEILING_DIRECTORIES \
    GIT_COMMON_DIR GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS \
    GIT_CONFIG_SYSTEM GIT_DIR GIT_DISCOVERY_ACROSS_FILESYSTEM \
    GIT_EXEC_PATH GIT_GRAFT_FILE GIT_INDEX_FILE GIT_NAMESPACE \
    GIT_OBJECT_DIRECTORY GIT_PREFIX GIT_REPLACE_REF_BASE \
    GIT_SHALLOW_FILE GIT_WORK_TREE
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
unset RIPGREP_CONFIG_PATH

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly ROOT

readonly CHECK_CACHE_MAGIC='reovim-basic-check-cache/v1'
readonly CHECK_CACHE_ROOT="$ROOT/target/basic-check-cache"
readonly CHECK_CACHE_MAX_DIRECTORIES=4096
readonly CHECK_CACHE_MAX_CONTEXT_BYTES=16777216
readonly CHECK_CACHE_MAX_GRAPH_BYTES=16777216
readonly CHECK_CACHE_MAX_INPUT_BYTES=67108864
readonly CHECK_CACHE_MAX_INPUT_TOTAL_BYTES=536870912
readonly CHECK_CACHE_MAX_INVENTORY_BYTES=16777216
readonly CHECK_CACHE_MAX_METADATA_BYTES=16777216
readonly CHECK_CACHE_MAX_PATH_BYTES=4096
readonly CHECK_CACHE_MAX_STATE_BYTES=67108864
readonly CHECK_CACHE_MAX_STATE_FILES=65536
readonly CHECK_CACHE_MAX_TOOL_BYTES=268435456
readonly CHECK_CACHE_MAX_VERSION_BYTES=1048576

check_cache_issue() {
    local code="$1"
    local message="$2"

    printf 'check-cache: %s: %s\n' "$code" "$message" >&2
}

check_cache_normal_path() {
    local path="$1"
    local dot=$'\x2e'
    local slash=$'\x2f'
    local parent="$dot$dot"

    [[ -n "$path" && ${#path} -le CHECK_CACHE_MAX_PATH_BYTES \
        && "$path" != "$slash"* && "$path" != "$dot" \
        && "$path" != "$dot$slash"* \
        && "$path" != *"$slash$dot" \
        && "$path" != *"$slash$dot$slash"* \
        && "$path" != "$parent" && "$path" != "$parent$slash"* \
        && "$path" != *"$slash$parent" \
        && "$path" != *"$slash$parent$slash"* \
        && "$path" != *"$slash$slash"* \
        && "$path" != *$'\t'* && "$path" != *$'\r'* \
        && "$path" != *$'\n'* ]]
}

check_cache_bound_nul_stream() {
    local maximum_entries="$1"
    local maximum_bytes="$2"
    local maximum_record_bytes="$3"

    perl -0 -e '
use strict;
use warnings;
use bytes;

my ($maximum_entries, $maximum_bytes, $maximum_record_bytes) = @ARGV;
my ($entries, $bytes) = (0, 0);
while (defined(my $record = <STDIN>)) {
    die "unterminated NUL record" if substr($record, -1) ne "\0";
    chop $record;
    my $length = length($record);
    die "empty NUL record" if $length == 0;
    die "NUL record exceeds bound" if $length > $maximum_record_bytes;
    $entries++;
    $bytes += $length + 1;
    die "NUL entry count exceeds bound" if $entries > $maximum_entries;
    die "NUL byte count exceeds bound" if $bytes > $maximum_bytes;
    print $record, "\0" or die "bounded NUL write failed: $!";
}
close STDOUT or die "bounded NUL close failed: $!";
' "$maximum_entries" "$maximum_bytes" "$maximum_record_bytes"
}

check_cache_bound_byte_stream() {
    local maximum_bytes="$1"

    perl -e '
use strict;
use warnings;
use bytes;

my ($maximum) = @ARGV;
my $total = 0;
while (1) {
    my $read = read STDIN, my $chunk, 65_536;
    die "bounded byte read failed: $!" if !defined $read;
    last if $read == 0;
    $total += $read;
    die "byte stream exceeds bound" if $total > $maximum;
    print $chunk or die "bounded byte write failed: $!";
}
close STDOUT or die "bounded byte close failed: $!";
' "$maximum_bytes"
}

check_cache_file_size_within() {
    local path="$1"
    local maximum="$2"
    local size

    size="$(stat -c '%s' -- "$path")" || return 1
    case "$size" in
        '' | *[!0-9]*) return 1 ;;
    esac
    ((size <= maximum))
}

check_cache_record_tool() {
    local output="$1"
    local program="$2"
    local resolved implementation identity rustup_path

    resolved="$(command -v "$program")" || return 1
    implementation="$resolved"
    rustup_path="$(command -v rustup 2>/dev/null || true)"
    case "$program" in
        cargo | cargo-clippy | cargo-fmt | rustc | rustdoc | rustfmt \
            | clippy-driver)
            if [[ -n "$rustup_path" \
                && "$(readlink -f -- "$resolved")" \
                    == "$(readlink -f -- "$rustup_path")" ]]; then
                implementation="$(rustup which "$program")" || return 1
            fi
            ;;
    esac
    [[ -f "$implementation" && -r "$implementation" ]] || return 1
    check_cache_file_size_within \
        "$implementation" "$CHECK_CACHE_MAX_TOOL_BYTES" || return 1
    identity="$(git hash-object --no-filters -- "$implementation")" || {
        return 1
    }
    printf 'tool\0%s\0%s\0%s\0%s\0' \
        "$program" "$resolved" "$implementation" "$identity" >>"$output"
    {
        case "$program" in
            bash) bash --version ;;
            git) git --version ;;
            cargo) cargo -Vv ;;
            cargo-clippy) cargo-clippy --version ;;
            cargo-fmt) cargo-fmt --version ;;
            rustc) rustc -Vv ;;
            rustdoc) rustdoc --version ;;
            rustfmt) rustfmt --version ;;
            clippy-driver) clippy-driver --version ;;
            rustup) rustup --version 2>&1 ;;
            perl)
                perl -MJSON::PP -e 'print "$JSON::PP::VERSION\n"'
                ;;
            *) printf 'content-addressed implementation\n' ;;
        esac
    } | check_cache_bound_byte_stream "$CHECK_CACHE_MAX_VERSION_BYTES" \
        >>"$output" || return 1
    printf '\0' >>"$output"
}

check_cache_require_fixed_tool() {
    local program="$1"
    local fixed="$2"
    local selected selected_resolved fixed_resolved

    selected="$(command -v "$program")" || return 1
    selected_resolved="$(readlink -f -- "$selected")" || return 1
    fixed_resolved="$(readlink -f -- "$fixed")" || return 1
    [[ "$selected_resolved" == "$fixed_resolved" ]]
}

check_cache_environment_affects_build() {
    case "$1" in
        CARGO | CARGO_* | RUST* | REOVIM_* | GIT_* | PERL* \
            | BASH_ENV | BASHOPTS | SHELLOPTS | ENV | CDPATH \
            | GLOBIGNORE | HOME | PATH | TMPDIR | TMP | TEMP | LANG \
            | LC_* | TERM | TZ | SOURCE_DATE_EPOCH | NIX_* \
            | RIPGREP_CONFIG_PATH \
            | CC | CC_* | CFLAGS | CFLAGS_* | CPPFLAGS | CPPFLAGS_* \
            | CXX | CXX_* | CXXFLAGS | CXXFLAGS_* | AR | AR_* \
            | RANLIB | RANLIB_* | LD | LD_* | LDFLAGS | LDFLAGS_* \
            | STRIP | STRIP_* | OBJCOPY | OBJCOPY_* | PKG_CONFIG \
            | PKG_CONFIG_* | BINDGEN_EXTRA_CLANG_ARGS \
            | BINDGEN_EXTRA_CLANG_ARGS_* | LIBCLANG_PATH | PROTOC)
            return 0
            ;;
    esac
    return 1
}

check_cache_record_environment() {
    local output="$1"
    local names="$output.environment-names"
    local variable value identity

    compgen -e \
        | check_cache_bound_byte_stream "$CHECK_CACHE_MAX_CONTEXT_BYTES" \
        | LC_ALL=C sort -u >"$names" || return 1
    while IFS= read -r variable; do
        check_cache_environment_affects_build "$variable" || continue
        value="${!variable-}"
        identity="$(printf '%s' "$value" | git hash-object --stdin)" || {
            return 1
        }
        printf 'environment\0%s\0%s\0' \
            "$variable" "$identity" >>"$output"
    done <"$names"
}

check_cache_reject_injected_environment() {
    local variable

    for variable in \
        BASH_ENV ENV PERL5OPT PERL5LIB GIT_DIR GIT_WORK_TREE \
        GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
        GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_EXEC_PATH \
        GIT_CONFIG_SYSTEM GIT_CONFIG_COUNT \
        RIPGREP_CONFIG_PATH; do
        [[ -z "${!variable-}" ]] || return 1
    done
    [[ "$GIT_CONFIG_GLOBAL" == /dev/null \
        && "$GIT_CONFIG_NOSYSTEM" == 1 ]]
}

check_cache_reject_unbound_flags() {
    local names="$1"
    local variable

    compgen -e \
        | check_cache_bound_byte_stream "$CHECK_CACHE_MAX_CONTEXT_BYTES" \
        | LC_ALL=C sort -u >"$names" || return 1
    while IFS= read -r variable; do
        case "$variable" in
            RUSTFLAGS | RUSTDOCFLAGS | CARGO_ENCODED_RUSTFLAGS \
                | CARGO_BUILD_RUSTFLAGS | CARGO_BUILD_RUSTDOCFLAGS \
                | CARGO_TARGET_*_RUSTFLAGS \
                | CARGO_TARGET_*_RUSTDOCFLAGS \
                | CFLAGS | CFLAGS_* | CPPFLAGS | CPPFLAGS_* \
                | CXXFLAGS | CXXFLAGS_* | LDFLAGS | LDFLAGS_* \
                | BINDGEN_EXTRA_CLANG_ARGS \
                | BINDGEN_EXTRA_CLANG_ARGS_* | LIBCLANG_PATH \
                | PKG_CONFIG_PATH | PKG_CONFIG_LIBDIR \
                | PKG_CONFIG_SYSROOT_DIR)
                [[ -z "${!variable-}" ]] || return 1
                ;;
        esac
    done <"$names"
}

check_cache_environment_is_tool() {
    case "$1" in
        CARGO | RUSTC | RUSTDOC | RUSTC_WRAPPER \
            | RUSTC_WORKSPACE_WRAPPER | CC | CC_* | CXX | CXX_* \
            | AR | AR_* | RANLIB | RANLIB_* | LD | LD_* \
            | STRIP | STRIP_* | OBJCOPY | OBJCOPY_* \
            | PKG_CONFIG | PKG_CONFIG_* | PROTOC \
            | CARGO_TARGET_*_LINKER | CARGO_TARGET_*_RUNNER \
            | CARGO_BUILD_RUSTC | CARGO_BUILD_RUSTDOC \
            | CARGO_BUILD_RUSTC_WRAPPER \
            | CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER)
            return 0
            ;;
    esac
    return 1
}

check_cache_record_override_tools() {
    local output="$1"
    local names="$output.override-names"
    local variable value resolved identity

    compgen -e \
        | check_cache_bound_byte_stream "$CHECK_CACHE_MAX_CONTEXT_BYTES" \
        | LC_ALL=C sort -u >"$names" || return 1
    while IFS= read -r variable; do
        check_cache_environment_is_tool "$variable" || continue
        value="${!variable-}"
        [[ -n "$value" ]] || continue
        resolved="$(command -v -- "$value")" || return 1
        resolved="$(readlink -f -- "$resolved")" || return 1
        [[ -f "$resolved" && -r "$resolved" ]] || return 1
        check_cache_file_size_within \
            "$resolved" "$CHECK_CACHE_MAX_TOOL_BYTES" || return 1
        identity="$(git hash-object --no-filters -- "$resolved")" || {
            return 1
        }
        printf 'override-tool\0%s\0%s\0%s\0%s\0' \
            "$variable" "$value" "$resolved" "$identity" >>"$output"
    done <"$names"
}

check_cache_record_optional_file() {
    local output="$1"
    local path="$2"
    local resolved identity

    if [[ ! -e "$path" && ! -L "$path" ]]; then
        printf 'configuration\0%s\0absent\0' "$path" >>"$output"
        return
    fi
    [[ -f "$path" && -r "$path" ]] || return 1
    resolved="$(readlink -f -- "$path")" || return 1
    check_cache_file_size_within \
        "$resolved" "$CHECK_CACHE_MAX_INPUT_BYTES" || return 1
    identity="$(git hash-object --no-filters -- "$resolved")" || return 1
    printf 'configuration\0%s\0%s\0%s\0' \
        "$path" "$resolved" "$identity" >>"$output"
}

check_cache_repository_cargo_configuration_present() {
    local current="$ROOT"

    while true; do
        if [[ -e "$current/.cargo/config.toml" \
            || -L "$current/.cargo/config.toml" \
            || -e "$current/.cargo/config" \
            || -L "$current/.cargo/config" ]]; then
            return 0
        fi
        [[ "$current" != / ]] || return 1
        current="${current%/*}"
        [[ -n "$current" ]] || current=/
    done
}

check_cache_record_local_git_configuration() {
    local output="$1"
    local git_dir common_dir include_status

    git_dir="$(git -C "$ROOT" rev-parse --absolute-git-dir)" || return 1
    common_dir="$(git -C "$ROOT" rev-parse --git-common-dir)" || return 1
    if [[ "$common_dir" != /* ]]; then
        common_dir="$ROOT/$common_dir"
    fi
    common_dir="$(realpath -m -- "$common_dir")" || return 1
    if git -C "$ROOT" config --local --name-only \
        --get-regexp '^include(If\..*)?\.' >/dev/null 2>&1; then
        return 1
    else
        include_status=$?
        ((include_status == 1)) || return 1
    fi
    if [[ -f "$ROOT/.git" && ! -L "$ROOT/.git" ]]; then
        check_cache_record_optional_file "$output" "$ROOT/.git" || {
            return 1
        }
    elif [[ ! -d "$ROOT/.git" || -L "$ROOT/.git" ]]; then
        return 1
    fi
    check_cache_record_optional_file \
        "$output" "$common_dir/config" || return 1
    check_cache_record_optional_file \
        "$output" "$git_dir/config.worktree"
}

check_cache_record_cargo_configuration() {
    local output="$1"
    local current="$ROOT"
    local cargo_home_path
    local found=no

    while true; do
        if [[ -e "$current/.cargo/config.toml" \
            || -L "$current/.cargo/config.toml" \
            || -e "$current/.cargo/config" \
            || -L "$current/.cargo/config" ]]; then
            found=yes
        fi
        check_cache_record_optional_file \
            "$output" "$current/.cargo/config.toml" || return 1
        check_cache_record_optional_file \
            "$output" "$current/.cargo/config" || return 1
        [[ "$current" != / ]] || break
        current="${current%/*}"
        [[ -n "$current" ]] || current=/
    done
    cargo_home_path="${CARGO_HOME-}"
    if [[ -z "$cargo_home_path" ]]; then
        [[ -n "${HOME-}" ]] || return 1
        cargo_home_path="$HOME/.cargo"
    elif [[ "$cargo_home_path" != /* ]]; then
        cargo_home_path="$ROOT/$cargo_home_path"
    fi
    cargo_home_path="$(realpath -m -- "$cargo_home_path")" || return 1
    if [[ -e "$cargo_home_path/config.toml" \
        || -L "$cargo_home_path/config.toml" \
        || -e "$cargo_home_path/config" \
        || -L "$cargo_home_path/config" ]]; then
        found=yes
    fi
    check_cache_record_optional_file \
        "$output" "$cargo_home_path/config.toml" || return 1
    check_cache_record_optional_file \
        "$output" "$cargo_home_path/config" || return 1
    [[ "$found" == no ]]
}

check_cache_context_paths() {
    local material="$1"

    perl -0 - "$material" <<'PERL'
use strict;
use warnings;

my ($material) = @ARGV;
open my $handle, '<:raw', $material or die "context open failed: $!";
local $/;
my @fields = split /\0/, <$handle>, -1;
close $handle or die "context close failed: $!";
pop @fields if @fields && $fields[-1] eq '';
die "context prefix is incomplete" if @fields < 2;
my $index = 2;
while ($index < @fields) {
    my $kind = $fields[$index++];
    if ($kind eq 'tool') {
        die "tool record is incomplete" if $index + 4 >= @fields;
        my ($name, $resolved, $implementation, $identity, $version) =
            @fields[$index .. $index + 4];
        $index += 5;
        print "tool-command\0$resolved\0";
        print "tool-implementation\0$implementation\0";
    } elsif ($kind eq 'override-tool') {
        die "override record is incomplete" if $index + 3 >= @fields;
        my ($name, $value, $resolved, $identity) =
            @fields[$index .. $index + 3];
        $index += 4;
        print "override-tool\0$resolved\0";
    } elsif ($kind eq 'configuration') {
        die "configuration record is incomplete" if $index + 1 >= @fields;
        my ($path, $next) = @fields[$index .. $index + 1];
        $index += 2;
        if ($next ne 'absent') {
            die "configuration identity is absent" if $index >= @fields;
            $index++;
        }
        print "configuration\0$path\0";
    } elsif ($kind eq 'environment' || $kind eq 'input') {
        die "$kind record is incomplete" if $index + 1 >= @fields;
        $index += 2;
    } else {
        die "unknown context record: $kind";
    }
}
PERL
}

check_cache_observe_context_path() {
    local output="$1"
    local kind="$2"
    local path="$3"
    local parent resolved stamp

    [[ -n "$path" && ${#path} -le CHECK_CACHE_MAX_PATH_BYTES ]] || {
        return 1
    }
    if [[ -e "$path" || -L "$path" ]]; then
        stamp="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' -- "$path")" || {
            return 1
        }
        printf 'path\0%s\0%s\0%s\0' \
            "$kind" "$path" "$stamp" >>"$output"
        resolved="$(readlink -f -- "$path")" || return 1
        if [[ "$resolved" != "$path" ]]; then
            stamp="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' \
                -- "$resolved")" || return 1
            printf 'resolved\0%s\0%s\0' \
                "$resolved" "$stamp" >>"$output"
        fi
        return
    fi
    if [[ "$kind" == configuration ]]; then
        printf 'absent\0%s\0%s\0not-present\0' \
            "$kind" "$path" >>"$output"
        return
    fi
    parent="${path%/*}"
    while [[ ! -d "$parent" || -L "$parent" ]]; do
        [[ "$parent" == / || -z "$parent" ]] && return 1
        parent="${parent%/*}"
        [[ -n "$parent" ]] || parent=/
    done
    stamp="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' -- "$parent")" || {
        return 1
    }
    printf 'absent\0%s\0%s\0%s\0' \
        "$kind" "$path" "$stamp" >>"$output"
}

check_cache_context_observation() {
    local material="$1"
    local output="$2"
    local paths="$output.paths"
    local search_paths="$output.search-paths"
    local kind path

    : >"$output"
    check_cache_file_size_within \
        "$material" "$CHECK_CACHE_MAX_CONTEXT_BYTES" || return 1
    check_cache_context_paths "$material" \
        | check_cache_bound_nul_stream \
            "$CHECK_CACHE_MAX_DIRECTORIES" \
            "$CHECK_CACHE_MAX_CONTEXT_BYTES" \
            "$CHECK_CACHE_MAX_PATH_BYTES" \
        >"$paths" || return 1
    while IFS= read -r -d '' kind \
        && IFS= read -r -d '' path; do
        check_cache_observe_context_path \
            "$output" "$kind" "$path" || return 1
    done <"$paths"
    perl -e '
use strict;
use warnings;

my ($root) = @ARGV;
for my $path (split /:/, ($ENV{PATH} // q{}), -1) {
    $path = $root if $path eq q{};
    $path = "$root/$path" if $path !~ m{\A/};
    print "$path\0" or die "PATH record write failed: $!";
}
' "$ROOT" \
        | check_cache_bound_nul_stream \
            "$CHECK_CACHE_MAX_DIRECTORIES" \
            "$CHECK_CACHE_MAX_CONTEXT_BYTES" \
            "$CHECK_CACHE_MAX_PATH_BYTES" \
        >"$search_paths" || return 1
    while IFS= read -r -d '' path; do
        path="$(realpath -m -- "$path")" || return 1
        check_cache_observe_context_path \
            "$output" path-search "$path" || return 1
    done <"$search_paths"
    check_cache_file_size_within \
        "$output" "$CHECK_CACHE_MAX_CONTEXT_BYTES"
}

check_cache_record_file() {
    local output="$1"
    local relative="$2"
    local identity

    [[ -f "$ROOT/$relative" && ! -L "$ROOT/$relative" ]] || return 1
    check_cache_file_size_within \
        "$ROOT/$relative" "$CHECK_CACHE_MAX_INPUT_BYTES" || return 1
    identity="$(git hash-object --no-filters -- "$ROOT/$relative")" || {
        return 1
    }
    printf 'input\0%s\0%s\0' "$relative" "$identity" >>"$output"
}

check_cache_context() {
    local directory="$1"
    local material="$directory/context.input"
    local program
    local -a programs=(
        ar
        awk
        bash
        cargo
        cargo-clippy
        cargo-fmt
        cat
        cc
        chmod
        clippy-driver
        cmp
        cp
        dirname
        env
        find
        git
        grep
        ln
        ld
        mkdir
        mktemp
        mv
        perl
        readlink
        realpath
        ranlib
        rg
        rm
        rustc
        rustdoc
        rustfmt
        rustup
        sed
        sha256sum
        sort
        stat
        strip
        tr
        uniq
        wc
        objcopy
    )

    : >"$material"
    printf '%s\0%s\0' "$CHECK_CACHE_MAGIC" \
        'host=x86_64-unknown-linux-gnu;portable=x86_64-unknown-none' \
        >"$material"
    check_cache_reject_injected_environment || return 1
    check_cache_reject_unbound_flags \
        "$material.unbound-flag-names" || return 1
    for program in "${programs[@]}"; do
        check_cache_record_tool "$material" "$program" || return 1
    done
    check_cache_require_fixed_tool env /usr/bin/env || return 1
    check_cache_require_fixed_tool git /usr/bin/git || return 1
    check_cache_record_environment "$material" || return 1
    check_cache_record_override_tools "$material" || return 1
    check_cache_record_local_git_configuration "$material" || return 1
    check_cache_record_cargo_configuration "$material" || return 1
    check_cache_record_file "$material" scripts/check.sh || return 1
    check_cache_record_file \
        "$material" scripts/check-cache.sh || return 1
    check_cache_record_file "$material" scripts/check-style.sh || return 1
    check_cache_record_file \
        "$material" scripts/check-documentation.sh || return 1
    check_cache_record_file "$material" rustfmt.toml || return 1
    check_cache_record_file "$material" clippy.toml || return 1
    check_cache_file_size_within \
        "$material" "$CHECK_CACHE_MAX_CONTEXT_BYTES" || return 1
    check_cache_context_observation \
        "$material" "$directory/context.observation" || return 1
    git hash-object --no-filters -- "$material"
}

check_cache_emit_workspace_graph() {
    local metadata="$1"
    local workspace="$2"
    local manifest="$3"
    local lockfile="$4"

    perl -MJSON::PP - "$ROOT" "$workspace" "$manifest" \
        "$lockfile" "$metadata" <<'PERL'
use strict;
use warnings;
use JSON::PP qw(decode_json);

my ($repo, $workspace, $manifest, $lockfile, $metadata) = @ARGV;
open my $handle, '<:raw', $metadata or die "metadata open failed: $!";
local $/;
my $value = decode_json(<$handle>);
close $handle or die "metadata close failed: $!";

sub field_ok {
    for my $field (@_) {
        return 0 if !defined($field) || $field =~ /[\t\r\n]/;
    }
    return 1;
}

sub relative {
    my ($path) = @_;
    my $prefix = "$repo/";
    die "path outside repository: $path" if index($path, $prefix) != 0;
    return substr($path, length($prefix));
}

die "workspace fields are invalid"
    if !field_ok($workspace, $manifest, $lockfile);
print join("\t", 'workspace', $workspace, $manifest, $lockfile), "\n";

my @packages = sort { $a->{name} cmp $b->{name} } @{$value->{packages}};
my %component_by_root;
my %component_by_name;
for my $package (@packages) {
    my $name = $package->{name};
    my $manifest_path = relative($package->{manifest_path});
    my $root = $manifest_path;
    $root =~ s{/Cargo\.toml\z}{};
    my $component = "$workspace:$name";
    die "package fields are invalid"
        if !field_ok($component, $workspace, $name, $root) || $root eq '';
    die "duplicate package root" if exists $component_by_root{$root};
    die "duplicate package name" if exists $component_by_name{$name};
    $component_by_root{$root} = $component;
    $component_by_name{$name} = $component;
    print join(
        "\t", 'package', $component, $workspace, $name, $root
    ), "\n";
}

for my $package (@packages) {
    my $consumer = $component_by_name{$package->{name}};
    for my $dependency (@{$package->{dependencies}}) {
        next if !defined($dependency->{path});
        my $root = relative("$dependency->{path}/Cargo.toml");
        $root =~ s{/Cargo\.toml\z}{};
        my $provider = $component_by_root{$root};
        die "unresolved local dependency" if !defined($provider);
        print join("\t", 'edge', $provider, $consumer), "\n";
    }
}
PERL
}

check_cache_validate_graph() {
    local graph="$1"

    perl - "$graph" <<'PERL'
use strict;
use warnings;

my ($graph) = @ARGV;
open my $handle, '<:raw', $graph or die "graph open failed: $!";
my (%components, %workspaces, @edges);
while (my $line = <$handle>) {
    chomp $line;
    my @fields = split /\t/, $line, -1;
    if ($fields[0] eq 'workspace' && @fields == 4) {
        die "duplicate workspace" if $workspaces{$fields[1]}++;
    } elsif ($fields[0] eq 'package' && @fields == 5) {
        die "duplicate component" if $components{$fields[1]}++;
        die "invalid package root"
            if $fields[4] eq '' || $fields[4] =~ m{(?:\A|/)\.\.?(/|\z)};
    } elsif ($fields[0] eq 'edge' && @fields == 3) {
        push @edges, [$fields[1], $fields[2]];
    } else {
        die "invalid graph row";
    }
}
close $handle or die "graph close failed: $!";
die "empty workspace graph" if !%workspaces || !%components;
for my $edge (@edges) {
    die "edge endpoint is absent"
        if !$components{$edge->[0]} || !$components{$edge->[1]};
}
PERL
}

check_cache_capture_metadata() {
    local output="$1"
    local target="$2"
    shift 2

    CARGO_TARGET_DIR="$target" \
        cargo metadata --locked --offline --no-deps --format-version 1 \
            "$@" \
        | check_cache_bound_byte_stream \
            "$CHECK_CACHE_MAX_METADATA_BYTES" \
        >"$output"
}

check_cache_validate_metadata_target() {
    local metadata="$1"
    local expected="$2"

    perl -MJSON::PP -0 -e '
use strict;
use warnings;

my ($expected) = @ARGV;
my $value = decode_json(<STDIN>);
die "metadata target escaped scratch"
    if $value->{target_directory} ne $expected;
' "$expected" <"$metadata"
}

check_cache_load_graph() {
    local directory="$1"
    local graph="$directory/graph.tsv"
    local product_metadata="$directory/product-metadata.json"
    local metadata_target="$directory/cargo-metadata-target"

    mkdir -m 700 -- "$metadata_target" || return 1
    [[ -d "$metadata_target" && ! -L "$metadata_target" \
        && -O "$metadata_target" ]] || return 1
    check_cache_capture_metadata \
        "$product_metadata" "$metadata_target" \
        --manifest-path "$ROOT/Cargo.toml" || return 1
    check_cache_validate_metadata_target \
        "$product_metadata" "$metadata_target" || return 1
    {
        check_cache_emit_workspace_graph \
            "$product_metadata" product Cargo.toml Cargo.lock
    } | check_cache_bound_byte_stream "$CHECK_CACHE_MAX_GRAPH_BYTES" \
        | LC_ALL=C sort -u >"$graph" || return 1
    check_cache_file_size_within \
        "$graph" "$CHECK_CACHE_MAX_GRAPH_BYTES" || return 1
    check_cache_validate_graph "$graph" || return 1
    printf '%s\n' "$graph"
}

check_cache_package_owner() {
    local path="$1"
    local graph="$2"
    local kind component workspace name root
    local found=''
    local found_length=0

    while IFS=$'\t' read -r kind component workspace name root; do
        [[ "$kind" == package ]] || continue
        if [[ "$path" != "$root" && "$path" != "$root/"* ]]; then
            continue
        fi
        if [[ ${#root} -lt found_length ]]; then
            continue
        fi
        if [[ ${#root} -eq found_length && -n "$found" ]]; then
            return 1
        fi
        found="$component"
        found_length=${#root}
    done <"$graph"
    [[ -n "$found" ]] || return 1
    printf 'package:%s\n' "$found"
}

check_cache_owner_for_path() {
    local path="$1"
    local graph="$2"

    case "$path" in
        Cargo.toml | Cargo.lock)
            printf '%s\n' 'workspace:product'
            return
            ;;
        Documentation/* | changelog/* | perf/* | .github/ISSUE_TEMPLATE/*)
            printf '%s\n' documentation
            return
            ;;
        README.md | CHANGELOG.md | CONTRIBUTING.md | GOVERNANCE.md | LICENSE)
            printf '%s\n' documentation
            return
            ;;
        .github/ACTIONS-REVIEW.md | .github/PULL_REQUEST_TEMPLATE.md)
            printf '%s\n' documentation
            return
            ;;
        scripts/* | .github/workflows/* | .cargo/* | AGENTS.md)
            printf '%s\n' global
            return
            ;;
        CODE_STYLE.md | rustfmt.toml | clippy.toml | ci.json | .gitignore)
            printf '%s\n' global
            return
            ;;
        .gitattributes)
            printf '%s\n' global
            return
            ;;
    esac
    check_cache_package_owner "$path" "$graph"
}

check_cache_seal_file() {
    local path="$1"
    local identity

    identity="$(git hash-object --no-filters -- "$path")" || return 1
    printf 'seal\t%s\n' "$identity" >>"$path"
}

check_cache_snapshot() {
    local graph="$1"
    local context="$2"
    local output="$3"
    local observation="$4"
    local dot='.'
    local path owner mode size device inode modified changed parent
    local before after identity
    local total_bytes=0
    local -a paths=() directories_sorted=()
    local paths_file="$output.paths.z"
    local directories_file="$output.directories.z"
    local directories_sorted_file="$output.directories-sorted.z"
    declare -A directories=()

    git ls-files --cached --others \
        --exclude-from="$ROOT/.gitignore" -z \
        | check_cache_bound_nul_stream \
            "$CHECK_CACHE_MAX_STATE_FILES" \
            "$CHECK_CACHE_MAX_INVENTORY_BYTES" \
            "$CHECK_CACHE_MAX_PATH_BYTES" \
        | LC_ALL=C sort -z >"$paths_file" || return 1
    mapfile -d '' -t paths <"$paths_file" || return 1
    printf '%s\n' "$CHECK_CACHE_MAGIC" >"$output"
    printf 'context\t%s\n' "$context" >>"$output"
    printf '%s\n' "$CHECK_CACHE_MAGIC-observation" >"$observation"
    printf 'context\t%s\n' "$context" >>"$observation"
    for path in "${paths[@]}"; do
        case "$path" in
            target/* | tmp/*) continue ;;
        esac
        check_cache_normal_path "$path" || return 1
        parent="${path%/*}"
        [[ "$parent" != "$path" ]] || parent=.
        while true; do
            directories["$parent"]=1
            [[ "$parent" != "$dot" ]] || break
            if [[ "$parent" == */* ]]; then
                parent="${parent%/*}"
            else
                parent=.
            fi
        done
        [[ -e "$ROOT/$path" ]] || continue
        [[ -f "$ROOT/$path" && ! -L "$ROOT/$path" ]] || return 1
        owner="$(check_cache_owner_for_path "$path" "$graph")" || return 1
        before="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' \
            -- "$ROOT/$path")" || return 1
        IFS=$'\t' read -r \
            mode size device inode modified changed <<<"$before"
        case "$size" in
            '' | *[!0-9]*) return 1 ;;
        esac
        ((${#size} <= ${#CHECK_CACHE_MAX_INPUT_BYTES})) || return 1
        ((10#$size <= CHECK_CACHE_MAX_INPUT_BYTES)) || return 1
        total_bytes=$((total_bytes + 10#$size))
        ((total_bytes <= CHECK_CACHE_MAX_INPUT_TOTAL_BYTES)) || return 1
        identity="$(git hash-object --no-filters -- "$ROOT/$path")" || {
            return 1
        }
        after="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' \
            -- "$ROOT/$path")" || return 1
        [[ "$before" == "$after" ]] || return 1
        printf 'file\t%s\t%s\t%s\t%s\t%s\n' \
            "$owner" "$mode" "$size" "$identity" "$path" >>"$output"
        printf 'file\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$owner" "$mode" "$size" "$device" "$inode" \
            "$modified" "$changed" "$identity" "$path" >>"$observation"
    done
    find "$ROOT" \
        \( -path "$ROOT/.git" -o -path "$ROOT/target" \
            -o -path "$ROOT/tmp" \) \
        -prune -o -type d -print0 \
        | check_cache_bound_nul_stream \
            "$CHECK_CACHE_MAX_DIRECTORIES" \
            "$CHECK_CACHE_MAX_INVENTORY_BYTES" \
            "$CHECK_CACHE_MAX_PATH_BYTES" \
        >"$directories_file" || return 1
    while IFS= read -r -d '' parent; do
        if [[ "$parent" == "$ROOT" ]]; then
            parent=.
        else
            parent="${parent#"$ROOT/"}"
        fi
        directories["$parent"]=1
        if [[ ${#directories[@]} -gt CHECK_CACHE_MAX_DIRECTORIES ]]; then
            return 1
        fi
    done <"$directories_file"
    printf '%s\0' "${!directories[@]}" \
        | LC_ALL=C sort -z >"$directories_sorted_file" || return 1
    mapfile -d '' -t directories_sorted \
        <"$directories_sorted_file" || return 1
    for parent in "${directories_sorted[@]}"; do
        [[ -d "$ROOT/$parent" && ! -L "$ROOT/$parent" ]] || return 1
        before="$(stat -c $'%f\t%s\t%d\t%i\t%y\t%z' \
            -- "$ROOT/$parent")" || return 1
        printf 'directory\t%s\t%s\n' \
            "$before" "$parent" >>"$observation"
    done
    check_cache_file_size_within \
        "$output" "$CHECK_CACHE_MAX_STATE_BYTES" || return 1
    check_cache_file_size_within \
        "$observation" "$CHECK_CACHE_MAX_STATE_BYTES" || return 1
    check_cache_seal_file "$output" || return 1
    check_cache_seal_file "$observation" || return 1
    check_cache_file_size_within \
        "$output" "$CHECK_CACHE_MAX_STATE_BYTES" || return 1
    check_cache_file_size_within \
        "$observation" "$CHECK_CACHE_MAX_STATE_BYTES"
}

check_cache_hex_identity() {
    local value="$1"

    case "$value" in
        *[!0-9a-f]*) return 1 ;;
    esac
    [[ ${#value} == 40 || ${#value} == 64 ]]
}

check_cache_validate_state() {
    local state="$1"
    local context="$2"
    local graph="$3"
    local first second seal_line seal_kind seal_identity seal_extra actual
    local kind owner mode size identity path extra expected
    local state_size index last_index
    local total_bytes=0
    local previous=''
    local -a lines=()

    state_size="$(stat -c '%s' -- "$state")" || return 1
    case "$state_size" in
        '' | *[!0-9]*) return 1 ;;
    esac
    ((state_size <= CHECK_CACHE_MAX_STATE_BYTES)) || return 1
    mapfile -t lines <"$state" || return 1
    ((${#lines[@]} >= 3)) || return 1
    ((${#lines[@]} <= CHECK_CACHE_MAX_STATE_FILES + 3)) || return 1
    last_index=$((${#lines[@]} - 1))
    first="${lines[0]}"
    [[ "$first" == "$CHECK_CACHE_MAGIC" ]] || return 1
    second="${lines[1]}"
    [[ "$second" == $'context\t'"$context" ]] || return 1
    seal_line="${lines[$last_index]}"
    IFS=$'\t' read -r \
        seal_kind seal_identity seal_extra <<<"$seal_line"
    [[ "$seal_kind" == seal && -z "$seal_extra" ]] || return 1
    check_cache_hex_identity "$seal_identity" || return 1
    actual="$(sed '$d' "$state" | git hash-object --stdin)" || return 1
    [[ "$actual" == "$seal_identity" ]] || return 1
    for ((index = 2; index < last_index; index++)); do
        IFS=$'\t' read -r \
            kind owner mode size identity path extra <<<"${lines[$index]}"
        [[ "$kind" == file && -z "$extra" ]] || return 1
        check_cache_normal_path "$path" || return 1
        [[ -z "$previous" || "$previous" < "$path" ]] || return 1
        case "$mode" in
            '' | *[!0-9a-f]*) return 1 ;;
        esac
        case "$size" in
            '' | *[!0-9]*) return 1 ;;
        esac
        ((${#size} <= ${#CHECK_CACHE_MAX_INPUT_BYTES})) || return 1
        ((10#$size <= CHECK_CACHE_MAX_INPUT_BYTES)) || return 1
        total_bytes=$((total_bytes + 10#$size))
        ((total_bytes <= CHECK_CACHE_MAX_INPUT_TOTAL_BYTES)) || return 1
        check_cache_hex_identity "$identity" || return 1
        expected="$(check_cache_owner_for_path "$path" "$graph")" || {
            return 1
        }
        [[ "$owner" == "$expected" ]] || return 1
        previous="$path"
    done
}

check_cache_load_records() {
    local state="$1"
    local owner_name="$2"
    local signature_name="$3"
    local -n owners="$owner_name"
    local -n signatures="$signature_name"
    local kind owner mode size identity path extra
    local index last_index
    local -a lines=()

    mapfile -t lines <"$state" || return 1
    last_index=$((${#lines[@]} - 1))
    for ((index = 2; index < last_index; index++)); do
        IFS=$'\t' read -r \
            kind owner mode size identity path extra <<<"${lines[$index]}"
        owners["$path"]="$owner"
        signatures["$path"]="$mode:$size:$identity"
    done
}

check_cache_add_owner() {
    local owner="$1"
    local graph="$2"
    local direct="$3"
    local full="$4"
    local workspace kind component candidate_workspace name root
    local matched=0

    case "$owner" in
        documentation)
            printf '%s\n' documentation >>"$direct"
            ;;
        package:*)
            printf '%s\n' "${owner#package:}" >>"$direct"
            ;;
        workspace:*)
            workspace="${owner#workspace:}"
            while IFS=$'\t' read -r \
                kind component candidate_workspace name root; do
                [[ "$kind" == package ]] || continue
                [[ "$candidate_workspace" == "$workspace" ]] || continue
                printf '%s\n' "$component" >>"$direct"
                matched=1
            done <"$graph"
            ((matched == 1)) || return 1
            ;;
        global)
            : >"$full"
            ;;
        *) return 1 ;;
    esac
}

check_cache_expand_components() {
    local graph="$1"
    local direct="$2"
    local selected="$3"
    local before="$selected.before"
    local candidate="$selected.candidate"
    local kind provider consumer

    LC_ALL=C sort -u -- "$direct" >"$selected" || return 1
    while true; do
        cp -- "$selected" "$before" || return 1
        cp -- "$selected" "$candidate" || return 1
        while IFS=$'\t' read -r kind provider consumer; do
            [[ "$kind" == edge ]] || continue
            if grep -Fqx -- "$provider" "$selected"; then
                printf '%s\n' "$consumer" >>"$candidate"
            fi
        done <"$graph"
        LC_ALL=C sort -u -- "$candidate" >"$selected" || return 1
        cmp -s -- "$before" "$selected" && break
    done
}

check_cache_write_plan() {
    local direct="$1"
    local selected="$2"
    local plan="$3"
    local component mode

    : >"$plan"
    while IFS= read -r component; do
        [[ -n "$component" ]] || continue
        mode=dependent
        if grep -Fqx -- "$component" "$direct"; then
            mode=direct
        fi
        printf '%s\t%s\n' "$mode" "$component" >>"$plan"
    done <"$selected"
}

check_cache_select_states() {
    local old="$1"
    local current="$2"
    local graph="$3"
    local directory="$4"
    local direct="$directory/direct"
    local selected="$directory/selected"
    local changed="$directory/changed.unsorted"
    local changed_sorted="$directory/changed-paths.z"
    local full="$directory/full"
    local path owner
    declare -A old_owners=() old_signatures=()
    declare -A current_owners=() current_signatures=()

    : >"$direct"
    : >"$changed"
    rm -f -- "$full"
    check_cache_load_records \
        "$old" old_owners old_signatures
    check_cache_load_records \
        "$current" current_owners current_signatures
    for path in "${!old_owners[@]}"; do
        if [[ -z "${current_owners[$path]+present}" ]]; then
            check_cache_add_owner \
                "${old_owners[$path]}" "$graph" "$direct" "$full" || {
                return 1
            }
        fi
    done
    for path in "${!current_owners[@]}"; do
        if [[ -n "${old_owners[$path]+present}" ]] \
            && [[ "${old_signatures[$path]}" \
                == "${current_signatures[$path]}" ]] \
            && [[ "${old_owners[$path]}" == "${current_owners[$path]}" ]]; then
            continue
        fi
        owner="${current_owners[$path]}"
        check_cache_add_owner "$owner" "$graph" "$direct" "$full" || {
            return 1
        }
        if [[ -n "${old_owners[$path]+present}" ]] \
            && [[ "${old_owners[$path]}" != "$owner" ]]; then
            check_cache_add_owner \
                "${old_owners[$path]}" "$graph" "$direct" "$full" || {
                return 1
            }
        fi
        printf '%s\0' "$path" >>"$changed"
    done
    LC_ALL=C sort -zu -- "$changed" >"$changed_sorted" || return 1
    if [[ -e "$full" ]]; then
        printf '%s\n' full >"$directory/mode"
        printf '%s\n' 'global input changed' >"$directory/reason"
        return
    fi
    LC_ALL=C sort -u -o "$direct" -- "$direct" || return 1
    if [[ ! -s "$direct" ]]; then
        printf '%s\n' hit >"$directory/mode"
        printf '%s\n' 'all inputs have successful coverage' \
            >"$directory/reason"
        : >"$directory/plan.tsv"
        return
    fi
    check_cache_expand_components "$graph" "$direct" "$selected" || {
        return 1
    }
    check_cache_write_plan "$direct" "$selected" \
        "$directory/plan.tsv" || return 1
    printf '%s\n' selected >"$directory/mode"
    printf '%s\n' 'known component inputs changed' >"$directory/reason"
}

check_cache_full_plan() {
    local directory="$1"
    local reason="$2"
    local publishable="$3"

    printf '%s\n' full >"$directory/mode"
    printf '%s\n' "$reason" >"$directory/reason"
    printf '%s\n' "$publishable" >"$directory/publishable"
    : >"$directory/plan.tsv"
    : >"$directory/changed-paths.z"
}

check_cache_prepare() {
    local directory="$1"
    local force_full="$2"
    local context graph state snapshot context_directory state_mode

    if ! mkdir -p -- "$ROOT/target" \
        || ! check_cache_safe_directory "$ROOT/target"; then
        check_cache_full_plan \
            "$directory" 'local cache root is unavailable' no
        return
    fi

    if ! context="$(check_cache_context "$directory")"; then
        check_cache_full_plan \
            "$directory" 'cache context is unavailable' no
        return
    fi
    printf '%s\n' "$context" >"$directory/context"
    if ! graph="$(check_cache_load_graph "$directory")"; then
        check_cache_full_plan \
            "$directory" 'component graph is unavailable' no
        return
    fi
    snapshot="$directory/pre-state.v1"
    if ! check_cache_snapshot \
        "$graph" "$context" "$snapshot" \
        "$directory/pre-observation.v1"; then
        # Failed output is not an input to the full fallback's state fixtures.
        rm -f -- "$snapshot" "$directory/pre-observation.v1" || return 1
        check_cache_full_plan \
            "$directory" 'working input snapshot is unavailable' no
        return
    fi
    printf '%s\n' yes >"$directory/publishable"
    state="$CHECK_CACHE_ROOT/$context/state.v1"
    printf '%s\n' "$state" >"$directory/state-path"
    if [[ "$force_full" == yes ]]; then
        check_cache_full_plan "$directory" 'explicit full mode' yes
        return
    fi
    if [[ ! -f "$state" || -L "$state" ]]; then
        check_cache_full_plan \
            "$directory" 'successful local coverage is absent' yes
        return
    fi
    context_directory="${state%/state.v1}"
    state_mode="$(stat -c '%a' -- "$state")" || {
        check_cache_full_plan \
            "$directory" 'local coverage metadata is unavailable' no
        return
    }
    if ! check_cache_safe_directory "$CHECK_CACHE_ROOT" \
        || ! check_cache_safe_directory "$context_directory" \
        || [[ ! -O "$state" ]] \
        || [[ "$state_mode" != 600 ]]; then
        check_cache_full_plan \
            "$directory" 'local coverage ownership is invalid' no
        return
    fi
    if ! check_cache_validate_state "$state" "$context" "$graph"; then
        check_cache_full_plan \
            "$directory" 'local coverage is malformed' yes
        return
    fi
    if ! check_cache_select_states \
        "$state" "$snapshot" "$graph" "$directory"; then
        check_cache_full_plan \
            "$directory" 'component selection is uncertain' yes
        return
    fi
    printf '%s\n' yes >"$directory/publishable"
}

check_cache_safe_directory() {
    local directory="$1"

    [[ "$directory" == "$ROOT/target" \
        || "$directory" == "$ROOT/target/"* ]] || return 1
    [[ -d "$directory" && ! -L "$directory" && -O "$directory" ]]
}

check_cache_inputs_stable() {
    local directory="$1"
    local post="$2"
    local observation="$3"

    if ! cmp -s -- "$directory/pre-state.v1" "$post"; then
        check_cache_issue CHECK_CACHE_INPUT_DRIFT \
            'inputs changed while checks were executing'
        return 1
    fi
    if ! cmp -s -- "$directory/pre-observation.v1" "$observation"; then
        check_cache_issue CHECK_CACHE_INPUT_ABA \
            'input metadata changed while checks were executing'
        return 1
    fi
}

check_cache_atomic_replace() {
    local context_directory="$1"
    local state="$2"
    local post="$3"
    local temporary

    [[ "$state" == "$context_directory/state.v1" ]] || return 1
    mkdir -p -- "$context_directory" || return 1
    [[ -d "$context_directory" && ! -L "$context_directory" \
        && -O "$context_directory" ]] || return 1
    if [[ -e "$state" || -L "$state" ]]; then
        [[ -f "$state" && ! -L "$state" && -O "$state" ]] || return 1
    fi
    temporary="$(mktemp "$context_directory/.state.XXXXXX")" || return 1
    if ! cp -- "$post" "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    chmod 600 -- "$temporary" || {
        rm -f -- "$temporary"
        return 1
    }
    mv -fT -- "$temporary" "$state"
}

check_cache_publish() {
    local directory="$1"
    local publishable mode context graph state expected_state
    local context_directory post observation post_context_directory
    local post_context

    publishable="$(sed -n '1p' "$directory/publishable")"
    [[ "$publishable" == yes ]] || return 0
    mode="$(sed -n '1p' "$directory/mode")"
    context="$(sed -n '1p' "$directory/context")"
    check_cache_hex_identity "$context" || return 1
    graph="$directory/graph.tsv"
    state="$(sed -n '1p' "$directory/state-path")"
    expected_state="$CHECK_CACHE_ROOT/$context/state.v1"
    [[ "$state" == "$expected_state" ]] || return 1
    post_context_directory="$directory/post-context"
    mkdir -p -- "$post_context_directory" || return 1
    post_context="$(check_cache_context "$post_context_directory")" || {
        check_cache_issue CHECK_CACHE_CONTEXT_DRIFT \
            'post-check tool context is unavailable'
        return 1
    }
    if [[ "$post_context" != "$context" ]] \
        || ! cmp -s -- \
            "$directory/context.observation" \
            "$post_context_directory/context.observation"; then
        check_cache_issue CHECK_CACHE_CONTEXT_DRIFT \
            'tool or configuration context changed during checks'
        return 1
    fi
    post="$directory/post-state.v1"
    observation="$directory/post-observation.v1"
    if ! check_cache_snapshot \
        "$graph" "$context" "$post" \
        "$observation"; then
        check_cache_issue CHECK_CACHE_POST_SNAPSHOT \
            'post-check input snapshot is unavailable'
        return 1
    fi
    check_cache_inputs_stable "$directory" "$post" "$observation" || {
        return 1
    }
    [[ "$mode" != hit ]] || return 0
    mkdir -p -- "$ROOT/target" || return 1
    check_cache_safe_directory "$ROOT/target" || return 1
    mkdir -p -- "$CHECK_CACHE_ROOT" || return 1
    check_cache_safe_directory "$CHECK_CACHE_ROOT" || return 1
    context_directory="$CHECK_CACHE_ROOT/$context"
    mkdir -p -- "$context_directory" || return 1
    check_cache_safe_directory "$context_directory" || return 1
    check_cache_atomic_replace "$context_directory" "$state" "$post"
}

check_cache_assert_set() {
    local name="$1"
    local actual="$2"
    local expected="$3"

    if ! cmp -s -- "$actual" "$expected"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            "$name selected the wrong component set"
        return 1
    fi
}

check_cache_fixture_state_begin() {
    local state="$1"
    local context="$2"

    printf '%s\n' "$CHECK_CACHE_MAGIC" >"$state"
    printf 'context\t%s\n' "$context" >>"$state"
}

check_cache_fixture_state_record() {
    local state="$1"
    local owner="$2"
    local path="$3"
    local payload="$4"
    local identity

    identity="$(printf '%s' "$payload" | git hash-object --stdin)" || {
        return 1
    }
    printf 'file\t%s\t81a4\t%s\t%s\t%s\n' \
        "$owner" "${#payload}" "$identity" "$path" >>"$state"
}

check_cache_assert_selection() {
    local name="$1"
    local old="$2"
    local current="$3"
    local graph="$4"
    local expected_mode="$5"
    local expected_plan="$6"
    local expected_changed="$7"
    local case_directory="$8"

    mkdir -p -- "$case_directory" || return 1
    check_cache_validate_state "$old" fixture-context "$graph" || return 1
    check_cache_validate_state \
        "$current" fixture-context "$graph" || return 1
    check_cache_select_states \
        "$old" "$current" "$graph" "$case_directory" || return 1
    if [[ "$(sed -n '1p' "$case_directory/mode")" != "$expected_mode" ]] \
        || ! cmp -s -- "$case_directory/plan.tsv" "$expected_plan" \
        || ! cmp -s -- \
            "$case_directory/changed-paths.z" "$expected_changed"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            "$name produced the wrong mode, plan, or changed paths"
        return 1
    fi
}

check_cache_run_metadata_target_fixture() {
    local directory="$1"
    local workspace="$directory/metadata-workspace"
    local target="$directory/metadata-target"
    local output="$directory/metadata-fixture.json"

    mkdir -p -- "$workspace/src" || return 1
    mkdir -m 700 -- "$target" || return 1
    printf '%s\n' \
        '[package]' \
        'name = "reovim-check-cache-metadata-fixture"' \
        'version = "0.0.0"' \
        'edition = "2024"' \
        '' \
        '[workspace]' >"$workspace/Cargo.toml"
    printf '%s\n' \
        '# This file is automatically @generated by Cargo.' \
        '# It is not intended for manual editing.' \
        'version = 4' \
        '' \
        '[[package]]' \
        'name = "reovim-check-cache-metadata-fixture"' \
        'version = "0.0.0"' >"$workspace/Cargo.lock"
    printf '%s\n' '#![forbid(unsafe_code)]' >"$workspace/src/lib.rs"
    CARGO_TARGET_DIR="$workspace/target" \
        check_cache_capture_metadata \
            "$output" "$target" \
            --manifest-path "$workspace/Cargo.toml" || return 1
    check_cache_validate_metadata_target \
        "$output" "$target" || return 1
    if [[ -e "$workspace/target" || -L "$workspace/target" ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'Cargo metadata wrote into its workspace target'
        return 1
    fi
}

check_cache_run_graph_fixtures() {
    local directory="$1"
    local graph="$directory/fixture-graph.tsv"
    local direct="$directory/fixture-direct"
    local selected="$directory/fixture-selected"
    local expected="$directory/fixture-expected"

    {
        printf 'workspace\tfixture\tCargo.toml\tCargo.lock\n'
        printf 'package\tfixture:kernel\tfixture\tkernel\tproduct/kernel\n'
        printf 'package\tfixture:tui\tfixture\ttui\tproduct/tui\n'
        printf 'package\tfixture:cli\tfixture\tcli\tproduct/cli\n'
        printf 'package\tfixture:web\tfixture\tweb\tproduct/web\n'
        printf 'edge\tfixture:kernel\tfixture:tui\n'
        printf 'edge\tfixture:kernel\tfixture:cli\n'
        printf 'edge\tfixture:kernel\tfixture:web\n'
    } | LC_ALL=C sort -u >"$graph"
    check_cache_validate_graph "$graph" || return 1
    if [[ "$(check_cache_owner_for_path ci.json "$graph")" != global ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'toolchain window did not invalidate global coverage'
        return 1
    fi

    printf '%s\n' fixture:tui >"$direct"
    check_cache_expand_components "$graph" "$direct" "$selected"
    printf '%s\n' fixture:tui >"$expected"
    check_cache_assert_set tui-only "$selected" "$expected" || return 1

    printf '%s\n' fixture:kernel >"$direct"
    check_cache_expand_components "$graph" "$direct" "$selected"
    printf '%s\n' \
        fixture:cli fixture:kernel fixture:tui fixture:web >"$expected"
    check_cache_assert_set kernel-clients "$selected" "$expected" || return 1
}

check_cache_run_transition_fixtures() {
    local directory="$1"
    local graph="$directory/fixture-graph.tsv"
    local old="$directory/transition-old.v1"
    local current="$directory/transition-current.v1"
    local plan="$directory/transition-plan.tsv"
    local changed="$directory/transition-changed.z"
    local cases="$directory/transition-cases"

    check_cache_fixture_state_begin "$old" fixture-context
    check_cache_fixture_state_record \
        "$old" package:fixture:tui product/tui/src/lib.rs before
    check_cache_seal_file "$old"
    cp -- "$old" "$current"
    : >"$plan"
    : >"$changed"
    check_cache_assert_selection \
        hit "$old" "$current" "$graph" hit "$plan" "$changed" \
        "$cases/hit" || return 1

    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_fixture_state_record \
        "$current" package:fixture:tui product/tui/src/lib.rs after
    check_cache_seal_file "$current"
    printf 'direct\tfixture:tui\n' >"$plan"
    printf 'product/tui/src/lib.rs\0' >"$changed"
    check_cache_assert_selection \
        tui-only "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/tui-only" || return 1

    check_cache_fixture_state_begin "$old" fixture-context
    check_cache_fixture_state_record \
        "$old" package:fixture:kernel product/kernel/src/lib.rs before
    check_cache_seal_file "$old"
    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_fixture_state_record \
        "$current" package:fixture:kernel product/kernel/src/lib.rs after
    check_cache_seal_file "$current"
    {
        printf 'dependent\tfixture:cli\n'
        printf 'direct\tfixture:kernel\n'
        printf 'dependent\tfixture:tui\n'
        printf 'dependent\tfixture:web\n'
    } >"$plan"
    printf 'product/kernel/src/lib.rs\0' >"$changed"
    check_cache_assert_selection \
        kernel-clients "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/kernel-clients" || return 1

    check_cache_fixture_state_begin "$old" fixture-context
    check_cache_seal_file "$old"
    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_fixture_state_record \
        "$current" package:fixture:tui product/tui/src/new.rs added
    check_cache_seal_file "$current"
    printf 'direct\tfixture:tui\n' >"$plan"
    printf 'product/tui/src/new.rs\0' >"$changed"
    check_cache_assert_selection \
        addition "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/addition" || return 1

    cp -- "$current" "$old"
    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_seal_file "$current"
    : >"$changed"
    check_cache_assert_selection \
        deletion "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/deletion" || return 1

    check_cache_fixture_state_begin "$old" fixture-context
    check_cache_fixture_state_record \
        "$old" package:fixture:tui product/tui/src/old.rs same
    check_cache_seal_file "$old"
    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_fixture_state_record \
        "$current" package:fixture:tui product/tui/src/new.rs same
    check_cache_seal_file "$current"
    printf 'product/tui/src/new.rs\0' >"$changed"
    check_cache_assert_selection \
        rename "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/rename" || return 1

    check_cache_fixture_state_begin "$current" fixture-context
    check_cache_fixture_state_record \
        "$current" package:fixture:cli product/cli/src/new.rs same
    check_cache_seal_file "$current"
    {
        printf 'direct\tfixture:cli\n'
        printf 'direct\tfixture:tui\n'
    } >"$plan"
    printf 'product/cli/src/new.rs\0' >"$changed"
    check_cache_assert_selection \
        cross-owner-rename "$old" "$current" "$graph" selected \
        "$plan" "$changed" "$cases/cross-owner-rename" || return 1

    if check_cache_owner_for_path unknown/input.rs "$graph" >/dev/null; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'unknown ownership unexpectedly resolved'
        return 1
    fi
}

check_cache_run_failed_snapshot_fixture() (
    local directory="$1/failed-snapshot"
    local fixture_graph="$1/fixture-graph.tsv"

    mkdir -p -- "$directory" || return 1
    check_cache_context() { printf '%s\n' fixture-context; }
    check_cache_load_graph() {
        cp -- "$fixture_graph" "$directory/graph.tsv" || return 1
        printf '%s\n' "$directory/graph.tsv"
    }
    # The fixture graph cannot own this checkout's actual provider files.
    check_cache_prepare "$directory" yes || return 1
    if [[ "$(<"$directory/mode")" != full \
        || "$(<"$directory/publishable")" != no \
        || -e "$directory/pre-state.v1" \
        || -e "$directory/pre-observation.v1" ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'failed snapshot poisoned the full fallback'
        return 1
    fi
    check_cache_run_state_fixtures "$directory"
)

check_cache_run_state_fixtures() {
    local directory="$1"
    local state="$directory/pre-state.v1"
    local graph="$directory/graph.tsv"
    local malformed="$directory/malformed-state.v1"
    local truncated="$directory/truncated-state.v1"
    local context other_context

    [[ -f "$state" && -f "$graph" ]] || return 0
    context="$(sed -n '1p' "$directory/context")"
    check_cache_validate_state "$state" "$context" "$graph" || return 1

    sed '$d' "$state" >"$malformed" || return 1
    sed -n '3p' "$state" >>"$malformed" || return 1
    check_cache_seal_file "$malformed" || return 1
    if check_cache_validate_state "$malformed" "$context" "$graph"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'duplicate or unsorted state unexpectedly passed'
        return 1
    fi

    sed '3d' "$state" >"$truncated" || return 1
    if check_cache_validate_state "$truncated" "$context" "$graph"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'truncated sealed state unexpectedly passed'
        return 1
    fi

    other_context="$(printf '%s' 'other tool context' | \
        git hash-object --stdin)" || return 1
    if check_cache_validate_state "$state" "$other_context" "$graph"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'another tool context unexpectedly reused state'
        return 1
    fi
}

check_cache_run_context_fixtures_controlled() {
    local directory="$1"
    local base alternate current_profile cargo_home rg_config wrapper linker
    local wrapper_before wrapper_after linker_before linker_after

    mkdir -p -- "$directory/context-base" "$directory/context-profile"
    base="$(check_cache_context "$directory/context-base")" || return 1
    current_profile="${CARGO_PROFILE_RELEASE_OPT_LEVEL-}"
    alternate=0
    [[ "$current_profile" != 0 ]] || alternate=1
    alternate="$(
        export CARGO_PROFILE_RELEASE_OPT_LEVEL="$alternate"
        check_cache_context "$directory/context-profile"
    )" || return 1
    if [[ "$base" == "$alternate" ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'Cargo profile override did not invalidate context'
        return 1
    fi

    cargo_home="$directory/context-cargo-home"
    mkdir -p -- "$cargo_home" "$directory/context-config-before" \
        "$directory/context-config-after"
    printf '[build]\nincremental = true\n' >"$cargo_home/config.toml"
    if (
        export CARGO_HOME="$cargo_home"
        check_cache_context "$directory/context-config-before"
    ) >/dev/null 2>&1; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'unsupported Cargo configuration entered cache context'
        return 1
    fi
    mv -- "$cargo_home/config.toml" "$cargo_home/config"
    if (
        export CARGO_HOME="$cargo_home"
        check_cache_context "$directory/context-config-after"
    ) >/dev/null 2>&1; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'legacy Cargo configuration entered cache context'
        return 1
    fi

    rg_config="$directory/context-ripgrep.conf"
    printf '%s\n' '--hidden' >"$rg_config"
    mkdir -p -- "$directory/context-ripgrep"
    if (
        export RIPGREP_CONFIG_PATH="$rg_config"
        check_cache_context "$directory/context-ripgrep"
    ) >/dev/null 2>&1; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'ripgrep configuration entered cache context'
        return 1
    fi

    wrapper="$directory/context-wrapper"
    printf '#!/usr/bin/env sh\nexit 0\n' >"$wrapper"
    chmod 700 -- "$wrapper"
    mkdir -p -- "$directory/context-wrapper-before" \
        "$directory/context-wrapper-after"
    wrapper_before="$(
        export RUSTC_WRAPPER="$wrapper"
        check_cache_context "$directory/context-wrapper-before"
    )" || return 1
    printf '#!/usr/bin/env sh\nexit 1\n' >"$wrapper"
    wrapper_after="$(
        export RUSTC_WRAPPER="$wrapper"
        check_cache_context "$directory/context-wrapper-after"
    )" || return 1
    if [[ "$wrapper_before" == "$wrapper_after" ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'wrapper implementation change did not invalidate context'
        return 1
    fi

    linker="$directory/context-linker"
    printf '#!/usr/bin/env sh\nexit 0\n' >"$linker"
    chmod 700 -- "$linker"
    mkdir -p -- "$directory/context-linker-before" \
        "$directory/context-linker-after"
    linker_before="$(
        export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$linker"
        check_cache_context "$directory/context-linker-before"
    )" || return 1
    printf '#!/usr/bin/env sh\nexit 1\n' >"$linker"
    linker_after="$(
        export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$linker"
        check_cache_context "$directory/context-linker-after"
    )" || return 1
    if [[ "$linker_before" == "$linker_after" ]]; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'target linker implementation did not invalidate context'
        return 1
    fi
}

check_cache_run_context_fixtures() (
    local directory="$1"
    local names="$directory/context-fixture-environment"
    local probe="$directory/context-fixture-probe"
    local variable
    local original_home="${HOME-}"
    local clean_home="$directory/context-fixture-home"
    local clean_path="$PATH"
    local clean_rustup_home="${RUSTUP_HOME-}"
    local clean_toolchain="${RUSTUP_TOOLCHAIN-}"

    if [[ -z "$clean_rustup_home" \
        && -n "$original_home" \
        && -d "$original_home/.rustup" ]]; then
        clean_rustup_home="$original_home/.rustup"
    fi
    mkdir -p -- "$clean_home"
    compgen -e | LC_ALL=C sort -u >"$names" || return 1
    while IFS= read -r variable; do
        check_cache_environment_affects_build "$variable" || continue
        case "$variable" in
            BASHOPTS | SHELLOPTS) continue ;;
        esac
        unset "$variable"
    done <"$names"
    export HOME="$clean_home"
    export PATH="$clean_path"
    export LC_ALL=C
    export GIT_CONFIG_GLOBAL=/dev/null
    export GIT_CONFIG_NOSYSTEM=1
    unset RIPGREP_CONFIG_PATH
    if [[ -n "$clean_rustup_home" ]]; then
        export RUSTUP_HOME="$clean_rustup_home"
    fi
    if [[ -n "$clean_toolchain" ]]; then
        export RUSTUP_TOOLCHAIN="$clean_toolchain"
    fi
    if check_cache_repository_cargo_configuration_present; then
        return 0
    fi
    mkdir -p -- "$probe"
    if ! check_cache_context "$probe" >/dev/null 2>&1; then
        return 0
    fi
    check_cache_run_context_fixtures_controlled "$directory"
)

check_cache_run_publication_fixtures() {
    local directory="$1"
    local atomic="$directory/atomic-publication"
    local state="$atomic/state.v1"
    local sentinel="$directory/publication-sentinel"
    local drift="$directory/publication-drift"
    local observation="$directory/publication-observation"
    local link_directory="$directory/atomic-publication-link"
    local directory_target="$directory/atomic-publication-directory"
    local mode

    [[ -f "$directory/pre-state.v1" \
        && -f "$directory/pre-observation.v1" ]] || return 0
    mkdir -p -- "$atomic"
    printf '%s\n' sentinel >"$sentinel"
    cp -- "$sentinel" "$state"
    if check_cache_atomic_replace \
        "$atomic" "$state" "$directory/absent-source" 2>/dev/null; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'failed atomic refresh unexpectedly succeeded'
        return 1
    fi
    cmp -s -- "$state" "$sentinel" || {
        check_cache_issue CHECK_CACHE_FIXTURE \
            'failed atomic refresh changed prior state'
        return 1
    }

    check_cache_atomic_replace \
        "$atomic" "$state" "$directory/pre-state.v1" || return 1
    cmp -s -- "$state" "$directory/pre-state.v1" || return 1
    mode="$(stat -c '%a' -- "$state")" || return 1
    [[ "$mode" == 600 ]] || return 1
    check_cache_inputs_stable \
        "$directory" "$directory/pre-state.v1" \
        "$directory/pre-observation.v1" || return 1

    cp -- "$directory/pre-state.v1" "$drift"
    printf '\n' >>"$drift"
    if check_cache_inputs_stable \
        "$directory" "$drift" "$directory/pre-observation.v1" \
        2>/dev/null; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'content drift unexpectedly passed'
        return 1
    fi
    cp -- "$directory/pre-observation.v1" "$observation"
    printf '\n' >>"$observation"
    if check_cache_inputs_stable \
        "$directory" "$directory/pre-state.v1" "$observation" \
        2>/dev/null; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'metadata ABA witness unexpectedly passed'
        return 1
    fi
    cmp -s -- "$state" "$directory/pre-state.v1" || return 1

    mkdir -p -- "$link_directory" "$directory_target/state.v1"
    ln -s -- "$sentinel" "$link_directory/state.v1" || return 1
    if check_cache_atomic_replace \
        "$link_directory" "$link_directory/state.v1" \
        "$directory/pre-state.v1"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'symlink state destination unexpectedly passed'
        return 1
    fi
    if check_cache_atomic_replace \
        "$directory_target" "$directory_target/state.v1" \
        "$directory/pre-state.v1"; then
        check_cache_issue CHECK_CACHE_FIXTURE \
            'directory state destination unexpectedly passed'
        return 1
    fi
}

check_cache_run_fixtures() {
    local directory="$1"

    check_cache_run_metadata_target_fixture "$directory" || return 1
    check_cache_run_graph_fixtures "$directory" || return 1
    check_cache_run_transition_fixtures "$directory" || return 1
    check_cache_run_failed_snapshot_fixture "$directory" || return 1
    check_cache_run_state_fixtures "$directory" || return 1
    check_cache_run_context_fixtures "$directory" || return 1
    check_cache_run_publication_fixtures "$directory"
}

check_cache_usage() {
    cat <<'EOF'
Usage: ./scripts/check-cache.sh COMMAND ARGUMENTS

Internal checked commands:
  prepare DIRECTORY {yes|no}
  publish DIRECTORY
  fixtures DIRECTORY
EOF
}

check_cache_safe_argument_directory() {
    local directory="$1"

    [[ "$directory" == /* && -d "$directory" \
        && ! -L "$directory" && -O "$directory" ]]
}

case "${1:-}" in
    prepare)
        [[ $# == 3 ]] || {
            check_cache_usage >&2
            exit 2
        }
        check_cache_safe_argument_directory "$2" || exit 2
        [[ "$3" == yes || "$3" == no ]] || exit 2
        check_cache_prepare "$2" "$3"
        ;;
    publish)
        [[ $# == 2 ]] || {
            check_cache_usage >&2
            exit 2
        }
        check_cache_safe_argument_directory "$2" || exit 2
        check_cache_publish "$2"
        ;;
    fixtures)
        [[ $# == 2 ]] || {
            check_cache_usage >&2
            exit 2
        }
        check_cache_safe_argument_directory "$2" || exit 2
        check_cache_run_fixtures "$2"
        ;;
    -h | --help)
        [[ $# == 1 ]] || exit 2
        check_cache_usage
        ;;
    *)
        check_cache_usage >&2
        exit 2
        ;;
esac
