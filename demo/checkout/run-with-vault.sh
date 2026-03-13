#!/bin/bash
set -euo pipefail

TASK_ID="${1:?Usage: run-with-vault.sh <TASK_ID> [CARD_FRIENDLY_NAME]}"
export TASK_ID
[ -n "${2:-}" ] && export CARD_FRIENDLY_NAME="$2"
export HEADLESS="${HEADLESS:-true}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Build purchase description for Touch ID prompt (all from DB + Shopify, not agent)
REASON_FLAG=()
TASK_URL=$(curl -sf "http://localhost:8080/v1/tasks/${TASK_ID}" | \
  /usr/bin/python3 -c "import sys,json; print(json.load(sys.stdin)['data']['url'])" 2>/dev/null || true)
if [ -n "$TASK_URL" ]; then
  DOMAIN=$(/usr/bin/python3 -c "from urllib.parse import urlparse; import sys; print(urlparse(sys.argv[1]).netloc)" "$TASK_URL" 2>/dev/null || true)
  JSON_URL=$(/usr/bin/python3 -c "import sys; print(sys.argv[1].split('?')[0] + '.json')" "$TASK_URL" 2>/dev/null || true)
  PRODUCT_INFO=$(curl -sf "$JSON_URL" | \
    /usr/bin/python3 -c "import sys,json; p=json.load(sys.stdin)['product']; print(f'{p[\"title\"]} (\${p[\"variants\"][0][\"price\"]})')" 2>/dev/null || true)
  if [ -n "$PRODUCT_INFO" ] && [ -n "$DOMAIN" ]; then
    REASON_FLAG=(--reason "Purchase: ${PRODUCT_INFO} from ${DOMAIN}")
  fi
fi

cd "$SCRIPT_DIR/bot"
exec keypo-signer vault exec \
  "${REASON_FLAG[@]}" \
  --env "$SCRIPT_DIR/.env.vault-template" \
  -- node ./scripts/start-task.js
