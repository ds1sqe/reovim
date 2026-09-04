#!/usr/bin/env bash
# Structural checks for Reovim's normative and public documentation.

set -euo pipefail

unset RIPGREP_CONFIG_PATH

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

readonly DOC=Documentation
readonly ARCH="$DOC/01-Architecture"
readonly LAYER="$ARCH/01-Layer"
readonly LAYER_ROLES="$LAYER/01-Roles-Descriptors-and-Dependencies"
readonly LAYER_PORTS="$LAYER/03-Ports-Source-Tree-and-Portability"
readonly BUFFER="$ARCH/02-Resource-and-Buffer"
readonly SEMANTICS="$ARCH/03-Semantics-and-View"
readonly INTERACTION_NAME=02-Interaction-Updates-Protocols-and-View
readonly INTERACTION="$SEMANTICS/$INTERACTION_NAME"
readonly HOST_API="$DOC/02-Host-API"
readonly BOOTSTRAP_HEALTH="$HOST_API/05-Bootstrap-Health"
readonly EXTENSIONS="$DOC/03-Extensions"
readonly PERFORMANCE="$DOC/04-Performance"
readonly EVENT_STREAM="$DOC/05-Editor-Event-Stream"
readonly CONFORMANCE="$DOC/06-Conformance"
readonly RULE_MATRIX="$CONFORMANCE/02-Rule-Matrix"
readonly CLASSIFIER_NAME=04-Source-Classification-and-Exact-Checker
readonly CLASSIFIER="$CONFORMANCE/$CLASSIFIER_NAME"
readonly PRODUCT_PROOF="$CLASSIFIER/05-Product-v2-Proof"
readonly PROOF_INPUT=01-Versioned-Inputs-Closures-and-Supported-Profile.md
readonly PRODUCT_INPUT="$PRODUCT_PROOF/$PROOF_INPUT"
readonly QUALITY="$DOC/07-Mission-Critical-Quality"

markdown_files=(
    README.md
    GOVERNANCE.md
    CONTRIBUTING.md
    AGENTS.md
    CODE_STYLE.md
    .github/ACTIONS-REVIEW.md
    .github/PULL_REQUEST_TEMPLATE.md
    perf/README.md
)
while IFS= read -r -d '' file; do
    markdown_files+=("$file")
done < <(find Documentation -type f -name '*.md' -print0 | sort -z)

