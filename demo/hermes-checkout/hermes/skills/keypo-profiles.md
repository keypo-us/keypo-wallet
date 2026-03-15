---
name: keypo-profiles
description: "TRIGGER ON ANY PURCHASE INTENT — whenever the user says they want to buy something. Step 1: always look up if a profile exists for the item/recipient. Step 2: if profile exists, use it for recommendations (offer to update if preferences changed). Step 3: if no profile, run the questionnaire. Handles both product category profiles (socks, snacks, gadgets) and gift/recipient profiles (mom, friend)."
version: 2.0.0
author: Keypo
metadata:
  hermes:
    tags: [shopping, profiles, taste, preferences, personalization, gifts, keypo]
    category: keypo
    requires_toolsets: [keypo]
    requires_tools: []
---

# Keypo Profiles — Taste Profiling & Preference Management

## CRITICAL: Always Do This First

**Whenever the user expresses intent to buy ANYTHING**, before searching for products or asking questions:

1. Check Hermes memory for an existing profile matching the item or recipient
2. Search past sessions with `session_search` for relevant purchase history or preferences
3. Then follow the decision tree below

This applies to ALL purchases — gifts for people AND product categories for self (socks, snacks, gadgets, etc.).

---

## Decision Tree

### Case 1: Profile exists and seems current
→ Say "I have a profile for [item/person] from before — [brief summary]. Let me find options based on that."
→ Proceed directly to product search using profile preferences
→ After showing results, ask "Still what you're looking for, or has anything changed?"

### Case 2: Profile exists but user says preferences have changed
→ "No problem — let's update it. What's changed?"
→ Run a targeted update questionnaire (only ask about what needs updating, not the whole thing)
→ Confirm changes, save updated profile to memory

### Case 3: No profile found
→ "I don't have a profile for [item/person] yet — let me ask a few questions so I can find the best option."
→ Run the appropriate questionnaire (product category or gift recipient)
→ Confirm and save profile, then proceed to product search

---

## Profile Types

### Type 1: Product Category Profile (self-purchases)

Used when the user wants to buy something for themselves (socks, snacks, headphones, etc.).

```
profile_type: "product_category"
category: "<e.g., socks, snacks, headphones>"
preferences:
  style: [list]           # e.g., ["no-show", "ankle"] for socks
  material: [list]        # e.g., ["cotton", "merino wool"]
  colors: [list]          # e.g., ["dark", "neutral", "black"]
  size: "<if applicable>"
  liked_attributes: [list]
  disliked_attributes: [list]
brand_affinity:
  tier: "budget" | "mid-range" | "premium" | "mixed"
  liked_brands: [list]
  disliked_brands: [list]
budget:
  target: <number>
  max: <number>
  currency: "USD"
past_purchases: []        # populated after purchases: [{date, product, store, price}]
```

### Type 2: Gift Recipient Profile

Used when buying a gift for a specific person (mom, friend, partner, etc.).

```
profile_type: "gift"
name: "<recipient's name>"
relationship: "<e.g., mother, friend, partner>"
birthday: "YYYY-MM-DD"
birthday_reminder_days: <number>   # default: 14
food_preferences:
  sweet_vs_savory: "sweet" | "savory" | "both"
  flavors_liked: [list]
  flavors_disliked: [list]
  dietary: [list]
product_categories:
  interested: [list]
  not_interested: [list]
brand_affinity:
  tier: "budget" | "mid-range" | "premium" | "mixed"
  liked_brands: [list]
  disliked_brands: [list]
  preference_notes: "<free text>"
aesthetic:
  style: "<e.g., minimalist, warm, bold>"
  colors_liked: [list]
  colors_disliked: [list]
  descriptors: [list]
gift_budget:
  target: <number>
  max: <number>
  currency: "USD"
past_gifts: []            # populated after purchases: [{date, product, store, price}]
```

### Type 3: General Self Profile

A broad profile covering the user's overall shopping preferences. Populated over time as category profiles accumulate, or via explicit "set up my profile" request.

```
profile_type: "self"
name: "<user's name>"
product_categories:
  interested: [list]
  not_interested: [list]
brand_affinity:
  tier: "budget" | "mid-range" | "premium" | "mixed"
  liked_brands: [list]
  disliked_brands: [list]
aesthetic:
  style: "<e.g., minimalist, warm, bold>"
  colors_liked: [list]
  colors_disliked: [list]
  descriptors: [list]
budget:
  weekly_target: <number>
  max_single_item: <number>
  currency: "USD"
```

