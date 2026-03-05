#!/usr/bin/env bash
# Documentation freshness checks for CI.
# Validates links, metadata headers, staleness, and orphan docs.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERRORS=0

# --- Helper ---
fail() {
    echo "FAIL: $1" >&2
    ERRORS=$((ERRORS + 1))
}

# Extract relative markdown links from a file (no URLs)
extract_links() {
    grep -oE '\[[^]]*\]\([^)#]+' "$1" 2>/dev/null | sed 's/.*](//' | grep -v '^https\{0,1\}://' | grep -v '^mailto:' || true
}

# --- 1. Metadata header check ---
echo "=== Checking metadata headers ==="
while IFS= read -r -d '' file; do
    rel="${file#"$REPO_ROOT"/}"
    case "$rel" in docs/archive/*) continue ;; esac
    if ! head -1 "$file" | grep -q '^---$'; then
        fail "$rel: missing YAML frontmatter"
        continue
    fi
    frontmatter=$(awk 'NR==1{next} /^---$/{exit} {print}' "$file")
    for field in title last_verified status; do
        if ! echo "$frontmatter" | grep -q "^${field}:"; then
            fail "$rel: missing '$field' in frontmatter"
        fi
    done
done < <(find "$REPO_ROOT/docs" -name '*.md' -print0)

# --- 2. Staleness detection ---
echo "=== Checking staleness ==="
if date -v-90d +%Y-%m-%d >/dev/null 2>&1; then
    THRESHOLD=$(date -v-90d +%Y-%m-%d)
else
    THRESHOLD=$(date -d "90 days ago" +%Y-%m-%d)
fi
while IFS= read -r -d '' file; do
    rel="${file#"$REPO_ROOT"/}"
    case "$rel" in docs/archive/*) continue ;; esac
    last_verified=$(sed -n 's/^last_verified: *//p' "$file" | head -1 | tr -d ' ')
    if [ -n "$last_verified" ] && [[ "$last_verified" < "$THRESHOLD" ]]; then
        fail "$rel: last_verified ($last_verified) is older than 90 days (threshold: $THRESHOLD)"
    fi
done < <(find "$REPO_ROOT/docs" -name '*.md' -print0)

# --- 3. Link validation ---
echo "=== Checking links ==="
check_links_in_file() {
    local file="$1"
    local dir
    dir=$(dirname "$file")
    local links
    links=$(extract_links "$file")
    local link
    for link in $links; do
        if [ ! -e "$dir/$link" ]; then
            local rel="${file#"$REPO_ROOT"/}"
            fail "$rel: broken link to '$link'"
        fi
    done
}
check_links_in_file "$REPO_ROOT/CLAUDE.md"
while IFS= read -r -d '' file; do
    rel="${file#"$REPO_ROOT"/}"
    case "$rel" in docs/archive/*) continue ;; esac
    check_links_in_file "$file"
done < <(find "$REPO_ROOT/docs" -name '*.md' -print0)

# --- 4. Orphan detection ---
echo "=== Checking for orphan docs ==="
hop1_links=$(extract_links "$REPO_ROOT/CLAUDE.md")

# Collect hop 2 links
hop2_links=""
for link in $hop1_links; do
    target="$REPO_ROOT/$link"
    if [ -f "$target" ]; then
        dir=$(dirname "$target")
        more=$(extract_links "$target")
        for ml in $more; do
            if [ -e "$dir/$ml" ]; then
                resolved=$(cd "$dir" && pwd)/"$ml"
                hop2_links="$hop2_links ${resolved#"$REPO_ROOT"/}"
            fi
        done
    fi
done

while IFS= read -r -d '' file; do
    rel="${file#"$REPO_ROOT"/}"
    found=0
    for link in $hop1_links; do
        if [ "$link" = "$rel" ]; then found=1; break; fi
    done
    if [ "$found" -eq 0 ]; then
        for link in $hop2_links; do
            if [ "$link" = "$rel" ]; then found=1; break; fi
        done
    fi
    if [ "$found" -eq 0 ]; then
        fail "$rel: orphan doc (not reachable from CLAUDE.md within 2 hops)"
    fi
done < <(find "$REPO_ROOT/docs" -name '*.md' -not -path '*/archive/*' -print0)

# --- Summary ---
echo ""
if [ "$ERRORS" -gt 0 ]; then
    echo "FAILED: $ERRORS issue(s) found" >&2
    exit 1
else
    echo "OK: all documentation checks passed"
fi