active_files=(README.md GOVERNANCE.md CONTRIBUTING.md AGENTS.md CODE_STYLE.md)
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
    local workflow_digest
    # reovim-style: allow-long-line exact-token
    workflow_digest=b3f71d42639c90dbf5cb063e19ece01518ed51f8ce1f20b223d845cd36c772d6
    local -a actual_uses expected_uses

    if [[ "$(sha256sum -- "$workflow")" != \
        "$workflow_digest  $workflow" ]]; then
        return 1
    fi

    mapfile -t actual_uses < <(
        # reovim-style: allow-long-line searchable-string
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
        my $style_marker = "reovim-" .
            "style: allow-long-line exact-token";
        # reovim-style: allow-long-line searchable-string
        my $checkout_ok = $text =~ /^      - uses: actions\/checkout\@\Q$checkout\E # v7\.0\.1\n        with:\n          persist-credentials: false\n(?=      # \Q$style_marker\E\n      - )/m;
        # reovim-style: allow-long-line searchable-string
        my $toolchain_ok = $text =~ /^      # \Q$style_marker\E\n      - uses: dtolnay\/rust-toolchain\@\Q$toolchain\E # v1\/current master\n        with:\n          toolchain: \$\{\{ matrix\.toolchain \}\}\n          components: clippy, rustfmt\n(?=      - )/m;
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
# reovim-style: allow-long-line searchable-string
sed 's/3d3c42e5aac5ba805825da76410c181273ba90b1/0000000000000000000000000000000000000000/' \
    "$ci_workflow" > "$ci_fixture/changed-workflow.yml"
if check_ci_action_admission \
    "$ci_fixture/changed-workflow.yml" "$ci_action_review"; then
    fail "CI action guard accepted a workflow pin mutant"
fi
# reovim-style: allow-long-line searchable-string
sed 's/6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772/0000000000000000000000000000000000000000/' \
    "$ci_action_review" > "$ci_fixture/changed-review.md"
if check_ci_action_admission \
    "$ci_workflow" "$ci_fixture/changed-review.md"; then
    fail "CI action guard accepted a review pin mutant"
fi
# reovim-style: allow-long-line searchable-string
sed '/dtolnay\/rust-toolchain@/a\      - uses: example/unreviewed@1111111111111111111111111111111111111111' \
    "$ci_workflow" > "$ci_fixture/extra-action.yml"
if check_ci_action_admission \
    "$ci_fixture/extra-action.yml" "$ci_action_review"; then
    fail "CI action guard accepted an unreviewed action mutant"
fi
# reovim-style: allow-long-line searchable-string
sed '/^jobs:$/a\  unreviewed-reusable:\n    uses: example/unreviewed/.github/workflows/ci.yml@1111111111111111111111111111111111111111' \
    "$ci_workflow" > "$ci_fixture/reusable-workflow.yml"
if check_ci_action_admission \
    "$ci_fixture/reusable-workflow.yml" "$ci_action_review"; then
    fail "CI action guard accepted an unreviewed reusable-workflow mutant"
fi
sed 's/uses:/uses :/' "$ci_workflow" > "$ci_fixture/spaced-uses.yml"
if check_ci_action_admission \
    "$ci_fixture/spaced-uses.yml" "$ci_action_review"; then
    fail "CI action guard accepted a noncanonical uses-key mutant"
fi
# reovim-style: allow-long-line searchable-string
sed '/dtolnay\/rust-toolchain@/i\      - { uses: example/unreviewed@1111111111111111111111111111111111111111 }' \
    "$ci_workflow" > "$ci_fixture/flow-uses.yml"
if check_ci_action_admission \
    "$ci_fixture/flow-uses.yml" "$ci_action_review"; then
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
    ${ARCH}/01-Layer-Model.md
    ${ARCH}/02-Resource-Buffer.md
    ${ARCH}/03-Domain-Representation-Projection-View.md
    ${ARCH}/04-Execution-and-Concurrency.md
    ${ARCH}/05-Client-CLM-CCM-and-Rendering.md
    Documentation/03-Extension-Contracts.md
    ${PERFORMANCE}.md
    ${EVENT_STREAM}.md
    ${QUALITY}.md
)
for path in "${retired_monoliths[@]}"; do
    [[ ! -e "$path" ]] || fail "retired normative monolith remains: $path"
done

