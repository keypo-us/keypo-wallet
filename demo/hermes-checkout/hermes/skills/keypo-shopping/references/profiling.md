# Taste Profiling

Build and manage taste profiles for personalized shopping recommendations. Profiles drive product selection in comparison shopping, weekly lists, and gift proposals.

## Profile Types

- **`self`** — The user's own shopping preferences. Covers product categories, food preferences, brand affinity, aesthetic style, and budget.
- **`gift`** — A gift recipient's preferences. Setup details are in `references/gift-shopping.md`.

Product-category preferences (socks, snacks, headphones, etc.) are captured as fields within the `self` profile under `product_categories.interested` with category-specific sub-preferences, rather than as standalone profiles.

## Phase 1: Direct Questions

Ask **1-2 questions per message**. Be conversational, not form-like. Acknowledge answers before moving on.

### Self Profile Questions

1. "What kinds of products are you most interested in — food, home goods, gadgets, clothing, something else?" → `product_categories`
2. For food: "Sweet or savory? Any flavors you love or can't stand? Dietary restrictions?" → `food_preferences`
3. Per-category follow-ups when they express specific interest:
   - Socks: "What style — no-show, ankle, crew? Material preferences?"
   - Snacks: "Chocolate, chips, jerky? Any brands you love?"
   - Headphones: "Over-ear or buds? What's most important — sound, comfort, noise cancelling?"
4. "Budget vs. premium — do you lean toward deals or splurges?" → `brand_affinity`
5. "Any brands you always go back to, or ones you avoid?" → `brand_affinity.liked/disliked`
6. "What's your weekly shopping budget? And max for a single item?" → `budget`

### Gift Profile Questions

Frame ALL questions about the recipient, not the user. See `references/gift-shopping.md` for the full gift profile setup flow.

## Phase 2: Image Questionnaire (Style Assessment)

Transition: "Let me show you a few images to dial in the style. Thumbs up, thumbs down, or meh on each one."

1. Find 8-12 images via web search using queries from `assets/image-questionnaire-prompts.md`
2. Adapt image categories to Phase 1 answers — no fashion images if they said no clothing, no candy if they said savory only
3. Categories: product aesthetics, packaging/brand style, color palettes, food presentation
4. Present 2-3 images at a time. Record thumbs up/down/meh for each.
5. After the round, summarize the pattern: "Looks like you lean toward [minimalist/warm/bold/etc.] — [specific descriptors]."

**Fallback:** If image search is unavailable or returns unhelpful results, present text descriptions of style archetypes:
- Minimalist (clean lines, neutral tones, Apple-style packaging)
- Warm & cozy (earth tones, rustic materials, handmade feel)
- Bold & colorful (bright packaging, graphic design, statement pieces)
- Eclectic & artisan (unique finds, indie brands, mixed styles)

Ask which resonates most. Use their selection for the `aesthetic` section.

## Phase 3: Confirmation

Present a clean summary:

```
Here's what I have for [you / person's name]:

Categories: [food, home goods, gadgets]
Food: [sweet and savory, loves chocolate and citrus, dislikes coconut]
Brands: [premium, likes Levain and Compartes]
Style: [minimalist, clean lines, neutral tones]
Budget: [$50/week, max $75/item]

Want me to change anything, or should I save this?
```

On confirmation, save to Hermes memory and proceed to product search if applicable.

## Memory Storage

Store as natural-language summaries in Hermes memory. Be specific enough to reconstruct the profile:

> Dave's shopping profile: interested in food/home goods/gadgets, prefers sweet and savory, likes chocolate and citrus, dislikes coconut, premium brands, minimalist style with clean lines, $50/week budget, max $75/item. Liked brands: Levain, Compartes. Socks: no-show, cotton, dark colors, size L.

## Profile Schema (Reference)

Fields the profile should capture:
- **food_preferences**: sweet_vs_savory, flavors_liked, flavors_disliked, dietary
- **product_categories**: interested, not_interested, per-category sub-preferences
- **brand_affinity**: tier (budget/mid-range/premium), liked_brands, disliked_brands, notes
- **aesthetic**: style, colors_liked, colors_disliked, descriptors (from image reactions)
- **budget**: weekly_target, max_single_item

## Profile Management

- **"Update my profile"** → Recall from memory, ask what changed, update, confirm, save
- **"Show my profile"** → Recall and display
- **"Delete my profile"** → Confirm with user, then remove from memory
- **"What profiles do I have?"** → List all profiles stored in memory
- **"I don't like those anymore"** → Treat as update, ask what's different

## Rules

- Always look up existing profiles before starting a new questionnaire
- Always confirm before saving or overwriting
- Ask 1-2 questions per message — never dump all questions at once
- Adapt questions to the specific category
- After any purchase, update `past_purchases` in memory
