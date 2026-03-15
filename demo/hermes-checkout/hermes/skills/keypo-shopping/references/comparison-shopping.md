# Comparison Shopping

Search across Shopify stores, compare prices, and let the user pick the best option. After selection, return to the Core Checkout Procedure in SKILL.md.

## 1. Query the Catalog MCP

Call `mcp_shopify_catalog_search_global_products` with:
- `query`: product description (e.g., "chocolate chip cookies")
- `context`: relevant buyer context (e.g., "looking for artisan cookies, gift-quality")
- `saved_catalog`: `01kkqvbgwy7re02m7qb4524ar9`
- `available_for_sale`: `true`
- `limit`: `10`
- `max_price`: include if the user mentioned a budget

Do NOT pass `ships_to` — it's baked into the saved catalog config (US shipping).

One call is usually sufficient. Don't over-query.

## 2. Process Results

1. Remove out-of-stock items (`availableForSale: false`)
2. Remove duplicates (same product from same store)
3. Apply user preferences if a profile exists in memory
4. Keep only the **top 5**. Discard the rest.
5. If only 1 result, skip the comparison table — say "I only found this at one store" and go to step 5
6. Save each product's `onlineStoreUrl` internally (needed for checkout)

## 3. Present Comparison Table

Use this EXACT format. Copy it literally, filling in the brackets:

```
I found [product] at [N] Shopify stores. Here are the top [3-5]:

1. [Store Name] — [Product Title]
   $XX.XX for [quantity/size] ($X.XX/unit) | Rating: X.X/5 (N reviews)

2. [Store Name] — [Product Title]
   $XX.XX for [quantity/size] ($X.XX/unit) | Rating: X.X/5 (N reviews)

3. [Store Name] — [Product Title]
   $XX.XX for [quantity/size] ($X.XX/unit) | Rating: X.X/5 (N reviews)

Prices don't include shipping or tax.
Best value: #[N] ([Store Name]) — [one specific reason]

Which one should I buy? Or should I go with the best value?
```

### Mandatory Format Rules

- Show 3-5 options. **NEVER more than 5.**
- End with EXACTLY ONE "Best value" line. Pick ONE winner. Not two.
- Compute per-unit price when pack sizes differ. Base "best value" on per-unit cost.
- **Do NOT show URLs, store domains, or links** — you have them internally for checkout.
- Do NOT add emojis, extra headers, descriptions, or commentary between items.
- Do NOT add a "best quality" line. Only "Best value."
- Ratings are optional — only show if available from the API response.
- Do NOT split into "best value" and "best quality." One recommendation only.

## 4. Wait for User Selection

- A number ("2") or store name ("buy from Levain") → proceed to step 5
- "cheapest" / "best value" / "go with your recommendation" → select the flagged best value
- "none" / "nevermind" → acknowledge and stop
- A follow-up question → answer, then re-ask for selection

## 5. Extract the Product URL

The selected product's `onlineStoreUrl` from the API response becomes the `product_url`.

**CRITICAL:** Use `onlineStoreUrl`, NOT `checkoutUrl`. The `checkoutUrl` is a Shopify cart permalink that bypasses the product page and is NOT compatible with the checkout script.

**URL parameter handling:**
- Keep the full URL including `?variant=...` (pre-selects the correct variant)
- If checkout fails, strip `_gsid` first (keep `?variant=...`)
- If still failing, strip everything after `/products/<handle>`

## 6. Return to Core Checkout

With the product URL extracted, return to the **Core Checkout Procedure** in SKILL.md (starting at step 4 — Read the Price).

## Pitfalls

- **Unit price normalization is critical.** A 6-pack for $24 ($4/unit) beats a single for $3 on sticker price but loses on value.
- **ML-inferred fields may be incomplete.** `rating`, `topFeatures`, `uniqueSellingPoint` are ML-generated and not always available.
- **Fewer than 2 results** skips the comparison table — go straight to checkout confirmation.
- **Stale prices.** Catalog API prices are real-time but there's still a window. The checkout script's price check is the enforcer.
- **Shipping can change the ranking.** If a price check fails, offer the next best option from the table.
- **Token expiry.** If the Catalog MCP returns 401, the token has expired. Tell the user and suggest regenerating it.