contract_indexes=(
    Documentation/00-Concept.md
    ${ARCH}/README.md
    ${LAYER_ROLES}.md
    ${LAYER_PORTS}.md
    ${INTERACTION}.md
    ${ARCH}/05-Client/README.md
    ${BOOTSTRAP_HEALTH}.md
    ${BOOTSTRAP_HEALTH}/03-Retention-Lease-and-Snapshot.md
    ${EXTENSIONS}/README.md
    ${PERFORMANCE}/README.md
    ${EVENT_STREAM}/README.md
    ${CONFORMANCE}/README.md
    ${CONFORMANCE}/01-Requirements-and-Evidence.md
    ${RULE_MATRIX}.md
    ${CLASSIFIER}.md
    ${PRODUCT_PROOF}.md
    ${QUALITY}/README.md
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
    ${LAYER_ROLES}.md
    ${LAYER_ROLES}/01-Logical-Roles-and-Source-Ownership.md
    ${LAYER_ROLES}/02-Descriptors-and-Dependency-Admission.md
    ${LAYER_ROLES}/03-Source-Dependency-Graph-and-Edges.md
    ${ARCH}/01-Layer/02-Interaction-Assembly-and-Lifecycle.md
    ${LAYER_PORTS}.md
    ${LAYER_PORTS}/01-Layer-Interface-Contracts.md
    ${LAYER_PORTS}/02-Source-Tree-and-Product-Compositions.md
    ${LAYER_PORTS}/03-Portable-Host-and-No-Std-Profiles.md
    ${BUFFER}/01-Resource-Access-Snapshots-and-Change.md
    ${BUFFER}/02-Buffer-Transactions-History-and-Residency.md
    ${BUFFER}/03-External-Effects-and-Conformance.md
    ${SEMANTICS}/01-Domains-Representations-and-Projections.md
    ${INTERACTION}.md
    ${INTERACTION}/01-Intent-Outcome-and-Representation-Updates.md
    ${INTERACTION}/02-Client-View-and-Presentation.md
    ${INTERACTION}/03-External-Semantic-Protocols.md
    ${INTERACTION}/04-End-to-End-Flow-and-Readiness.md
    ${INTERACTION}/05-Fidelity-and-Conformance-Proof.md
    ${ARCH}/04-Execution/01-Execution-Laws-and-Operation-Classes.md
    ${ARCH}/04-Execution/02-State-Lanes-and-Synchronization.md
    ${ARCH}/04-Execution/03-Runtime-Execution-Facility.md
    ${ARCH}/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md
    ${ARCH}/04-Execution/05-Causal-Debugging-and-Evidence.md
    ${ARCH}/05-Client/01-Client-Ownership-and-Family-Contracts.md
    ${ARCH}/05-Client/02-Rendering-Capabilities.md
    ${ARCH}/05-Client/03-Extension-Modules-and-CCM.md
    ${ARCH}/05-Client/04-Lifecycle-Families-and-Release-Evidence.md
    ${ARCH}/05-Client/README.md
    ${ARCH}/README.md
    Documentation/02-Host-API.md
    ${HOST_API}/01-Boundary-and-Capability-Admission.md
    ${HOST_API}/02-Monotonic-Clock.md
    ${HOST_API}/03-Diagnostic-Output.md
    ${HOST_API}/04-Diagnostic-Shutdown-Values.md
    ${BOOTSTRAP_HEALTH}.md
    ${BOOTSTRAP_HEALTH}/01-Ownership-Schema-and-Counters.md
    ${BOOTSTRAP_HEALTH}/02-Output-Health-Proposals.md
    ${BOOTSTRAP_HEALTH}/03-Retention-Lease-and-Snapshot.md
    ${BOOTSTRAP_HEALTH}/03-Retention/01-Descriptor-Lease-and-Record.md
    ${BOOTSTRAP_HEALTH}/03-Retention/02-Counters-Sequence-and-Retention.md
    ${BOOTSTRAP_HEALTH}/03-Retention/03-Owner-Snapshot.md
    ${BOOTSTRAP_HEALTH}/04-Import-Readiness-and-Failure.md
    ${HOST_API}/06-Lifecycle-Adapters-and-Conformance.md
    ${EXTENSIONS}/01-Packages-Contribution-Points-and-Registry.md
    ${EXTENSIONS}/02-SDKs-Service-Contracts-and-Compatibility.md
    ${EXTENSIONS}/03-Adapters-Execution-and-Lifecycle.md
    ${EXTENSIONS}/04-Quick-Authoring-Admission-and-Compatibility.md
    ${EXTENSIONS}/README.md
    ${PERFORMANCE}/01-Reaction-Metrics-Baselines-and-Workloads.md
    ${PERFORMANCE}/02-Scenarios-Bundles-and-Traces.md
    ${PERFORMANCE}/03-Path-Overhead-and-Regression-Evidence.md
    ${PERFORMANCE}/README.md
    ${EVENT_STREAM}/01-Records-Admission-Retention-and-Consumers.md
    ${EVENT_STREAM}/02-Scope-Failure-and-Conformance.md
    ${EVENT_STREAM}/03-Startup-Family-and-Canonical-Bytes.md
    ${EVENT_STREAM}/README.md
    Documentation/05-Migration/01-Selective-Migration.md
    ${CONFORMANCE}/01-Requirements-and-Evidence.md
    ${RULE_MATRIX}.md
    ${RULE_MATRIX}/01-Product-Authority-and-Layering.md
    ${RULE_MATRIX}/02-Safety-Bounds-Effects-and-Exposure.md
    ${RULE_MATRIX}/03-Resource-and-Editing-State.md
    ${RULE_MATRIX}/04-Meaning-Interaction-and-View.md
    ${RULE_MATRIX}/05-Execution-Lifecycle-and-Debugging.md
    ${RULE_MATRIX}/06-Clients-Synchronization-and-Rendering.md
    ${RULE_MATRIX}/07-Host-SDK-and-Extensions.md
    ${RULE_MATRIX}/08-Performance-Events-Migration-and-Qualification.md
    ${CONFORMANCE}/03-Evidence-Taxonomy-and-Authority.md
    ${CLASSIFIER}.md
    ${CLASSIFIER}/01-Policy-Artifacts-and-Source-Axes.md
    ${CLASSIFIER}/02-Exact-Execution-Topology-and-Dependency-Evidence.md
    ${CLASSIFIER}/03-Inventory-Includes-and-Evidence-Publication.md
    ${CLASSIFIER}/04-Phase-B-Scope-and-Product-Gate.md
    ${PRODUCT_PROOF}.md
    ${PRODUCT_INPUT}
    ${PRODUCT_PROOF}/02-Prepared-Root-and-Bounded-Execution.md
    ${PRODUCT_PROOF}/03-Compiler-Units-Inputs-and-Correlation.md
    ${PRODUCT_PROOF}/04-Package-Membership-Evidence.md
    ${PRODUCT_PROOF}/05-Public-Surface-and-Consumer-Evidence.md
    ${PRODUCT_PROOF}/06-Claims-Unavailability-and-Verification.md
    ${CONFORMANCE}/05-Public-Reduction-and-Qualification.md
    ${CONFORMANCE}/README.md
    ${QUALITY}/01-Integrity-Effects-and-Fencing.md
    ${QUALITY}/02-Lifecycle-Isolation-and-Authority.md
    ${QUALITY}/03-Admission-Qualification-and-Release-Scope.md
    ${QUALITY}/README.md
    Documentation/README.md
)
mapfile -t actual_normative_files < <(
    find Documentation -type f -name '*.md' \
        ! -path 'Documentation/heritage/*' -print | sort
)
if [[ "${actual_normative_files[*]}" != \
    "${expected_normative_files[*]}" ]]; then
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
    ${ARCH}/README.md \
    Documentation/02-Host-API.md \
    ${EXTENSIONS}/README.md \
    ${PERFORMANCE}/README.md \
    ${EVENT_STREAM}/README.md \
    Documentation/05-Migration/01-Selective-Migration.md \
    ${CONFORMANCE}/README.md \
    ${QUALITY}/README.md
