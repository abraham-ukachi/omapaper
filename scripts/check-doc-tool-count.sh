#!/usr/bin/env bash
# Fails when a doc claims a different MCP tool count than tools.rs defines.
# Ignored on purpose: CHANGELOG.md (historical entries describe what shipped then)
# and any line that mentions Doop, whose own tool count appears in the comparison table.
set -euo pipefail

# Counts that legitimately belong to another product, not to OmaPaper.
external_counts="42"

root="$(cd "$(dirname "$0")/.." && pwd)"
defs="$root/crates/omapaper-mcp/src/tools.rs"

actual="$(grep -cE '^[[:space:]]+tool\($' "$defs")"
if [ "$actual" -eq 0 ]; then
  echo "could not find any tool definitions in $defs" >&2
  exit 1
fi

status=0
while IFS= read -r file; do
  while IFS=: read -r line text; do
    while IFS= read -r claim; do
      [ "${claim%% *}" = "$actual" ] && continue
      case "$text" in
        *Doop*) continue ;;
      esac
      case " $external_counts " in
        *" ${claim%% *} "*) continue ;;
      esac
      echo "${file#"$root"/}:$line claims \"$claim\", but tools.rs defines $actual" >&2
      status=1
    done < <(grep -oE '[0-9]+ (MCP )?tools' <<<"$text")
  done < <(grep -nE '[0-9]+ (MCP )?tools' "$file")
done < <(printf '%s\n' "$root/README.md" "$root/CONTRIBUTING.md" "$root"/docs/*.md)

for required in "$root/README.md" "$root/docs/mcp.md"; do
  if ! grep -qE "$actual (MCP )?tools" "$required"; then
    echo "${required#"$root"/} no longer states the tool count ($actual)" >&2
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  echo "docs agree with tools.rs: $actual MCP tools"
fi
exit "$status"
