#!/usr/bin/env bash
# Structural checks for Reovim's normative and public documentation.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

markdown_files=(
    README.md
    GOVERNANCE.md
    CONTRIBUTING.md
    AGENTS.md
    .github/PULL_REQUEST_TEMPLATE.md
    perf/README.md
)
while IFS= read -r -d '' file; do
    markdown_files+=("$file")
done < <(find Documentation -type f -name '*.md' -print0 | sort -z)

active_files=(README.md GOVERNANCE.md CONTRIBUTING.md AGENTS.md)
active_files+=(.github/PULL_REQUEST_TEMPLATE.md)
active_files+=(perf/README.md)
while IFS= read -r -d '' file; do
    active_files+=("$file")
done < <(find Documentation -type f -name '*.md' \
    ! -path 'Documentation/heritage/*' -print0 | sort -z)

fail() {
    echo "check-documentation: $*" >&2
    exit 1
}

if rg -n '[[:blank:]]+$' "${markdown_files[@]}"; then
    fail "trailing whitespace found"
fi

for file in "${markdown_files[@]}"; do
    if ! awk '
        /^[[:space:]]*```/ { fences += 1 }
        END { exit(fences % 2) }
    ' "$file"; then
        fail "unbalanced fenced block in $file"
    fi
done

if ! perl -MFile::Spec -e '
    use strict;
    use warnings;

    my @bad;
    for my $file (@ARGV) {
        open my $fh, "<", $file or die "$file: $!\n";
        local $/;
        my $text = <$fh>;
        while ($text =~ /\[[^\]]*\]\(([^)]+)\)/g) {
            my $target = $1;
            my $prefix = substr($text, 0, $-[0]);
            my $line_number = 1 + ($prefix =~ tr/\n//);
            $target =~ s/^\s+|\s+$//g;
            next if $target =~ m{^(?:https?://|mailto:|#)};
            $target =~ s/^<(.*)>$/$1/;
            $target =~ s/#.*$//;
            next if $target eq "";

            my (undef, $directory, undef) =
                File::Spec->splitpath($file);
            my $resolved = File::Spec->rel2abs(
                $target,
                $directory || ".",
            );
            push @bad, "$file:$line_number: $target"
                unless -e $resolved;
        }
    }

    if (@bad) {
        print STDERR "broken local Markdown links:\n", join("\n", @bad), "\n";
        exit 1;
    }
  ' "${markdown_files[@]}"; then
    fail "local link validation failed"
fi

obsolete_paths=(
    Documentation/02-Process
    Documentation/03-State
    Documentation/04-Domain-Substrate
    Documentation/05-View
    Documentation/06-ABI
    Documentation/07-Surfaces
    Documentation/08-Client
    Documentation/09-Conformance
    Documentation/10-Development
    Documentation/debt
    Documentation/future
)
for path in "${obsolete_paths[@]}"; do
    [[ ! -e "$path" ]] || fail "obsolete normative path remains: $path"
done

retired_monoliths=(
    Documentation/01-Architecture/01-Layer-Model.md
    Documentation/01-Architecture/02-Resource-Buffer.md
    Documentation/01-Architecture/03-Domain-Representation-Projection-View.md
    Documentation/01-Architecture/04-Execution-and-Concurrency.md
    Documentation/01-Architecture/05-Client-CLM-CCM-and-Rendering.md
    Documentation/03-Extension-Contracts.md
    Documentation/04-Performance.md
    Documentation/05-Editor-Event-Stream.md
    Documentation/07-Mission-Critical-Quality.md
)
for path in "${retired_monoliths[@]}"; do
    [[ ! -e "$path" ]] || fail "retired normative monolith remains: $path"
done

contract_indexes=(
    Documentation/00-Concept.md
    Documentation/01-Architecture/README.md
    Documentation/01-Architecture/05-Client/README.md
    Documentation/03-Extensions/README.md
    Documentation/04-Performance/README.md
    Documentation/05-Editor-Event-Stream/README.md
    Documentation/06-Conformance/README.md
    Documentation/07-Mission-Critical-Quality/README.md
)
for path in "${contract_indexes[@]}"; do
    [[ -f "$path" ]] || fail "normative contract index is missing: $path"
done

expected_normative_files=(
    Documentation/00-Concept.md
    Documentation/00-Concept/01-Product-System-and-Kernel.md
    Documentation/00-Concept/02-State-Semantics-Clients-and-Extensions.md
    Documentation/00-Concept/03-Quality-Authority-and-v016-Proof.md
    Documentation/00-Vocabulary.md
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md
    Documentation/01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md
    Documentation/01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md
    Documentation/01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md
    Documentation/01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md
    Documentation/01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md
    Documentation/01-Architecture/04-Execution/01-Execution-Laws-and-Operation-Classes.md
    Documentation/01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md
    Documentation/01-Architecture/04-Execution/03-Runtime-Execution-Facility.md
    Documentation/01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md
    Documentation/01-Architecture/04-Execution/05-Causal-Debugging-and-Evidence.md
    Documentation/01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md
    Documentation/01-Architecture/05-Client/02-Rendering-Capabilities.md
    Documentation/01-Architecture/05-Client/03-Extension-Modules-and-CCM.md
    Documentation/01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md
    Documentation/01-Architecture/05-Client/README.md
    Documentation/01-Architecture/README.md
    Documentation/02-Host-API.md
    Documentation/03-Extensions/01-Packages-Contribution-Points-and-Registry.md
    Documentation/03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md
    Documentation/03-Extensions/03-Adapters-Execution-and-Lifecycle.md
    Documentation/03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md
    Documentation/03-Extensions/README.md
    Documentation/04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md
    Documentation/04-Performance/02-Scenarios-Bundles-and-Traces.md
    Documentation/04-Performance/03-Path-Overhead-and-Regression-Evidence.md
    Documentation/04-Performance/README.md
    Documentation/05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md
    Documentation/05-Editor-Event-Stream/02-Scope-Failure-and-Conformance.md
    Documentation/05-Editor-Event-Stream/README.md
    Documentation/05-Migration/01-Selective-Migration.md
    Documentation/06-Conformance/01-Requirements-and-Evidence.md
    Documentation/06-Conformance/02-Rule-Matrix.md
    Documentation/06-Conformance/README.md
    Documentation/07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md
    Documentation/07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md
    Documentation/07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md
    Documentation/07-Mission-Critical-Quality/README.md
    Documentation/README.md
)
mapfile -t actual_normative_files < <(
    find Documentation -type f -name '*.md' \
        ! -path 'Documentation/heritage/*' -print | sort
)
if [[ "${actual_normative_files[*]}" != "${expected_normative_files[*]}" ]]; then
    fail "active normative file topology differs from the contract inventory"
fi

check_index_children() {
    local index="$1"
    shift
    local child relative count
    for child in "$@"; do
        relative="$(realpath --relative-to="$(dirname "$index")" "$child")"
        count="$( { rg -Fo "]($relative)" "$index" || true; } | wc -l)"
        if [[ "$count" != 1 ]]; then
            fail "$index must route exactly once to $child (found $count)"
        fi
    done
}

check_index_children Documentation/README.md \
    Documentation/00-Concept.md \
    Documentation/00-Vocabulary.md \
    Documentation/01-Architecture/README.md \
    Documentation/02-Host-API.md \
    Documentation/03-Extensions/README.md \
    Documentation/04-Performance/README.md \
    Documentation/05-Editor-Event-Stream/README.md \
    Documentation/05-Migration/01-Selective-Migration.md \
    Documentation/06-Conformance/README.md \
    Documentation/07-Mission-Critical-Quality/README.md
check_index_children Documentation/00-Concept.md \
    Documentation/00-Concept/01-Product-System-and-Kernel.md \
    Documentation/00-Concept/02-State-Semantics-Clients-and-Extensions.md \
    Documentation/00-Concept/03-Quality-Authority-and-v016-Proof.md
check_index_children Documentation/01-Architecture/README.md \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md \
    Documentation/01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md \
    Documentation/01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md \
    Documentation/01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md \
    Documentation/01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md \
    Documentation/01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md \
    Documentation/01-Architecture/04-Execution/01-Execution-Laws-and-Operation-Classes.md \
    Documentation/01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md \
    Documentation/01-Architecture/04-Execution/03-Runtime-Execution-Facility.md \
    Documentation/01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md \
    Documentation/01-Architecture/04-Execution/05-Causal-Debugging-and-Evidence.md \
    Documentation/01-Architecture/05-Client/README.md
check_index_children Documentation/01-Architecture/05-Client/README.md \
    Documentation/01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md \
    Documentation/01-Architecture/05-Client/02-Rendering-Capabilities.md \
    Documentation/01-Architecture/05-Client/03-Extension-Modules-and-CCM.md \
    Documentation/01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md
check_index_children Documentation/03-Extensions/README.md \
    Documentation/03-Extensions/01-Packages-Contribution-Points-and-Registry.md \
    Documentation/03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md \
    Documentation/03-Extensions/03-Adapters-Execution-and-Lifecycle.md \
    Documentation/03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md
check_index_children Documentation/04-Performance/README.md \
    Documentation/04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md \
    Documentation/04-Performance/02-Scenarios-Bundles-and-Traces.md \
    Documentation/04-Performance/03-Path-Overhead-and-Regression-Evidence.md
check_index_children Documentation/05-Editor-Event-Stream/README.md \
    Documentation/05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md \
    Documentation/05-Editor-Event-Stream/02-Scope-Failure-and-Conformance.md
check_index_children Documentation/06-Conformance/README.md \
    Documentation/06-Conformance/01-Requirements-and-Evidence.md \
    Documentation/06-Conformance/02-Rule-Matrix.md
check_index_children Documentation/07-Mission-Critical-Quality/README.md \
    Documentation/07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md \
    Documentation/07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md \
    Documentation/07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md

for file in "${active_files[@]}"; do
    [[ "$file" == Documentation/* ]] || continue
    lines="$(wc -l < "$file")"
    if (( lines > 250 )); then
        fail "active normative chapter exceeds 250 lines: $file ($lines)"
    fi
done

if ! perl -e '
    use strict;
    use warnings;

    my @bad;
    for my $file (@ARGV) {
        open my $fh, "<", $file or die "$file: $!\n";
        local $/;
        my $text = <$fh>;
        while ($text =~ /\[[^\]]*\]\(([^)]+)\)/g) {
            my $target = $1;
            my $prefix = substr($text, 0, $-[0]);
            my $line_number = 1 + ($prefix =~ tr/\n//);
            $target =~ s/^\s+|\s+$//g;
            next if $target =~ m{^(?:https?://|mailto:)};
            push @bad, "$file:$line_number: $target"
                if $target =~ /#/;
        }
    }

    if (@bad) {
        print STDERR "local Markdown fragments:\n",
            join("\n", @bad), "\n";
        exit 1;
    }
  ' "${active_files[@]}"; then
    fail "local Markdown fragments are prohibited; link to a focused chapter"
fi

if rg -n \
    '02-Process/|03-State/|04-Domain-Substrate/|05-View/|06-ABI/|07-Surfaces/|08-Client/|09-Conformance/|10-Development/|Documentation/(debt|future)/' \
    "${active_files[@]}"; then
    fail "active documentation links or delegates to an obsolete authority"
fi

if rg -n '\b[0-9a-f]{40}\b|#[0-9]{3,}' Documentation \
    --glob '*.md' --glob '!Documentation/heritage/**'; then
    fail "normative Documentation contains a branch hash or tracker chronology"
fi

required_statements=(
    'Text is one Domain'
    'It is not an operating-system'
    'The mandatory v0.16 shipping Client Families are **TUI**, **CLI**, and **Web**.'
    'On selection, each of the seven rows enters'
    'input-to-present p99'
    '1 MiB'
)
for statement in "${required_statements[@]}"; do
    if ! rg -Fq "$statement" Documentation; then
        fail "required normative statement is missing: $statement"
    fi
done

rule_file=Documentation/06-Conformance/02-Rule-Matrix.md
rule_ids="$(awk -F'`' '/^\| `[A-Z]+-[0-9][0-9][0-9]`/ { print $2 }' "$rule_file")"
[[ -n "$rule_ids" ]] || fail "Rule Matrix contains no conformance identifiers"
duplicates="$(printf '%s\n' "$rule_ids" | sort | uniq -d)"
if [[ -n "$duplicates" ]]; then
    printf '%s\n' "$duplicates" >&2
    fail "duplicate conformance identifiers found"
