# Scheduled Shopping

Generate recurring personalized shopping lists based on the user's taste profile. Delivers lists via Telegram for approval, then executes purchases through the checkout pipeline.

## Setup (One-Time, User-Initiated)

1. **Check for self profile.** If none exists, load `references/profiling.md` and run the questionnaire first. Do not proceed without a profile.
2. Ask: "What day of the week should I send you the list?" (e.g., "Monday")
3. Ask: "What time?" (e.g., "9am")
4. Confirm: "I'll send you a shopping list every [day] at [time] via Telegram. I'll use your profile to pick things I think you'll like. You approve the list, and I'll buy them. Sound good?"
5. On confirmation, create a Hermes cron job:
   - Schedule: weekly, on the user-specified day and time
   - Action: generate a personalized shopping list from the user's self profile and send it via Telegram

## Weekly Execution (Cron-Triggered)

When the cron fires:

### 1. Recall Profile
Load the self profile from Hermes memory.

### 2. Generate Search Queries
- Pick 2-3 categories from `product_categories.interested`
- Use `food_preferences` (flavors, sweet/savory) for food queries
- Use `brand_affinity` (tier, liked brands) to inform query terms
- Use `aesthetic` (style, descriptors) to guide non-food selection
- Use `budget.weekly_target` as the price constraint

### 3. Rotate Categories
Check memory for last week's shopping list categories. If last week was heavy on food, lead with home goods or gadgets. Don't send all food every week.

### 4. Search and Filter
- Call `search_global_products` with derived queries (same `saved_catalog` as comparison shopping)
- Remove items conflicting with `flavors_disliked`, `dietary`, `colors_disliked`
- Prefer `liked_brands` when available
- **Exclude products purchased within the last 4 weeks** (check shopping history in memory)
- Pick top 1-2 results per category (2-3 items total)

### 5. Build and Send Shopping List

```
Your weekly shopping list for [Day], [Date]:

1. [Store Name] — [Product Title]
   $XX.XX
   Why: [one-sentence rationale tied to profile preferences]

2. [Store Name] — [Product Title]
   $XX.XX
   Why: [one-sentence rationale tied to profile preferences]

Total: $XX.XX (budget: $[weekly_target]/week)

Reply YES to approve all, or tell me which to drop or swap.
```

If total exceeds budget: `Total: $XX.XX (budget: $[weekly_target]/week — $X.XX over, want me to drop one?)`

Send via Telegram.

## Approval Flow

- **"yes" / "approve"** → Execute via `references/batch-purchasing.md`
- **"drop #N"** → Remove item, update total, re-present, re-confirm
- **"swap #N for something [query]"** → Re-search with adjusted query, replace item, re-present
- **"skip this week"** → "Got it, skipping this week. I'll send a new list next [day]." No purchases.
- **"not now"** → Hold the list, send a reminder in 4 hours
- **No response within 24 hours** → One reminder. If still no response after another 24h, skip this week.

## Post-Purchase

After purchases complete:
1. Store in memory: `Weekly shopping list [date]: Purchased [product] from [store] ($XX.XX), [product] from [store] ($XX.XX). Categories: [food, home goods]. Total: $XX.XX.`
2. This record informs future lists:
   - No repeat products within 4 weeks
   - Category tracking for rotation

## Stopping

- **"Stop my weekly shopping"** → Remove the cron job. "I've cancelled your weekly shopping list. You can restart anytime."
