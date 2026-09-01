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
    .github/ACTIONS-REVIEW.md
    .github/PULL_REQUEST_TEMPLATE.md
    perf/README.md
)
while IFS= read -r -d '' file; do
    markdown_files+=("$file")
done < <(find Documentation -type f -name '*.md' -print0 | sort -z)

active_files=(README.md GOVERNANCE.md CONTRIBUTING.md AGENTS.md)
active_files+=(.github/ACTIONS-REVIEW.md .github/PULL_REQUEST_TEMPLATE.md)
active_files+=(perf/README.md)
while IFS= read -r -d '' file; do
    active_files+=("$file")
done < <(find Documentation -type f -name '*.md' \
    ! -path 'Documentation/heritage/*' -print0 | sort -z)

fail() {
    echo "check-documentation: $*" >&2
    exit 1
}

check_ci_action_admission() {
    local workflow="$1"
    local review="$2"
    local checkout_sha=3d3c42e5aac5ba805825da76410c181273ba90b1
    local toolchain_sha=6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
    local workflow_digest=54820cd83daee64e4a65593554cedc7714444adab4ef24fa0ee37aebab71c45f
    local -a actual_uses expected_uses

    [[ "$(sha256sum -- "$workflow")" == "$workflow_digest  $workflow" ]] || return 1

    mapfile -t actual_uses < <(
        sed -nE 's/^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]+([^[:space:]#]+).*/\2/p' "$workflow"
    )
    expected_uses=()
    expected_uses+=("actions/checkout@$checkout_sha")
    expected_uses+=("dtolnay/rust-toolchain@$toolchain_sha")
    [[ "${actual_uses[*]}" == "${expected_uses[*]}" ]] || return 1

    [[ "$(rg -c '^permissions:$' "$workflow")" == 1 ]] || return 1
    [[ "$(rg -c '^  contents: read$' "$workflow")" == 1 ]] || return 1
    [[ "$(rg -c '^[[:space:]]*permissions:' "$workflow")" == 1 ]] || return 1
    perl -0777 -e '
        my ($checkout, $toolchain, $file) = @ARGV;
        open my $fh, "<", $file or die "$file: $!\n";
        local $/;
        my $text = <$fh>;
        my $checkout_ok = $text =~ /^      - uses: actions\/checkout\@\Q$checkout\E # v7\.0\.1\n        with:\n          persist-credentials: false\n(?=      - )/m;
        my $toolchain_ok = $text =~ /^      - uses: dtolnay\/rust-toolchain\@\Q$toolchain\E # v1\/current master\n        with:\n          toolchain: \$\{\{ matrix\.toolchain \}\}\n          components: clippy, rustfmt\n(?=      - )/m;
        exit(($checkout_ok && $toolchain_ok) ? 0 : 1);
      ' "$checkout_sha" "$toolchain_sha" "$workflow" || return 1

    for required in \
        'Upstream: <https://github.com/actions/checkout>' \
        "Pin: \`$checkout_sha\` (\`v7.0.1\`)" \
        'Upstream: <https://github.com/dtolnay/rust-toolchain>' \
        "Pin: \`$toolchain_sha\` (\`v1\`, reviewed current master)" \
        "Workflow digest: \`sha256:$workflow_digest\`" \
        'They are CI tooling, not Cargo dependencies,' \
        'Product admission, release qualification, or evidence publication.'; do
        rg -Fq -- "$required" "$review" || return 1
    done
}

ci_workflow=.github/workflows/ci.yml
ci_action_review=.github/ACTIONS-REVIEW.md
check_ci_action_admission "$ci_workflow" "$ci_action_review" || \
    fail "CI action pins, authority, and retained review are not exact"

ci_fixture="$(mktemp -d)"
trap 'rm -rf "$ci_fixture"' EXIT
sed 's/3d3c42e5aac5ba805825da76410c181273ba90b1/0000000000000000000000000000000000000000/' \
    "$ci_workflow" > "$ci_fixture/changed-workflow.yml"
if check_ci_action_admission "$ci_fixture/changed-workflow.yml" "$ci_action_review"; then
    fail "CI action guard accepted a workflow pin mutant"
fi
sed 's/6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772/0000000000000000000000000000000000000000/' \
    "$ci_action_review" > "$ci_fixture/changed-review.md"
