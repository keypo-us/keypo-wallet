# Checkout Errors

Reference for handling errors from `keypo_approve` calls. Load this when any checkout step fails.

## Checkout Script Exit Codes

| Exit Code | Name | Meaning | What to Tell the User | What to Offer |
|-----------|------|---------|----------------------|---------------|
| 0 | Success | Order placed successfully | Share the confirmation number | — |
| 2 | `PRICE_CHECK_FAILED` | Cart total exceeds `max_price` (shipping/tax pushed it over) | "The total came to $X.XX, which is over the $Y.YY limit I set." | Retry with a higher limit, or try the next-best option from comparison |
| 3 | `PRODUCT_ERROR` | Product not found, out of stock, or page structure changed | "That product doesn't seem to be available right now." | Re-search for alternatives |
| 4 | `CHECKOUT_ERROR` | Card declined or checkout form error | "The checkout didn't go through." | Try a different store or product |
| 5 | `CONFIG_ERROR` | Missing environment variable or malformed manifest | "There's a configuration issue on my end." | Check daemon setup (internal error) |
| 6 | `NAV_ERROR` | Navigation timeout (page didn't load) | "The store's page took too long to load." | Retry once, or try a different store |

### Special Case: Test Card (Exit 4)

If the vault contains a test card (`4242 4242 4242 4242`), exit code 4 (card declined) is **expected behavior** — it proves the entire pipeline works end-to-end. Report this as a **successful test**, not an error:

> "The checkout pipeline worked perfectly — the test card was declined as expected, which means the product was found, the form was filled, and the payment step was reached."

## Daemon-Level Errors

These come from the daemon (keypo-signer), not the checkout script:

| Error Message | Meaning | What to Offer |
|--------------|---------|---------------|
| `biometric authentication cancelled` | User cancelled Touch ID / Watch prompt | Ask if they want to try again |
| `biometric authentication failed` | Too many failed biometric attempts | Wait and try again, or use a different auth method |
| `request expired` | Staged request is older than 5 minutes | Re-stage the request (new `keypo_approve` request+confirm cycle) |
| `another request is already staged` | A different request is pending confirmation | Cancel the existing request first, or wait for it to complete |

## General Rules

- **Never retry automatically** — always ask the user before retrying
- **Never reveal card data** in error messages (you don't have it anyway)
- **For batch purchases**, report per-item results (some may succeed while others fail)
- If a price check fails during comparison shopping, **offer the next-best option** from the comparison table — don't re-search unless the user asks