check_index_children Documentation/00-Concept.md \
    Documentation/00-Concept/01-Product-System-and-Kernel.md \
    Documentation/00-Concept/02-State-Semantics-Clients-and-Extensions.md \
    Documentation/00-Concept/03-Quality-Authority-and-v016-Proof.md
check_index_children ${ARCH}/README.md \
    ${LAYER_ROLES}.md \
    ${ARCH}/01-Layer/02-Interaction-Assembly-and-Lifecycle.md \
    ${LAYER_PORTS}.md \
    ${BUFFER}/01-Resource-Access-Snapshots-and-Change.md \
    ${BUFFER}/02-Buffer-Transactions-History-and-Residency.md \
    ${BUFFER}/03-External-Effects-and-Conformance.md \
    ${SEMANTICS}/01-Domains-Representations-and-Projections.md \
    ${INTERACTION}.md \
    ${ARCH}/04-Execution/01-Execution-Laws-and-Operation-Classes.md \
    ${ARCH}/04-Execution/02-State-Lanes-and-Synchronization.md \
    ${ARCH}/04-Execution/03-Runtime-Execution-Facility.md \
    ${ARCH}/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md \
    ${ARCH}/04-Execution/05-Causal-Debugging-and-Evidence.md \
    ${ARCH}/05-Client/README.md
