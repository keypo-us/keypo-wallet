---
name: checkout-purchase
description: Use when the user asks to buy a product from the Shopify store.
  Orchestrates checkout with credit card details injected via keypo-signer
  vault exec (biometric policy — Touch ID required).
version: "0.2.0"
metadata:
  author: keypo-us
  requires: keypo-signer, Node 18
---

# Checkout Purchase Skill

Orchestrate a Shopify purchase with credit card secrets injected at runtime from the keypo-signer vault. The agent never sees or handles card data — Touch ID acts as the human-in-the-loop approval.

**For vault usage rules, see `skills/keypo-signer/SKILL.md`.**

---

## Prerequisites check

Run these checks **in parallel** before starting:

```bash
# 1. Vault has address secrets in open tier + card secrets in biometric tier
keypo-signer vault list
# Expect: SHIPPING_FIRST_NAME, SHIPPING_LAST_NAME, etc. in "open"
# Expect: CARD_NUMBER, NAME_ON_CARD, EXPIRATION_MONTH, EXPIRATION_YEAR, SECURITY_CODE in "biometric"
```

If vault secrets are missing, **tell the user to set up the vault** — do not attempt to store secrets yourself.

---

## Workflow

### Discover available products

Query the store's JSON API to find products and their variant IDs:

```bash
curl -s https://shop.keypo.io/products.json | jq '.products[] | {title, handle, price: .variants[0].price, variant_id: .variants[0].id}'
```

Build the product URL as: `https://shop.keypo.io/products/<handle>?variant=<variant_id>`

### 1. Run the checkout

```bash
demo/checkout/run-with-vault.sh <product-url> [size]
```

> **Path note:** `run-with-vault.sh` is in `demo/checkout/`. Always run from the repo root or use an absolute path.

This triggers `keypo-signer vault exec --env .env.vault-template` which will:
- Decrypt open-tier address secrets (no auth needed)
- Decrypt biometric-tier card secrets (**Touch ID prompt appears for user**)
- Launch checkout with all secrets injected as env vars

**Wait for the user to authenticate via Touch ID before proceeding.**

### 2. Monitor output

Watch stdout for checkout status:
- `Navigating to URL` — browser launching
- `Attempting to add product to cart` — add to cart
- `Entering contact email` — checkout started
- `Entering card details` — payment fields
- `Clicking Pay now button` — submitting order
- `has completed` — **success**
- `has a checkout error` — **failure**, inspect browser

### 3. Report result

Tell the user whether the checkout succeeded or failed. If succeeded, note that they should check their email for the order confirmation.

> **Note:** An `Error sending email` / `ECONNREFUSED 127.0.0.1:587` error is expected — there is no local SMTP server. This does not affect the order; it only means the bot's internal notification email was not sent. The Shopify order confirmation is sent separately by Shopify.

---

## Forbidden Actions

These rules are **absolute** — violating them breaks the security model.

1. **Never call `vault get`** — this retrieves plaintext secrets. Use only `vault exec`.
2. **Never write secrets to files** — no `.env` files with real card values.
3. **Never inspect the subprocess environment** — don't try to read env vars from the vault exec child.
4. **Never populate the blank `CARD_*` fields** in `.env.vault-template` — they must remain blank.
5. **Never attempt to store vault secrets** — if secrets are missing, tell the user to set them up.
6. **Never log, echo, or print card values** in any command you construct.

See `skills/keypo-signer/SKILL.md` for the complete vault safety rules.