if check_ci_action_admission "$ci_workflow" "$ci_fixture/changed-review.md"; then
    fail "CI action guard accepted a review pin mutant"
fi
sed '/dtolnay\/rust-toolchain@/a\      - uses: example/unreviewed@1111111111111111111111111111111111111111' \
    "$ci_workflow" > "$ci_fixture/extra-action.yml"
if check_ci_action_admission "$ci_fixture/extra-action.yml" "$ci_action_review"; then
    fail "CI action guard accepted an unreviewed action mutant"
fi
sed '/^jobs:$/a\  unreviewed-reusable:\n    uses: example/unreviewed/.github/workflows/ci.yml@1111111111111111111111111111111111111111' \
    "$ci_workflow" > "$ci_fixture/reusable-workflow.yml"
if check_ci_action_admission "$ci_fixture/reusable-workflow.yml" "$ci_action_review"; then
    fail "CI action guard accepted an unreviewed reusable-workflow mutant"
fi
sed 's/uses:/uses :/' "$ci_workflow" > "$ci_fixture/spaced-uses.yml"
if check_ci_action_admission "$ci_fixture/spaced-uses.yml" "$ci_action_review"; then
    fail "CI action guard accepted a noncanonical uses-key mutant"
fi
sed '/dtolnay\/rust-toolchain@/i\      - { uses: example/unreviewed@1111111111111111111111111111111111111111 }' \
    "$ci_workflow" > "$ci_fixture/flow-uses.yml"
if check_ci_action_admission "$ci_fixture/flow-uses.yml" "$ci_action_review"; then
    fail "CI action guard accepted a flow-mapping uses mutant"
fi
rm -rf "$ci_fixture"
trap - EXIT

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
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md
    Documentation/01-Architecture/05-Client/README.md
    Documentation/03-Extensions/README.md
    Documentation/04-Performance/README.md
    Documentation/05-Editor-Event-Stream/README.md
    Documentation/06-Conformance/README.md
    Documentation/06-Conformance/01-Requirements-and-Evidence.md
    Documentation/06-Conformance/02-Rule-Matrix.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md
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
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/02-Descriptors-and-Dependency-Admission.md
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md
    Documentation/01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md
    Documentation/01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md
    Documentation/01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md
    Documentation/01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md
    Documentation/01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/02-Client-View-and-Presentation.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/03-External-Semantic-Protocols.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/04-End-to-End-Flow-and-Readiness.md
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/05-Fidelity-and-Conformance-Proof.md
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
    Documentation/06-Conformance/02-Rule-Matrix/01-Product-Authority-and-Layering.md
    Documentation/06-Conformance/02-Rule-Matrix/02-Safety-Bounds-Effects-and-Exposure.md
    Documentation/06-Conformance/02-Rule-Matrix/03-Resource-and-Editing-State.md
    Documentation/06-Conformance/02-Rule-Matrix/04-Meaning-Interaction-and-View.md
    Documentation/06-Conformance/02-Rule-Matrix/05-Execution-Lifecycle-and-Debugging.md
    Documentation/06-Conformance/02-Rule-Matrix/06-Clients-Synchronization-and-Rendering.md
    Documentation/06-Conformance/02-Rule-Matrix/07-Host-SDK-and-Extensions.md
    Documentation/06-Conformance/02-Rule-Matrix/08-Performance-Events-Migration-and-Qualification.md
    Documentation/06-Conformance/03-Evidence-Taxonomy-and-Authority.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/01-Policy-Artifacts-and-Source-Axes.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/02-Exact-Execution-Topology-and-Dependency-Evidence.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/03-Inventory-Includes-and-Evidence-Publication.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/04-Phase-B-Scope-and-Product-Gate.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/01-Versioned-Inputs-Closures-and-Supported-Profile.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/02-Prepared-Root-and-Bounded-Execution.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/03-Compiler-Units-Inputs-and-Correlation.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/04-Package-Membership-Evidence.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/05-Public-Surface-and-Consumer-Evidence.md
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/06-Claims-Unavailability-and-Verification.md
    Documentation/06-Conformance/05-Public-Reduction-and-Qualification.md
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
check_index_children Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/02-Descriptors-and-Dependency-Admission.md \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md
check_index_children Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/02-Client-View-and-Presentation.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/03-External-Semantic-Protocols.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/04-End-to-End-Flow-and-Readiness.md \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/05-Fidelity-and-Conformance-Proof.md
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
    Documentation/06-Conformance/02-Rule-Matrix.md \
    Documentation/06-Conformance/03-Evidence-Taxonomy-and-Authority.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md \
    Documentation/06-Conformance/05-Public-Reduction-and-Qualification.md