---

## Questionnaire Procedure

Ask **1-2 questions per message**. Feel conversational, not form-like. Acknowledge answers before moving on ("Got it, no-show cotton in dark colors").

### Product Category Questionnaire

Adapt questions to the specific category. Examples for "socks":

1. "What style do you usually go for — no-show, ankle, crew, knee-high?" → `style`
2. "Any material preferences? Like cotton, wool, bamboo?" → `material`
3. "Color-wise — do you go mostly dark/neutral, or do you like patterns and colors?" → `colors`
4. "Any brands you've loved or hated in the past?" → `brand_affinity`
5. "What's your budget per pair (or per pack)?" → `budget`

For food/snack categories, add flavor/dietary questions. For electronics, add feature/spec questions. Always adapt to the category.

### Gift Recipient Questionnaire

1. "What's her name?" (if not already known) → `name`
2. "When's her birthday?" → `birthday` (and set reminder 14 days before by default)
3. "What kinds of things does she like — hobbies, interests, things she collects?" → `product_categories`
4. "Does she have any dietary restrictions or food preferences?" (if relevant) → `food_preferences`
5. "What's her style like — minimalist, cozy, bold, eclectic?" → `aesthetic`
6. "What's your budget for her gift?" → `gift_budget`

### Image Questionnaire (Style Assessment)

For gift profiles and general self profiles, after the text questions run an image round:

Transition: "Let me show you a few images to dial in the style. Thumbs up, thumbs down, or meh on each one."

Find 6-10 images via web search across:
- Product aesthetic (minimalist vs. rustic vs. colorful)
- Packaging/brand style (sleek vs. handmade vs. bold graphic)
- Color palettes (earth tones vs. pastels vs. monochrome vs. neon)

**Always adapt images to stated interests** — no fashion images if they said no clothing, no candy if they said savory only.

After the image round, summarize the pattern and let the user confirm or correct.

### Confirmation

After all questions, present a clean summary:

```
Here's what I have for [item/person]:

Style: no-show, cotton, dark colors
Brands: open to trying new ones, budget to mid-range
Budget: up to $20/pack

Want me to change anything, or should I start searching?
```

User confirms or edits. On confirmation, save to memory and proceed to product search.

---

## Memory Storage

Store as natural-language summaries. Be specific enough to reconstruct the profile accurately.

**Product category example:**
```
David's socks profile: prefers no-show and ankle styles, cotton or merino wool, dark/neutral colors, size L. Budget $15-25/pack. Liked: Bombas. Past purchases: Bombas ankle 3-pack from shop.bombas.com, $35, 2026-03-15.
```

**Gift recipient example:**
```
Gift profile for Mom (David's mother): birthday April 1, remind 14 days before. Likes home decor, gardening, cooking. Food: sweet, likes vanilla and berry, dislikes coffee, no dietary restrictions. Style: warm and cozy, likes warm tones and florals. Mid-range budget, appreciates handmade/artisan items. Gift budget: $50 target, $75 max, USD. Past gifts: none yet.
```

---

## Profile Management

- **"Update [item/person]'s profile"** → Recall from memory, ask what changed, update, confirm, save
- **"I don't like those anymore" / "my taste has changed"** → Treat as profile update, ask what's different
- **"Show me my socks profile"** → Recall and display
- **"Delete [item/person]'s profile"** → Confirm with user, then remove from memory
- **"What profiles do I have?"** → List all profiles stored in memory

---

## Pitfalls — CRITICAL

- **ALWAYS look up existing profiles BEFORE asking questions or searching products** — never skip this step
- **NEVER** ask for or display card/payment data
- **ALWAYS** confirm before saving or overwriting a profile
- **ALWAYS** ask 1-2 questions per message — never dump all questions at once
- For gift profiles, frame ALL questions about the recipient, not the user
- Default `birthday_reminder_days` to 14 if user doesn't specify
- Adapt questionnaire questions to the specific category — don't ask about flavor for gadgets
- After any purchase, update the profile's `past_purchases` or `past_gifts` list in memory
