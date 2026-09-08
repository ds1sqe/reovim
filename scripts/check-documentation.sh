#!/usr/bin/env bash
# Local links and readability bounds for active contributor documentation.

set -euo pipefail
unset RIPGREP_CONFIG_PATH

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

markdown_files=(
    README.md
    GOVERNANCE.md
    CONTRIBUTING.md
    AGENTS.md
    CODE_STYLE.md
    perf/README.md
)
exec 3< <(rg --files -0 Documentation .github --glob '*.md' \
    --glob '!Documentation/heritage/**' | sort -z)
inventory_pid=$!
while IFS= read -r -d '' file <&3; do
    markdown_files+=("$file")
done
exec 3<&-
if ! wait "$inventory_pid"; then
    echo 'DOC_INVENTORY: active documentation could not be read' >&2
    exit 1
fi

perl -MFile::Spec -e '
use strict;
use warnings;

my @issues;
for my $file (@ARGV) {
    open my $handle, "<", $file or die "DOC_READ: $file: $!\n";
    local $/;
    my $text = <$handle>;
    close $handle or die "DOC_READ: $file: $!\n";
    my $lines = ($text =~ tr/\n//);
    ++$lines if length($text) && substr($text, -1) ne "\n";
    push @issues, "DOC_CHAPTER_LENGTH: $file exceeds 250 lines"
        if $file =~ m{^Documentation/} && $lines > 250;

    my $line = 0;
    my $fence = "";
    for my $source (split /\n/, $text) {
        ++$line;
        if ($source =~ /^\s*(```+|~~~+)/) {
            my $marker = substr($1, 0, 1);
            $fence = $fence eq $marker ? "" : $marker if !$fence
                || $fence eq $marker;
            next;
        }
        next if $fence;
        my @targets = $source =~ /\[[^\]]*\]\(([^)]+)\)/g;
        push @targets, $1 if $source =~ /^\s*\[[^\]]+\]:\s*(\S+)/;
        for my $target (@targets) {
            $target =~ s/^\s+|\s+$//g;
            $target =~ s/^<(.*)>$/$1/;
            next if $target =~ m{^(?:https?://|mailto:)};
            if ($target =~ /#/) {
                push @issues, "DOC_LOCAL_FRAGMENT: $file:$line: $target";
                next;
            }
            $target =~ s/%([0-9a-fA-F]{2})/chr(hex($1))/eg;
            my (undef, $directory, undef) = File::Spec->splitpath($file);
            my $resolved = File::Spec->rel2abs($target, $directory || ".");
            push @issues, "DOC_LINK_MISSING: $file:$line: $target"
                unless -e $resolved;
        }
    }
}
if (@issues) {
    print STDERR join("\n", @issues), "\n";
    exit 1;
}
' "${markdown_files[@]}"

echo 'check-documentation: active links and chapter bounds passed'