check_index_children ${LAYER_ROLES}.md \
    ${LAYER_ROLES}/01-Logical-Roles-and-Source-Ownership.md \
    ${LAYER_ROLES}/02-Descriptors-and-Dependency-Admission.md \
    ${LAYER_ROLES}/03-Source-Dependency-Graph-and-Edges.md
check_index_children ${LAYER_PORTS}.md \
    ${LAYER_PORTS}/01-Layer-Interface-Contracts.md \
    ${LAYER_PORTS}/02-Source-Tree-and-Product-Compositions.md \
    ${LAYER_PORTS}/03-Portable-Host-and-No-Std-Profiles.md
check_index_children ${INTERACTION}.md \
    ${INTERACTION}/01-Intent-Outcome-and-Representation-Updates.md \
    ${INTERACTION}/02-Client-View-and-Presentation.md \
    ${INTERACTION}/03-External-Semantic-Protocols.md \
    ${INTERACTION}/04-End-to-End-Flow-and-Readiness.md \
    ${INTERACTION}/05-Fidelity-and-Conformance-Proof.md
check_index_children ${ARCH}/05-Client/README.md \
    ${ARCH}/05-Client/01-Client-Ownership-and-Family-Contracts.md \
    ${ARCH}/05-Client/02-Rendering-Capabilities.md \
    ${ARCH}/05-Client/03-Extension-Modules-and-CCM.md \
    ${ARCH}/05-Client/04-Lifecycle-Families-and-Release-Evidence.md
check_index_children Documentation/02-Host-API.md \
    ${HOST_API}/01-Boundary-and-Capability-Admission.md \
    ${HOST_API}/02-Monotonic-Clock.md \
    ${HOST_API}/03-Diagnostic-Output.md \
    ${HOST_API}/04-Diagnostic-Shutdown-Values.md \
    ${BOOTSTRAP_HEALTH}.md \
    ${HOST_API}/06-Lifecycle-Adapters-and-Conformance.md
check_index_children ${BOOTSTRAP_HEALTH}.md \
    ${BOOTSTRAP_HEALTH}/01-Ownership-Schema-and-Counters.md \
    ${BOOTSTRAP_HEALTH}/02-Output-Health-Proposals.md \
    ${BOOTSTRAP_HEALTH}/03-Retention-Lease-and-Snapshot.md \
    ${BOOTSTRAP_HEALTH}/04-Import-Readiness-and-Failure.md
check_index_children \
    ${BOOTSTRAP_HEALTH}/03-Retention-Lease-and-Snapshot.md \
    ${BOOTSTRAP_HEALTH}/03-Retention/01-Descriptor-Lease-and-Record.md \
    ${BOOTSTRAP_HEALTH}/03-Retention/02-Counters-Sequence-and-Retention.md \
    ${BOOTSTRAP_HEALTH}/03-Retention/03-Owner-Snapshot.md
check_index_children ${EXTENSIONS}/README.md \
    ${EXTENSIONS}/01-Packages-Contribution-Points-and-Registry.md \
    ${EXTENSIONS}/02-SDKs-Service-Contracts-and-Compatibility.md \
    ${EXTENSIONS}/03-Adapters-Execution-and-Lifecycle.md \
    ${EXTENSIONS}/04-Quick-Authoring-Admission-and-Compatibility.md
check_index_children ${PERFORMANCE}/README.md \
    ${PERFORMANCE}/01-Reaction-Metrics-Baselines-and-Workloads.md \
    ${PERFORMANCE}/02-Scenarios-Bundles-and-Traces.md \
    ${PERFORMANCE}/03-Path-Overhead-and-Regression-Evidence.md
check_index_children ${EVENT_STREAM}/README.md \
    ${EVENT_STREAM}/01-Records-Admission-Retention-and-Consumers.md \
    ${EVENT_STREAM}/02-Scope-Failure-and-Conformance.md \
    ${EVENT_STREAM}/03-Startup-Family-and-Canonical-Bytes.md
