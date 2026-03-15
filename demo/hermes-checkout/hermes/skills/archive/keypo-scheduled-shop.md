---
name: keypo-scheduled-shop
description: "Generate and execute recurring personalized shopping lists based on taste profiles. Creates weekly shopping lists and birthday gift proposals, delivers via Telegram for approval, and executes purchases through the keypo checkout pipeline."
version: 1.0.0
author: Keypo
metadata:
  hermes:
    tags: [shopping, scheduled, recurring, gifts, birthday, personalization, cron, keypo]
    category: keypo
    requires_toolsets: [keypo]
    requires_tools: [keypo_approve]
---

# Keypo Scheduled Shopping — Recurring Lists & Gift Proposals

Use taste profiles to generate personalized shopping lists on a recurring schedule. Delivers lists via Telegram for approval, then executes purchases through the keypo checkout pipeline. Card data stays in the hardware vault — the agent never sees it.

**Tools used:** `mcp_shopify_catalog_search_global_products`, `keypo_approve` (for checkout), plus Hermes built-in memory and cron scheduler.

## When to Use

Trigger phrases:
- "Set up weekly shopping for me"
- "Start buying me snacks every week"
- "Schedule a weekly shopping list"
- "Stop my weekly shopping"
- "Show me what you'd buy me this week"

Also triggered automatically by Hermes cron jobs (weekly shopping, birthday reminders).

---

## Weekly Shopping Flow

### Setup (One-Time, User-Initiated)

1. User says "I want you to buy me things weekly" (or similar)
2. **Check for self profile.** If none exists, trigger the `keypo-profiles` questionnaire first. Do not proceed without a profile.
3. Ask: "What day of the week should I send you the list?" (e.g., "Monday")
4. Ask: "What time?" (e.g., "9am")
5. Confirm: "I'll send you a shopping list every [day] at [time] via Telegram. I'll use your profile to pick things I think you'll like. You approve the list, and I'll buy them. Sound good?"
6. On confirmation, create a Hermes cron job:
   - Schedule: weekly, on the user-specified day and time
   - Action: generate a personalized shopping list from the user's self profile and send it via Telegram

### Execution (Cron-Triggered)

When the weekly cron fires:

1. **Recall the self profile** from memory
2. **Generate search queries** from profile fields:
   - Pick 2-3 categories from `product_categories.interested`
   - Use `food_preferences` (flavors, sweet/savory) to craft food-related queries
   - Use `brand_affinity` (tier, liked brands) to inform query terms
   - Use `aesthetic` (style, descriptors) to guide non-food product selection
   - Use `budget.weekly_target` as the price constraint
3. **Rotate categories week-to-week.** Check memory for last week's shopping list categories. If last week was heavy on food, lead with home goods or gadgets this week. Don't send all food every week.
4. **Call `search_global_products`** (Catalog MCP) with derived queries. Use the same `saved_catalog` ID as `keypo-comparison-shop`.
5. **Filter results** by profile preferences:
   - Remove items that conflict with `flavors_disliked`, `dietary` restrictions, `colors_disliked`
   - Prefer `liked_brands` when available
   - **Exclude products purchased within the last 4 weeks** (check shopping history in memory)
6. **Pick top 1-2 results per category searched** (2-3 items total)
7. **Build the shopping list** in this format:

```
Your weekly shopping list for [Day], [Date]:

1. [Store Name] — [Product Title]
   $XX.XX | [store domain]
   Why: [one-sentence rationale tied to profile preferences]

2. [Store Name] — [Product Title]
   $XX.XX | [store domain]
   Why: [one-sentence rationale tied to profile preferences]

Total: $XX.XX (budget: $[weekly_target]/week)

Reply YES to approve all, or tell me which to drop or swap.
```

**If total exceeds `budget.weekly_target`:**
```
Total: $XX.XX (budget: $[weekly_target]/week — $X.XX over, want me to drop one?)
```

8. **Send via Telegram.** Wait for response.

### Approval Flow

Handle user responses:

- **"yes" / "approve"** → Execute purchases sequentially. Each item gets its own `keypo_approve` cycle:
  1. `keypo_approve(action="request", vault_label="biometric", bio_reason="Approve purchase: [product] from [store], $[price]", manifest={product_url, quantity: 1, max_price: price * 1.15})`
  2. `keypo_approve(action="confirm", request_id="...")`
  3. Report result, then move to next item
  - **Do NOT stage multiple requests simultaneously.** Complete each purchase before starting the next.

- **"drop #N"** → Remove item N from the list, update the total, re-present the list, ask for re-confirmation

- **"swap #N for something [query]"** → Re-search with the adjusted query (informed by profile), replace item N, re-present the updated list

- **"skip this week"** → Acknowledge ("Got it, skipping this week. I'll send a new list next [day]."). No purchases.