check_index_children Documentation/06-Conformance/01-Requirements-and-Evidence.md \
    Documentation/06-Conformance/02-Rule-Matrix.md \
    Documentation/06-Conformance/03-Evidence-Taxonomy-and-Authority.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md \
    Documentation/06-Conformance/05-Public-Reduction-and-Qualification.md
check_index_children Documentation/06-Conformance/02-Rule-Matrix.md \
    Documentation/06-Conformance/02-Rule-Matrix/01-Product-Authority-and-Layering.md \
    Documentation/06-Conformance/02-Rule-Matrix/02-Safety-Bounds-Effects-and-Exposure.md \
    Documentation/06-Conformance/02-Rule-Matrix/03-Resource-and-Editing-State.md \
    Documentation/06-Conformance/02-Rule-Matrix/04-Meaning-Interaction-and-View.md \
    Documentation/06-Conformance/02-Rule-Matrix/05-Execution-Lifecycle-and-Debugging.md \
    Documentation/06-Conformance/02-Rule-Matrix/06-Clients-Synchronization-and-Rendering.md \
    Documentation/06-Conformance/02-Rule-Matrix/07-Host-SDK-and-Extensions.md \
    Documentation/06-Conformance/02-Rule-Matrix/08-Performance-Events-Migration-and-Qualification.md
check_index_children Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/01-Policy-Artifacts-and-Source-Axes.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/02-Exact-Execution-Topology-and-Dependency-Evidence.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/03-Inventory-Includes-and-Evidence-Publication.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/04-Phase-B-Scope-and-Product-Gate.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md
check_index_children Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/01-Versioned-Inputs-Closures-and-Supported-Profile.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/02-Prepared-Root-and-Bounded-Execution.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/03-Compiler-Units-Inputs-and-Correlation.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/04-Package-Membership-Evidence.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/05-Public-Surface-and-Consumer-Evidence.md \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/06-Claims-Unavailability-and-Verification.md
check_index_children Documentation/07-Mission-Critical-Quality/README.md \
    Documentation/07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md \
    Documentation/07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md \
    Documentation/07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md

check_index_sections() {
    local index="$1"
    shift
    local expected actual
    expected="$(printf '## %s\n' "$@")"
    actual="$( { rg '^## ' "$index" || true; } )"
    if [[ "$actual" != "$expected" ]]; then
        fail "$index contains a detailed or unexpected section instead of navigation only"
    fi
}

check_index_sections \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md \
    'Reading order'
check_index_sections \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md \
    'Reading order'
check_index_sections \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md \
    'Authority and reading order'
check_index_sections \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md \
    'Authority and reading order'

check_index_digest() {
    local index="$1"
    local expected="$2"
    local actual
    actual="$(sha256sum -- "$index")"
    actual="${actual%% *}"
    [[ "$actual" == "$expected" ]] || \
        fail "$index changed outside its reviewed navigation-only form"
}

check_index_digest \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md \
    e6ad34ae8a299eb3a2d91c2df618a35f381eebf48ebddba1ebbeef44f8c8b1dd
check_index_digest \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md \
    bfebb820eee5fd6566eed7eb40e60da0a5efe40f9d0ae1bad3d71c4b0b997221
check_index_digest \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker.md \
    4d5a0b3adc58bc28f8dd3c338183f0f61dd6890b323de83dbc14e543387f20eb
check_index_digest \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md \
    fcc07d3f0b2025687041e1f6767c88ba7690e447fe27a85f53f1bfac87140a4d

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

authority_source_files=()
for file in "${expected_normative_files[@]}"; do
    case "$file" in
        Documentation/06-Conformance/02-Rule-Matrix*) continue ;;
        *) authority_source_files+=("$file") ;;
    esac
done