check_index_children ${CONFORMANCE}/README.md \
    ${CONFORMANCE}/01-Requirements-and-Evidence.md \
    ${RULE_MATRIX}.md \
    ${CONFORMANCE}/03-Evidence-Taxonomy-and-Authority.md \
    ${CLASSIFIER}.md \
    ${CONFORMANCE}/05-Public-Reduction-and-Qualification.md
check_index_children ${CONFORMANCE}/01-Requirements-and-Evidence.md \
    ${RULE_MATRIX}.md \
    ${CONFORMANCE}/03-Evidence-Taxonomy-and-Authority.md \
    ${CLASSIFIER}.md \
    ${CONFORMANCE}/05-Public-Reduction-and-Qualification.md
check_index_children ${RULE_MATRIX}.md \
    ${RULE_MATRIX}/01-Product-Authority-and-Layering.md \
    ${RULE_MATRIX}/02-Safety-Bounds-Effects-and-Exposure.md \
    ${RULE_MATRIX}/03-Resource-and-Editing-State.md \
    ${RULE_MATRIX}/04-Meaning-Interaction-and-View.md \
    ${RULE_MATRIX}/05-Execution-Lifecycle-and-Debugging.md \
    ${RULE_MATRIX}/06-Clients-Synchronization-and-Rendering.md \
    ${RULE_MATRIX}/07-Host-SDK-and-Extensions.md \
    ${RULE_MATRIX}/08-Performance-Events-Migration-and-Qualification.md
check_index_children ${CLASSIFIER}.md \
    ${CLASSIFIER}/01-Policy-Artifacts-and-Source-Axes.md \
    ${CLASSIFIER}/02-Exact-Execution-Topology-and-Dependency-Evidence.md \
    ${CLASSIFIER}/03-Inventory-Includes-and-Evidence-Publication.md \
    ${CLASSIFIER}/04-Phase-B-Scope-and-Product-Gate.md \
    ${PRODUCT_PROOF}.md
check_index_children ${PRODUCT_PROOF}.md \
    ${PRODUCT_INPUT} \
    ${PRODUCT_PROOF}/02-Prepared-Root-and-Bounded-Execution.md \
    ${PRODUCT_PROOF}/03-Compiler-Units-Inputs-and-Correlation.md \
    ${PRODUCT_PROOF}/04-Package-Membership-Evidence.md \
    ${PRODUCT_PROOF}/05-Public-Surface-and-Consumer-Evidence.md \
    ${PRODUCT_PROOF}/06-Claims-Unavailability-and-Verification.md
check_index_children ${QUALITY}/README.md \
    ${QUALITY}/01-Integrity-Effects-and-Fencing.md \
    ${QUALITY}/02-Lifecycle-Isolation-and-Authority.md \
    ${QUALITY}/03-Admission-Qualification-and-Release-Scope.md

check_index_sections() {
    local index="$1"
    shift
    local expected actual
    expected="$(printf '## %s\n' "$@")"
    actual="$( { rg '^## ' "$index" || true; } )"
    if [[ "$actual" != "$expected" ]]; then
        fail "$index contains unexpected detail instead of navigation only"
    fi
}

check_index_sections \
    ${LAYER_ROLES}.md \
    'Reading order'
check_index_sections \
    ${LAYER_PORTS}.md \
    'Reading order'
check_index_sections \
    ${INTERACTION}.md \
    'Reading order'
check_index_sections \
    Documentation/02-Host-API.md \
    'Reading order'
check_index_sections \
    ${BOOTSTRAP_HEALTH}.md \
    'Reading order'
check_index_sections \
    ${BOOTSTRAP_HEALTH}/03-Retention-Lease-and-Snapshot.md \
    'Reading order'
check_index_sections \
    ${CLASSIFIER}.md \
    'Authority and reading order'
check_index_sections \
    ${PRODUCT_PROOF}.md \
    'Authority and reading order'

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

