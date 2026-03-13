#!/bin/bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: $0 <product-url> [size]" >&2
  exit 1
fi

PRODUCT_URL="$1"
export PRODUCT_URL
[ -n "${2:-}" ] && export PRODUCT_SIZE="$2"
export HEADLESS="${HEADLESS:-true}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Extract domain for Touch ID reason
DOMAIN=$(echo "$PRODUCT_URL" | sed -E 's|https?://([^/]+).*|\1|')

# Fetch product info for a more descriptive Touch ID prompt
CLEAN_URL=$(echo "$PRODUCT_URL" | sed -E 's/[?#].*//; s|/$||')
PRODUCT_JSON_URL="${CLEAN_URL%.json}.json"
PRODUCT_INFO=$(curl -sf "$PRODUCT_JSON_URL" | \
  /usr/bin/python3 -c "import sys,json; p=json.load(sys.stdin)['product']; \
  print(f'{p[\"title\"]} (\${p[\"variants\"][0][\"price\"]})')" 2>/dev/null || true)

# Build --reason flag with fallback chain
REASON_FLAG=()
if [ -n "$PRODUCT_INFO" ] && [ -n "$DOMAIN" ]; then
  REASON_FLAG=(--reason "Purchase: ${PRODUCT_INFO} from ${DOMAIN}")
elif [ -n "$DOMAIN" ]; then
  REASON_FLAG=(--reason "Checkout from ${DOMAIN}")
else
  REASON_FLAG=(--reason "Checkout: ${PRODUCT_URL}")
fi

cd "$SCRIPT_DIR/bot"
exec keypo-signer vault exec \
  "${REASON_FLAG[@]}" \
  --env "$SCRIPT_DIR/.env.vault-template" \
  -- node ./scripts/start-direct.js