require_unique_owner_clause() {
    local file="$1"
    local clause="$2"
    if ! perl -e '
        use strict;
        use warnings;

        sub normalized {
            my ($text) = @_;
            $text =~ s/\s+/ /g;
            $text =~ s/^\s+|\s+$//g;
            return $text;
        }

        my ($owner, $clause, @files) = @ARGV;
        $clause = normalized($clause);
        my ($owner_count, $total) = (0, 0);
        for my $candidate (@files) {
            open my $fh, "<", $candidate or die "$candidate: $!\n";
            local $/;
            my $text = normalized(<$fh>);
            my $quoted = quotemeta($clause);
            my $count = () = $text =~ /$quoted/g;
            $owner_count = $count if $candidate eq $owner;
            $total += $count;
        }
        exit(($owner_count == 1 && $total == 1) ? 0 : 1);
      ' "$file" "$clause" "${authority_source_files[@]}"; then
        fail "focused authority clause must be complete and unique: $file: $clause"
    fi
}

layer_roles=Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md
layer_edges=Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md
classification_artifacts=Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/01-Policy-Artifacts-and-Source-Axes.md
product_proof_authority=Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/01-Versioned-Inputs-Closures-and-Supported-Profile.md
require_unique_owner_clause "$layer_edges" \
    'Product dependencies point inward without cycles. `RepositoryTooling` grants no product edge and enters no product shipping closure.'
require_unique_owner_clause "$layer_edges" \
    'Foundation and Kernel contracts contain no concrete environment, adapter, Domain, transport, Client, presentation, or extension mechanism.'
require_unique_owner_clause "$layer_roles" \
    'Resource Adapter, Host Adapter, External Protocol Binding, Protocol Adapter, Presentation Adapter, Extension Adapter, and Extension Engine Adapter are distinct roles. Selection or composition of one never merges it with another.'
require_unique_owner_clause "$classification_artifacts" \
    'Every evidence set carries an exact-revision Source Role Map generated from version-matched, schema-valid classification policy, exact workspace topology, entrypoint reviews, and dependency admissions. Phase-B v1 reports only dependency reachability for an all-`RepositoryTooling` topology and fails closed on Product or shipping input.'
require_unique_owner_clause "$product_proof_authority" \
    'Every Product shipping or validation closure is selected by version-matched, schema-valid Product-closure and execution-tool admissions and is proved by correlated compiler-reported Product source/dependencies, Cargo package membership where shipping, and external-crate public-consumer evidence. Unsupported mechanisms fail closed, and fixture capability never qualifies active Product.'

if rg -Fq 'roles do not merge' "$layer_edges"; then
    fail 'dependency graph duplicates the role-distinction authority'
fi
if rg -Fq 'owns its deliberately unproved Product boundary' "$classification_artifacts"; then
    fail 'classification authority delegates part of ARCH-006 to a second owner'
fi

rule_index=Documentation/06-Conformance/02-Rule-Matrix.md
rule_chapters=(
    Documentation/06-Conformance/02-Rule-Matrix/01-Product-Authority-and-Layering.md
    Documentation/06-Conformance/02-Rule-Matrix/02-Safety-Bounds-Effects-and-Exposure.md
    Documentation/06-Conformance/02-Rule-Matrix/03-Resource-and-Editing-State.md
    Documentation/06-Conformance/02-Rule-Matrix/04-Meaning-Interaction-and-View.md
    Documentation/06-Conformance/02-Rule-Matrix/05-Execution-Lifecycle-and-Debugging.md
    Documentation/06-Conformance/02-Rule-Matrix/06-Clients-Synchronization-and-Rendering.md
    Documentation/06-Conformance/02-Rule-Matrix/07-Host-SDK-and-Extensions.md
    Documentation/06-Conformance/02-Rule-Matrix/08-Performance-Events-Migration-and-Qualification.md
)

architecture_rule_chapter=Documentation/06-Conformance/02-Rule-Matrix/01-Product-Authority-and-Layering.md
meaning_rule_chapter=Documentation/06-Conformance/02-Rule-Matrix/04-Meaning-Interaction-and-View.md