# reovim-style: allow-long-line searchable-string
obsolete_authority_pattern='02-Process/|03-State/|04-Domain-Substrate/|05-View/|06-ABI/|07-Surfaces/|08-Client/|09-Conformance/|10-Development/|Documentation/(debt|future)/'
if rg -n "$obsolete_authority_pattern" "${active_files[@]}"; then
    fail "active documentation links or delegates to an obsolete authority"
fi

if rg -n '\b[0-9a-f]{40}\b|#[0-9]{3,}' Documentation \
    --glob '*.md' --glob '!Documentation/heritage/**'; then
    fail "normative Documentation contains a branch hash or tracker chronology"
fi

required_statements=(
    'Text is one Domain'
    'It is not an operating-system'
    # reovim-style: allow-long-line searchable-string
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
        fail "focused authority clause is not complete and unique:" \
            "$file: $clause"
    fi
}

layer_roles=${LAYER_ROLES}/01-Logical-Roles-and-Source-Ownership.md
layer_edges=${LAYER_ROLES}/03-Source-Dependency-Graph-and-Edges.md
layer_interfaces=${LAYER_PORTS}/01-Layer-Interface-Contracts.md
source_placement=${LAYER_PORTS}/02-Source-Tree-and-Product-Compositions.md
portable_host=${LAYER_PORTS}/03-Portable-Host-and-No-Std-Profiles.md
classification_artifacts=${CLASSIFIER}/01-Policy-Artifacts-and-Source-Axes.md
product_proof_authority=${PRODUCT_INPUT}
tooling_authority=${QUALITY}/03-Admission-Qualification-and-Release-Scope.md
# reovim-style: allow-long-line searchable-string
inward_clause='Product dependencies point inward without cycles. `RepositoryTooling` grants no product edge and enters no product shipping closure.'
# reovim-style: allow-long-line searchable-string
foundation_clause='Foundation and Kernel contracts contain no concrete environment, adapter, Domain, transport, Client, presentation, or extension mechanism.'
# reovim-style: allow-long-line searchable-string
role_clause='Resource Adapter, Host Adapter, External Protocol Binding, Protocol Adapter, Presentation Adapter, Extension Adapter, and Extension Engine Adapter are distinct roles. Selection or composition of one never merges it with another.'
# reovim-style: allow-long-line searchable-string
evidence_clause='Every evidence set carries an exact-revision Source Role Map generated from version-matched, schema-valid classification policy, exact workspace topology, entrypoint reviews, and dependency admissions. Phase-B v1 reports only dependency reachability for an all-`RepositoryTooling` topology and fails closed on Product or shipping input.'
# reovim-style: allow-long-line searchable-string
proof_clause='Every Product shipping or validation closure is selected by version-matched, schema-valid Product-closure and execution-tool admissions and is proved by correlated compiler-reported Product source/dependencies, Cargo package membership where shipping, and external-crate public-consumer evidence. Unsupported mechanisms fail closed, and fixture capability never qualifies active Product.'
# reovim-style: allow-long-line searchable-string
tooling_clause='Repository Tooling enters active implementation only for one named immediate consumer and exact block, remains the smallest bounded end-to-end path, is accepted only when that consumer invokes and consumes it, and stops when the block clears; fixtures alone and anticipated future use never admit or expand it.'
require_unique_owner_clause "$layer_edges" "$inward_clause"
require_unique_owner_clause "$layer_edges" "$foundation_clause"
require_unique_owner_clause "$layer_roles" "$role_clause"
require_unique_owner_clause "$classification_artifacts" "$evidence_clause"
require_unique_owner_clause "$product_proof_authority" "$proof_clause"
require_unique_owner_clause "$tooling_authority" "$tooling_clause"

if rg -Fq 'roles do not merge' "$layer_edges"; then
    fail 'dependency graph duplicates the role-distinction authority'
fi
if rg -Fq 'owns its deliberately unproved Product boundary' \
    "$classification_artifacts"; then
    fail 'classification authority delegates part of ARCH-006 to a second owner'
fi