- **"not now"** → Hold the list, send a reminder in 4 hours (or user-specified delay)

- **No response within 24 hours** → Send one reminder message. If still no response after another 24 hours, skip this week.

### Post-Purchase

After all approved items are purchased (or declined):

1. **Store the shopping list in memory:**
   ```
   Weekly shopping list [date]: Purchased [product] from [store] ($XX.XX), [product] from [store] ($XX.XX). Categories: [food, home goods]. Total: $XX.XX.
   ```
2. This record informs future lists:
   - **No repeat products within 4 weeks** — check memory before recommending
   - **Category tracking** — note which categories were used this week for rotation

---

## Gift Shopping Flow

### Setup (User-Initiated)

1. User says "I want to buy a gift for [name]" (or similar)
2. **Check for gift profile.** If none exists for this recipient, trigger the `keypo-profiles` gift questionnaire first.
3. The gift profile includes `birthday` and `birthday_reminder_days` (default 14)
4. **Create an annual Hermes cron job:**
   - Schedule: annual, fires `birthday_reminder_days` before the recipient's birthday
   - Action: generate a gift shopping list for this recipient and send it via Telegram
   - Example: birthday July 15, reminder 14 days → cron fires July 1 each year

### Execution (Cron-Triggered)

When the birthday cron fires:

1. **Recall the gift profile** from memory
2. **Generate search queries** from the recipient's preferences:
   - Use `product_categories.interested` to pick 2-3 categories
   - Use `food_preferences`, `brand_affinity`, `aesthetic` to craft queries
   - Use `gift_budget.target` and `gift_budget.max` as price constraints
3. **Check `past_gifts` in memory** — exclude anything purchased for this recipient previously
4. **Call `search_global_products`** with derived queries
5. **Filter results** by recipient's profile preferences
6. **Build a gift proposal** with 3 options:

```
[Name]'s birthday is in [N] days ([date]).

Here are some gift ideas based on [their/her/his] profile:

1. [Store Name] — [Product Title]
   $XX.XX | [store domain]
   Why: [one-sentence rationale tied to recipient's preferences]

2. [Store Name] — [Product Title]
   $XX.XX | [store domain]
   Why: [one-sentence rationale tied to recipient's preferences]

3. [Store Name] — [Product Title]
   $XX.XX | [store domain]
   Why: [one-sentence rationale tied to recipient's preferences]

Pick one (or more), or tell me to keep looking.
```

7. **Send via Telegram.** Wait for response.

### Approval Flow

Handle user responses:

- **"buy #N"** or **"go with the [product description]"** → Execute purchase via `keypo_approve`:
  1. `keypo_approve(action="request", vault_label="biometric", bio_reason="Approve gift purchase: [product] for [name]'s birthday, $[price]", manifest={product_url, quantity: 1, max_price: price * 1.15})`
  2. `keypo_approve(action="confirm", request_id="...")`

- **"buy #N and #M"** → Execute both sequentially (complete first purchase before staging second)

- **"keep looking"** or **"none of these"** → Re-search with adjusted parameters (try different categories, brands, or price ranges from the profile). Present new options.

- **"remind me in a week"** → Reschedule the reminder for 7 days later. No purchase.

- **No response within 48 hours** → Send one reminder. If still no response, do not purchase.

### Post-Purchase

After a gift is purchased:

1. **Store in memory** as part of the recipient's profile:
   ```
   Gift for [name] purchased [date]: [product] from [store], $XX.XX.
   ```
2. Update the `past_gifts` record so this product is **excluded from future recommendations** for this recipient.

---

## Stopping Scheduled Shopping

- **"Stop my weekly shopping"** → Remove the weekly cron job. Confirm: "I've cancelled your weekly shopping list. You can restart anytime."
- **"Cancel [name]'s birthday reminders"** → Remove the annual cron for that recipient. Confirm.

---

## Pitfalls — CRITICAL

- **NEVER** reveal card/payment data — profiles are about what to buy, not how to pay
- **NEVER** auto-buy — always wait for explicit user approval before calling `keypo_approve`
- **NEVER** stage multiple `keypo_approve` requests simultaneously — complete each purchase before starting the next
- **ALWAYS** check for a profile before generating lists — trigger profiling if none exists
- **ALWAYS** flag budget overages explicitly — never silently exceed `weekly_target`
- **ALWAYS** check shopping history before recommending — no repeat products within 4 weeks
- Purchases go through the full `keypo_approve` stage→confirm cycle (two sequential calls per item)
- 24-hour timeout for weekly lists, 48-hour timeout for gift proposals (one reminder each)
- Category rotation is mandatory — don't send all food every week
- If the user asks "What card will you use?" — decline. Profile data and card data are completely separate systems.