check_rule_authority() {
    local chapter="$1"
    local id="$2"
    local owner="$3"
    local expected row
    expected="$(realpath --relative-to="$(dirname "$chapter")" "$owner")"
    row="$(rg -F -m1 "\`$id\`" "$chapter")"
    [[ -n "$row" ]] || fail "Rule Matrix row is missing: $id"
    [[ "$row" == *"]($expected)"* ]] || \
        fail "$id does not route to its focused authority: $expected"
}

check_rule_authority "$architecture_rule_chapter" ARCH-001 \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md
check_rule_authority "$architecture_rule_chapter" ARCH-002 \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md
check_rule_authority "$architecture_rule_chapter" ARCH-003 \
    Documentation/01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md
check_rule_authority "$architecture_rule_chapter" ARCH-006 \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/01-Policy-Artifacts-and-Source-Axes.md
check_rule_authority "$architecture_rule_chapter" ARCH-007 \
    Documentation/06-Conformance/04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof/01-Versioned-Inputs-Closures-and-Supported-Profile.md
check_rule_authority "$meaning_rule_chapter" INT-001 \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md
check_rule_authority "$meaning_rule_chapter" INT-002 \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md
check_rule_authority "$meaning_rule_chapter" VIEW-001 \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/02-Client-View-and-Presentation.md
check_rule_authority "$meaning_rule_chapter" PROTO-001 \
    Documentation/01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/03-External-Semantic-Protocols.md

rule_ids="$(awk -F'`' '/^\| `[A-Z]+-[0-9][0-9][0-9]`/ { print $2 }' "${rule_chapters[@]}")"
[[ -n "$rule_ids" ]] || fail "Rule Matrix contains no conformance identifiers"
[[ "$(printf '%s\n' "$rule_ids" | wc -l)" == 91 ]] || \
    fail "Rule Matrix must contain exactly 91 conformance identifiers"
duplicates="$(printf '%s\n' "$rule_ids" | sort | uniq -d)"
if [[ -n "$duplicates" ]]; then
    printf '%s\n' "$duplicates" >&2
    fail "duplicate conformance identifiers found"
fi

if ! perl -e '
    use strict;
    use warnings;

    my $index = shift @ARGV;
    my (%rules, %ledger);
    my $in_ledger = 0;
    my %evidence = map { $_ => 1 } qw(
        DOC TRACE STATIC SHAPE UNIT MODEL CONTRACT FAULT INTEG GOLDEN
        COMPAT PERF MEM IO EVENT SEC REVIEW QUAL
    );
    my @bad_authority;
    my @bad_evidence;

    for my $file (@ARGV) {
        open my $fh, "<", $file or die "$file: $!\n";
        while (my $line = <$fh>) {
            next unless $line =~ /^\| `([A-Z]+-[0-9]{3})` \| .*? \| \[[^\]]+\]\(([^)]+)\) \| `([^`]+(?:,[^`]*)?)` \|$/;
            my ($id, $authority, $codes) = ($1, $2, $3);
            ++$rules{$id};
            push @bad_authority, "$id:$authority"
                if $authority =~ m{^(?:[A-Za-z][A-Za-z0-9+.-]*:|/|#)}
                    || $authority =~ /#/;
            for my $code (split /,/, $codes) {
                push @bad_evidence, "$id:$code" unless $evidence{$code};
            }
        }
        close $fh;
    }

    open my $fh, "<", $index or die "$index: $!\n";
    while (my $line = <$fh>) {
        if ($line =~ /^## Applicability and responsibility ledger/) {
            $in_ledger = 1;
            next;
        }
        $in_ledger = 0 if $in_ledger && $line =~ /^## /;
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
    if (@missing || @duplicate || @extra || @bad_authority || @bad_evidence) {
        print STDERR "missing applicability: @missing\n" if @missing;
        print STDERR "non-single applicability: @duplicate\n" if @duplicate;
        print STDERR "unknown applicability IDs: @extra\n" if @extra;
        print STDERR "non-local or fragment authority links: @bad_authority\n"
            if @bad_authority;
        print STDERR "unknown evidence codes: @bad_evidence\n" if @bad_evidence;
        exit 1;
    }
  ' "$rule_index" "${rule_chapters[@]}"; then
    fail "Rule Matrix applicability, responsibility, or taxonomy validation failed"
fi

rg -Fq '`reovim-rule-matrix/v1`' "$rule_index" || \
    fail "Rule Matrix schema version is missing"
for taxonomy_file in \
    "$rule_index" \
    Documentation/06-Conformance/03-Evidence-Taxonomy-and-Authority.md; do
    rg -Fq '`reovim-evidence/v1`' "$taxonomy_file" || \
        fail "evidence taxonomy version is missing from $taxonomy_file"
done

echo "check-documentation: all checks passed (${#markdown_files[@]} files)"
