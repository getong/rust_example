#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s [--dry-run] [path]\n' "${0##*/}"
  printf 'Replaces libp2p 0.56/0.56.0 full-feature Cargo dependencies under path (default: .).\n'
}

dry_run=0
root="."
root_set=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run|-n)
      dry_run=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    -*)
      printf 'Unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [ "$root_set" -eq 1 ]; then
        printf 'Only one path argument is supported.\n' >&2
        usage >&2
        exit 2
      fi
      root="$1"
      root_set=1
      ;;
  esac
  shift
done

if [ ! -d "$root" ]; then
  printf 'Path is not a directory: %s\n' "$root" >&2
  exit 2
fi

manifests=()
while IFS= read -r -d '' manifest; do
  manifests+=("$manifest")
done < <(find "$root" -path '*/target/*' -prune -o -type f -name Cargo.toml -print0)

if [ "${#manifests[@]}" -eq 0 ]; then
  printf 'No Cargo.toml files found under %s\n' "$root" >&2
  exit 0
fi

perl -Mstrict -Mwarnings -e '
my $dry_run = shift @ARGV;
my $replacement = q{libp2p = { git = "https://github.com/libp2p/rust-libp2p", rev = "3e72d4c071d5ec8815d2f6f7ee3602600ff51798", features = ["full"] }};
my $total = 0;

for my $path (@ARGV) {
    open my $in, "<", $path or die "cannot read $path: $!\n";
    local $/;
    my $text = <$in>;
    close $in or die "cannot close $path after reading: $!\n";

    my $count = ($text =~ s#
        ^([ \t]*)libp2p[ \t]*=[ \t]*\{
        (?=[^}]*\bversion[ \t]*=[ \t]*["\047]0\.56(?:\.0)?["\047])
        (?=[^}]*\bfeatures[ \t]*=[ \t]*\[[ \t\r\n]*["\047]full["\047][ \t\r\n]*,?[ \t\r\n]*\])
        [^}]*\}
    #$1 . $replacement#egmsx);

    next if $count == 0;
    $total += $count;

    if ($dry_run) {
        print "would update $path ($count replacement";
    } else {
        open my $out, ">", $path or die "cannot write $path: $!\n";
        print {$out} $text or die "cannot write $path: $!\n";
        close $out or die "cannot close $path after writing: $!\n";
        print "updated $path ($count replacement";
    }
    print $count == 1 ? ")\n" : "s)\n";
}

print $dry_run ? "would update $total replacement" : "updated $total replacement";
print $total == 1 ? "\n" : "s\n";
' "$dry_run" "${manifests[@]}"
