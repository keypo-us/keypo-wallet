# Vault Concepts

Background on Keypo's hardware-secured vault system and why the agent never sees card data.

## Vault Tiers

Keypo stores encrypted secrets in three tiers:

| Tier | Authentication | Use Case |
|------|---------------|----------|
| `open` | None (device unlocked) | Non-sensitive config |
| `passcode` | Device passcode | Moderate sensitivity |
| `biometric` | Touch ID / Apple Watch | Payment cards, PII |

## How Secrets Are Injected

- Secrets are encrypted at rest using Secure Enclave P-256 keys
- `vault exec` decrypts secrets and injects them as **environment variables** into a child process
- The child process (checkout.js) reads secrets from its environment
- After the child exits, the environment variables are gone — never written to disk

## Secure Enclave Properties

- Private keys **never leave** the Secure Enclave hardware
- Biometric authentication is enforced by hardware, not software
- Even root access cannot extract SE private keys or bypass biometric requirements
- Each `vault exec` invocation gets its own biometric prompt — authentication doesn't leak across processes

## Three-Way Separation

| Component | Role | What It Sees |
|-----------|------|-------------|
| **Agent** (Hermes) | Finds products, presents options, gets user approval | Product URLs, prices, user preferences. **Never** card data. |
| **Daemon** (keypo-signer) | Gates vault access, enforces biometric | Request manifests, biometric results. **Never** makes purchasing decisions. |
| **Checkout script** (checkout.js) | Fills checkout forms, submits payment | Card data (via env vars), product URLs. **Never** sees user conversation. |

## Why Prompt Injection Can't Extract Card Data

The agent process does NOT have vault environment variables. Card data only exists inside the checkout.js child process, which runs in a separate process with no connection to the agent's conversation. There is no tool, command, or prompt that can bridge this gap — the separation is architectural, not policy-based.