fi

if ! perl -e '
    use strict;
    use warnings;

    my $file = shift @ARGV;
    open my $fh, "<", $file or die "$file: $!\n";
    my (%rules, %ledger);
    my $in_ledger = 0;
    my %evidence = map { $_ => 1 } qw(
        DOC TRACE STATIC SHAPE UNIT MODEL CONTRACT FAULT INTEG GOLDEN
        COMPAT PERF MEM IO EVENT SEC REVIEW QUAL
    );
    my @bad_evidence;

    while (my $line = <$fh>) {
        $in_ledger = 1 if $line =~ /^## Applicability and responsibility ledger/;
        $in_ledger = 0 if $line =~ /^## Product, authority, and layering/;

        if ($line =~ /^\| `([A-Z]+-[0-9]{3})` .* \| `([^`]+(?:,[^`]*)?)` \|$/) {
            my ($id, $codes) = ($1, $2);
            ++$rules{$id};
            for my $code (split /,/, $codes) {
                push @bad_evidence, "$id:$code" unless $evidence{$code};
            }
        }

        if ($in_ledger &&
            $line =~ /^\| ([A-Z]+)-([0-9]{3})(?:\.\.([0-9]{3}))? \|/) {
            my ($prefix, $first, $last) = ($1, $2, $3 // $2);
            die "descending ledger range $prefix-$first..$last\n"
                if $last < $first;
            for my $number ($first .. $last) {
                ++$ledger{sprintf "%s-%03d", $prefix, $number};
            }
        }
    }

    my @missing = sort grep { !$ledger{$_} } keys %rules;
    my @duplicate = sort grep { ($ledger{$_} // 0) != 1 } keys %rules;
    my @extra = sort grep { !$rules{$_} } keys %ledger;
    if (@missing || @duplicate || @extra || @bad_evidence) {
        print STDERR "missing applicability: @missing\n" if @missing;
        print STDERR "non-single applicability: @duplicate\n" if @duplicate;
        print STDERR "unknown applicability IDs: @extra\n" if @extra;
        print STDERR "unknown evidence codes: @bad_evidence\n" if @bad_evidence;
        exit 1;
    }
  ' "$rule_file"; then
    fail "Rule Matrix applicability, responsibility, or taxonomy validation failed"
fi

rg -Fq '`reovim-rule-matrix/v1`' "$rule_file" || \
    fail "Rule Matrix schema version is missing"
for taxonomy_file in \
    "$rule_file" \
    Documentation/06-Conformance/01-Requirements-and-Evidence.md; do
    rg -Fq '`reovim-evidence/v1`' "$taxonomy_file" || \
        fail "evidence taxonomy version is missing from $taxonomy_file"
done

echo "check-documentation: all checks passed (${#markdown_files[@]} files)"
