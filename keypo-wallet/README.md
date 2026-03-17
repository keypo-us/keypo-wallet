---
title: keypo-wallet (Rust Crate + CLI)
owner: "@davidblumenfeld"
last_verified: 2026-03-05
status: current
---

# keypo-wallet

Rust crate and CLI for EIP-7702 smart account setup, ERC-4337 bundler interaction, and on-chain queries. Shells out to `keypo-signer` for P-256 signing via the Secure Enclave.

## Key Modules

| Module | Purpose |
|---|---|
| `account.rs` | EIP-7702 setup flow |
| `transaction.rs` | UserOp construction + ERC-7821 execution |
| `bundler.rs` | ERC-7769 bundler client |
| `signer.rs` | P-256 signer trait + MockSigner |
| `config.rs` | 4-tier config resolution |
| `query.rs` | Balance queries, output formatting |
| `paymaster.rs` | ERC-7677 paymaster client |
| `state.rs` | Account state persistence (`~/.keypo/accounts.json`) |

## Build and Test

```bash
cargo check
cargo test
cargo clippy --all-targets -- -D warnings

# Integration tests (requires .env + Base Sepolia)
cargo test -- --ignored --test-threads=1
```

## References

- [Root CLAUDE.md](../CLAUDE.md) — repo map and conventions summary
- [Coding conventions](../docs/conventions.md) — alloy API rules, signing rules, gotchas
- [Architecture overview](../docs/architecture.md)
- [Full specification](../docs/archive/specs/keypo-wallet-spec.md)
