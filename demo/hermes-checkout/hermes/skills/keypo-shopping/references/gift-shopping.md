# Gift Shopping

Buy gifts for specific people with taste-profile-driven recommendations. Supports both on-demand gift purchases and automated birthday reminders via cron.

## Gift Profile Setup

When the user wants to set up gift shopping for someone:

1. **Check for existing gift profile** in Hermes memory. If found, use it.
2. If no profile exists, gather:
   - Recipient name and relationship
   - Birthday (YYYY-MM-DD)
   - Reminder lead time (default: 14 days before birthday)
3. **Run the taste questionnaire** (same as `references/profiling.md` Phases 1-3, but frame ALL questions about the recipient):
   - "What kinds of things does [name] like — hobbies, interests, things they collect?" → `product_categories`
   - "Any dietary restrictions or food preferences?" → `food_preferences`
   - "What's their style like — minimalist, cozy, bold, eclectic?" → `aesthetic`
   - Image round adapted to recipient's interests
   - Confirm and save profile
4. Ask: "What's your budget for [name]'s gift?" → `gift_budget` (target + max)
5. **Create an annual Hermes cron job:**
   - Schedule: fires `birthday_reminder_days` before the recipient's birthday each year
   - Example: birthday July 15, reminder 14 days → cron fires July 1 annually

Save the complete gift profile to Hermes memory.

## Birthday-Triggered Execution (Cron-Triggered)

When the birthday cron fires:

1. **Recall the gift profile** from memory
2. **Generate search queries** from the recipient's preferences:
   - Pick 2-3 categories from their `product_categories.interested`
   - Use `food_preferences`, `brand_affinity`, `aesthetic` to craft queries
   - Use `gift_budget.target` and `gift_budget.max` as price constraints
3. **Exclude past gifts** — check `past_gifts` in memory, don't recommend anything previously purchased for this recipient
4. **Search** via `search_global_products` with derived queries
5. **Build a gift proposal** with 2-3 options:

```
[Name]'s birthday is in [N] days ([date]).

Here are some gift ideas based on their profile:

1. [Store Name] — [Product Title]
   $XX.XX
   Why: [one-sentence rationale tied to recipient's preferences]

2. [Store Name] — [Product Title]
   $XX.XX
   Why: [one-sentence rationale tied to recipient's preferences]

3. [Store Name] — [Product Title]
   $XX.XX
   Why: [one-sentence rationale tied to recipient's preferences]

Pick one (or more), or tell me to keep looking.
```

Send via Telegram.

## Approval Flow

- **"buy #N"** or **"go with [product]"** → Execute via Core Checkout Procedure in SKILL.md
- **"buy #N and #M"** → Execute both via `references/batch-purchasing.md`
- **"keep looking"** or **"none of these"** → Re-search with adjusted parameters (different categories, brands, or price ranges from the profile). Present new options.
- **"remind me later"** or **"remind me in a week"** → Reschedule the reminder. No purchase.
- **No response within 48 hours** → Send one reminder. If still no response, do not purchase.

## Post-Purchase

After a gift is purchased:
1. Store in memory as part of the recipient's record:
   > Gift for [name] purchased [date]: [product] from [store], $XX.XX.
2. Update `past_gifts` so this product is **excluded from future recommendations** for this recipient.

## Stopping

- **"Cancel [name]'s birthday reminders"** → Remove the annual cron for that recipient. Confirm with user first.
