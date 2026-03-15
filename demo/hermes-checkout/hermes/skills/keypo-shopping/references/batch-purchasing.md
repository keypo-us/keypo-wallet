# Batch Purchasing

Buy multiple items in a single session. Build a consolidated plan, get user approval, then execute each purchase sequentially through the Core Checkout Procedure.

## Build the Shopping Plan

Present all items with per-item details before any purchases:

```
Here's your shopping plan:

1. [Store Name] — [Product Title]
   $XX.XX (qty: N, max: $YY.YY)

2. [Store Name] — [Product Title]
   $XX.XX (qty: N, max: $YY.YY)

3. [Store Name] — [Product Title]
   $XX.XX (qty: N, max: $YY.YY)

Estimated total: $XX.XX

Approve all, or tell me which to drop or swap?
```

Each item needs: `label` (human-readable name), `product_url`, `quantity`, `max_price`.

## Partial Approval

- **"drop #N"** → Remove item N, update total, re-present plan
- **"swap #N for something else"** → Re-search for item N (load `references/comparison-shopping.md` if needed), replace in plan, re-present
- **"approve"** or **"yes"** → Execute all items

## Execution: Sequential Purchases

Each item gets its own `keypo_approve` request+confirm cycle:

1. For each item in the approved plan:
   - Stage: `keypo_approve(action="request", vault_label="biometric", bio_reason="Approve purchase: [item label], $[price]", manifest={product_url, quantity, max_price})`
   - Confirm: `keypo_approve(action="confirm", request_id="...")`
   - Record result (success or failure)
   - Move to next item
2. Each item gets its own biometric (Touch ID) prompt
3. **Complete each purchase before starting the next** — do NOT stage multiple requests simultaneously

> **Note:** A future `batch_request` action will enable one biometric prompt for multiple checkouts. For now, each item requires a separate approval.

## Per-Item Result Reporting

After all items are attempted, report results per item:

```
Results:
1. [Product] from [Store] — Success (order #XXXX)
2. [Product] from [Store] — Failed (price check: $XX exceeded $YY max)
3. [Product] from [Store] — Success (order #XXXX)
```

- Failure on one store does **not** skip the next — always attempt all approved items
- For failures, offer the appropriate remedy per `references/checkout-errors.md`

## Limits and Rules

- **Max 10 items** per batch plan
- **Never retry automatically** — ask the user before retrying any failed item
- **Never auto-buy** — the consolidated plan must be explicitly approved before any purchases
- If total exceeds a budget, flag it: "Total is $X over your $Y budget — want to drop anything?"