rule_index=${RULE_MATRIX}.md
rule_chapters=(
    ${RULE_MATRIX}/01-Product-Authority-and-Layering.md
    ${RULE_MATRIX}/02-Safety-Bounds-Effects-and-Exposure.md
    ${RULE_MATRIX}/03-Resource-and-Editing-State.md
    ${RULE_MATRIX}/04-Meaning-Interaction-and-View.md
    ${RULE_MATRIX}/05-Execution-Lifecycle-and-Debugging.md
    ${RULE_MATRIX}/06-Clients-Synchronization-and-Rendering.md
    ${RULE_MATRIX}/07-Host-SDK-and-Extensions.md
    ${RULE_MATRIX}/08-Performance-Events-Migration-and-Qualification.md
)

architecture_rule_chapter=${RULE_MATRIX}/01-Product-Authority-and-Layering.md
meaning_rule_chapter=${RULE_MATRIX}/04-Meaning-Interaction-and-View.md
host_rule_chapter=${RULE_MATRIX}/07-Host-SDK-and-Extensions.md
migration_rule_chapter=${rule_chapters[7]}

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
    ${LAYER_ROLES}/03-Source-Dependency-Graph-and-Edges.md
check_rule_authority "$architecture_rule_chapter" ARCH-002 \
    ${LAYER_ROLES}/03-Source-Dependency-Graph-and-Edges.md
check_rule_authority "$architecture_rule_chapter" ARCH-003 \
    ${LAYER_ROLES}/01-Logical-Roles-and-Source-Ownership.md
check_rule_authority "$architecture_rule_chapter" ARCH-006 \
    ${CLASSIFIER}/01-Policy-Artifacts-and-Source-Axes.md
check_rule_authority "$architecture_rule_chapter" ARCH-007 \
    ${PRODUCT_INPUT}
check_rule_authority "$architecture_rule_chapter" ARCH-008 \
    "$layer_interfaces"
check_rule_authority "$architecture_rule_chapter" ARCH-009 \
    "$source_placement"
check_rule_authority "$architecture_rule_chapter" ARCH-010 \
    "$portable_host"
check_rule_authority "$architecture_rule_chapter" TOOL-001 \
    "$tooling_authority"
check_rule_authority "$meaning_rule_chapter" INT-001 \
    ${INTERACTION}/01-Intent-Outcome-and-Representation-Updates.md
check_rule_authority "$meaning_rule_chapter" INT-002 \
    ${INTERACTION}/01-Intent-Outcome-and-Representation-Updates.md
check_rule_authority "$meaning_rule_chapter" VIEW-001 \
    ${INTERACTION}/02-Client-View-and-Presentation.md
check_rule_authority "$meaning_rule_chapter" PROTO-001 \
    ${INTERACTION}/03-External-Semantic-Protocols.md
check_rule_authority "$host_rule_chapter" HOST-004 \
    ${HOST_API}/01-Boundary-and-Capability-Admission.md
check_rule_authority "$host_rule_chapter" SDK-005 \
    ${EXTENSIONS}/01-Packages-Contribution-Points-and-Registry.md
check_rule_authority "$migration_rule_chapter" MIG-003 \
    Documentation/05-Migration/01-Selective-Migration.md

# reovim-style: allow-long-line searchable-string
rule_ids="$(awk -F'`' '/^\| `[A-Z]+-[0-9][0-9][0-9]`/ { print $2 }' "${rule_chapters[@]}")"
[[ -n "$rule_ids" ]] || fail "Rule Matrix contains no conformance identifiers"
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
            # reovim-style: allow-long-line searchable-string
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
    fail "Rule Matrix applicability, responsibility, or taxonomy" \
        "validation failed"
fi

rg -Fq '`reovim-rule-matrix/v1`' "$rule_index" || \
    fail "Rule Matrix schema version is missing"
for taxonomy_file in \
    "$rule_index" \
    ${CONFORMANCE}/03-Evidence-Taxonomy-and-Authority.md; do
    rg -Fq '`reovim-evidence/v1`' "$taxonomy_file" || \
        fail "evidence taxonomy version is missing from $taxonomy_file"
done

echo "check-documentation: all checks passed (${#markdown_files[@]} files)"
