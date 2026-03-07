---
name: uniswap-v3-swap
description: Use when the user wants to swap tokens on Base Sepolia (or Base mainnet) using Uniswap V3. Handles any ERC-20 token pair including ETH wrapping/unwrapping. The agent discovers pools, gets quotes, constructs calldata, and executes swaps via keypo-wallet. Also use when the user says "swap", "trade", "exchange tokens", "buy USDC", "sell WETH", or asks about Uniswap liquidity or pricing on Base. Requires Foundry (cast) for read calls and keypo-wallet for transaction execution.
license: MIT
metadata:
  author: keypo-us
  version: "0.1.0"
  compatibility: Requires Foundry (cast) and keypo-wallet. Works on Base Sepolia and Base mainnet.
---

# Uniswap V3 Swap — Generalized Token Swaps on Base

Swap any ERC-20 token pair on Uniswap V3. This skill teaches the agent to **discover** the right pool, **quote** the expected output, **construct** the swap calldata, and **execute** via keypo-wallet — for any token pair, not just predetermined ones.

---

## Contract Addresses

### Base Sepolia (chain 84532)

| Contract | Address |
|----------|---------|
| V3 Factory | `0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24` |
| SwapRouter | `0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4` |
| QuoterV2 | `0xC5290058841028F1614F3A6F0F5816cAd0df5E27` |
| WETH | `0x4200000000000000000000000000000000000006` |

### Base Mainnet (chain 8453)

| Contract | Address |
|----------|---------|
| V3 Factory | `0x33128a8fC17869897dcE68Ed026d694621f6FDfD` |
| SwapRouter | `0x2626664c2603336E57B271c5C0b26F421741e481` |
| QuoterV2 | `0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a` |
| WETH | `0x4200000000000000000000000000000000000006` |

### Known Testnet Tokens (Base Sepolia)

| Token | Address | Decimals |
|-------|---------|----------|
| USDC | `0x036CbD53842c5426634e7929541eC2318f3dCF7e` | 6 |
| WETH | `0x4200000000000000000000000000000000000006` | 18 |

For tokens not listed here, ask the user for the contract address, or use the `contract-learner` skill to look up a token by address.

---

## Swap Workflow

Follow these steps in order. Each step is a separate `cast call` or `cast calldata` command. Do not skip steps — the agent must gather information before constructing the transaction.

### Step 1: Resolve Token Addresses and Decimals

Determine the `tokenIn` and `tokenOut` contract addresses. If the user says "ETH", use the WETH address — Uniswap V3 only works with ERC-20 tokens, not native ETH.

For each token, confirm its decimals:

```bash
cast call <token-address> "decimals()(uint8)" --rpc-url https://sepolia.base.org
```

You need decimals to convert human-readable amounts (e.g. "0.001 ETH", "50 USDC") to raw integers:
- 0.001 ETH → `1000000000000000` (0.001 × 10^18)
- 50 USDC → `50000000` (50 × 10^6)

If you don't know a token's address, ask the user. Do not guess token addresses.

### Step 2: Find a Pool

Uniswap V3 pools are keyed by `(tokenA, tokenB, fee)`. The fee tiers are:

| Fee | Basis Points | Typical Use |
|-----|-------------|-------------|
| `100` | 0.01% | Stablecoin pairs |
| `500` | 0.05% | Stable/major pairs |
| `3000` | 0.3% | Most pairs |
| `10000` | 1% | Exotic/volatile pairs |

Query the factory for each fee tier until you find a pool (non-zero address):

```bash
# Try 500 first (most common for major pairs)
cast call <FACTORY> "getPool(address,address,uint24)(address)" <tokenIn> <tokenOut> 500 --rpc-url https://sepolia.base.org

# If zero, try 3000
cast call <FACTORY> "getPool(address,address,uint24)(address)" <tokenIn> <tokenOut> 3000 --rpc-url https://sepolia.base.org

# If zero, try 10000
cast call <FACTORY> "getPool(address,address,uint24)(address)" <tokenIn> <tokenOut> 10000 --rpc-url https://sepolia.base.org

# If zero, try 100
cast call <FACTORY> "getPool(address,address,uint24)(address)" <tokenIn> <tokenOut> 100 --rpc-url https://sepolia.base.org
```

A return value of `0x0000000000000000000000000000000000000000` means no pool exists at that fee tier. If no pool exists at any fee tier, tell the user there is no Uniswap V3 liquidity for this pair on this chain.

**On testnets, liquidity may be very thin or nonexistent for many pairs.** Common testnet pairs with liquidity: WETH/USDC.

### Step 3: Check Pool Liquidity

Once you find a pool address, verify it has liquidity:

```bash
cast call <pool-address> "liquidity()(uint128)" --rpc-url https://sepolia.base.org
```

If liquidity is `0`, the pool exists but has no liquidity — the swap will fail. Tell the user and try the next fee tier.

### Step 4: Get a Quote

Use QuoterV2 to simulate the swap and get the expected output amount. This is a read-only call — no gas needed.

```bash
# For exact input (you know how much you're putting in):
cast call <QUOTER_V2> \
  "quoteExactInputSingle((address,address,uint256,uint24,uint160))(uint256,uint160,uint32,uint256)" \
  "(<tokenIn>,<tokenOut>,<amountIn>,<fee>,0)" \
  --rpc-url https://sepolia.base.org
```

The tuple parameter order is: `(tokenIn, tokenOut, amountIn, fee, sqrtPriceLimitX96)`. Set `sqrtPriceLimitX96` to `0` for no price limit.

The return values are: `(amountOut, sqrtPriceX96After, initializedTicksCrossed, gasEstimate)`.

**Convert `amountOut` to human-readable** using the output token's decimals and show it to the user before executing. For example: "Swapping 0.001 WETH → expected ~2.34 USDC. Proceed?"

If the quote call reverts, the pool likely has insufficient liquidity for the requested amount. Try a smaller amount or a different fee tier.

### Step 5: Check Balance and Allowance

Before executing, verify the wallet has enough of the input token:

```bash
# Check tokenIn balance
cast call <tokenIn> "balanceOf(address)(uint256)" <wallet-address> --rpc-url https://sepolia.base.org

# Check current allowance for the SwapRouter
cast call <tokenIn> "allowance(address,address)(uint256)" <wallet-address> <SWAP_ROUTER> --rpc-url https://sepolia.base.org
```

If the balance is insufficient, tell the user. If the allowance is less than `amountIn`, you'll need to approve in the same batch (see Step 6).

### Step 6: Construct and Execute the Swap

#### Case A: TokenIn is an ERC-20 (already have the token)

If allowance is sufficient, just swap:

```bash
SWAP_DATA=$(cast calldata \
  "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))" \
  "(<tokenIn>,<tokenOut>,<fee>,<recipient>,<amountIn>,<amountOutMinimum>,0)")

keypo-wallet send --key <key-name> --to <SWAP_ROUTER> --data $SWAP_DATA
```

If allowance is insufficient, approve + swap in one batch:

```bash
APPROVE_DATA=$(cast calldata "approve(address,uint256)" <SWAP_ROUTER> <amountIn>)

SWAP_DATA=$(cast calldata \
  "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))" \
  "(<tokenIn>,<tokenOut>,<fee>,<recipient>,<amountIn>,<amountOutMinimum>,0)")

echo "[
  {\"to\": \"<tokenIn>\", \"value\": \"0\", \"data\": \"$APPROVE_DATA\"},
  {\"to\": \"<SWAP_ROUTER>\", \"value\": \"0\", \"data\": \"$SWAP_DATA\"}
]" | keypo-wallet batch --key <key-name> --calls -
```

#### Case B: Swapping from native ETH

Native ETH must be wrapped to WETH first. Then approve + swap:

```bash
DEPOSIT_DATA=$(cast calldata "deposit()")

APPROVE_DATA=$(cast calldata "approve(address,uint256)" <SWAP_ROUTER> <amountIn>)

SWAP_DATA=$(cast calldata \
  "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))" \
  "(<WETH>,<tokenOut>,<fee>,<recipient>,<amountIn>,<amountOutMinimum>,0)")

echo "[
  {\"to\": \"<WETH>\", \"value\": \"<amountIn>\", \"data\": \"$DEPOSIT_DATA\"},
  {\"to\": \"<WETH>\", \"value\": \"0\", \"data\": \"$APPROVE_DATA\"},
  {\"to\": \"<SWAP_ROUTER>\", \"value\": \"0\", \"data\": \"$SWAP_DATA\"}
]" | keypo-wallet batch --key <key-name> --calls -
```

The `deposit()` call wraps ETH → WETH. The `value` field on the deposit call must equal `amountIn` (in wei). All three calls execute atomically.

#### Case C: Swapping to native ETH

Swap tokenIn → WETH, then unwrap. Set the swap `recipient` to the wallet's own address, then unwrap:

```bash
SWAP_DATA=$(cast calldata \
  "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))" \
  "(<tokenIn>,<WETH>,<fee>,<wallet-address>,<amountIn>,<amountOutMinimum>,0)")

WITHDRAW_DATA=$(cast calldata "withdraw(uint256)" <expectedWethOut>)

echo "[
  {\"to\": \"<tokenIn>\", \"value\": \"0\", \"data\": \"$APPROVE_DATA\"},
  {\"to\": \"<SWAP_ROUTER>\", \"value\": \"0\", \"data\": \"$SWAP_DATA\"},
  {\"to\": \"<WETH>\", \"value\": \"0\", \"data\": \"$WITHDRAW_DATA\"}
]" | keypo-wallet batch --key <key-name> --calls -
```

### Step 7: Verify the Result

After the transaction succeeds, check the output token balance:

```bash
keypo-wallet balance --key <key-name> --token <tokenOut>
```

Or for native ETH:

```bash
keypo-wallet balance --key <key-name>
```

---

## exactInputSingle Parameters

The `exactInputSingle` function takes a tuple with these fields in order:

| Field | Type | Description |
|-------|------|-------------|
| `tokenIn` | `address` | Input token contract address |
| `tokenOut` | `address` | Output token contract address |
| `fee` | `uint24` | Pool fee tier (100, 500, 3000, or 10000) |
| `recipient` | `address` | Address to receive output tokens (usually the wallet itself) |
| `amountIn` | `uint256` | Amount of input token (raw integer, adjusted for decimals) |
| `amountOutMinimum` | `uint256` | Minimum acceptable output (set to ~95-98% of quoted amount for slippage protection) |
| `sqrtPriceLimitX96` | `uint160` | Price limit — set to `0` for no limit |

**Note:** The SwapRouter on Base Sepolia uses the V3 SwapRouter (not SwapRouter02). Its `exactInputSingle` does NOT include a `deadline` parameter in the tuple — the deadline is handled differently on this deployment. If you get encoding errors, check the router's ABI with `cast interface`.

---

## Slippage Protection

Never set `amountOutMinimum` to `0` in production — this allows any output amount including near-zero (sandwich attack). Calculate a reasonable minimum:

```bash
# 3% slippage tolerance on testnet
python3 -c "
quoted = <amountOut-from-step-4>
min_out = int(quoted * 0.97)
print(min_out)
"
```

On testnet, 3-5% slippage is reasonable due to thin liquidity. On mainnet, 0.5-1% is typical.

---

## Multi-Hop Swaps

If no direct pool exists for your pair but both tokens have pools with a common intermediary (usually WETH), you can route through multiple pools using `exactInput`:

```bash
# Example: TokenA → WETH → TokenB
# Path is encoded as: tokenA + fee1 + WETH + fee2 + tokenB (packed bytes)

# Encode the path
PATH=$(python3 -c "
tokenA = '<tokenA-address>'[2:]  # remove 0x
fee1 = '<fee1-hex>'  # e.g. '0001f4' for 500
weth = '<WETH-address>'[2:]
fee2 = '<fee2-hex>'
tokenB = '<tokenB-address>'[2:]
print('0x' + tokenA + fee1 + weth + fee2 + tokenB)
")

SWAP_DATA=$(cast calldata \
  "exactInput((bytes,address,uint256,uint256))" \
  "($PATH,<recipient>,<amountIn>,<amountOutMinimum>)")
```

Fee hex encoding: `100` → `000064`, `500` → `0001f4`, `3000` → `000bb8`, `10000` → `002710`.

Multi-hop is more complex. Only use it when no direct pool exists. Always try direct pools first.

---

## Common Issues

**"Pool not found"** — No pool exists at any fee tier for this pair on this chain. On testnets, many pairs have no liquidity. Suggest the user try a different pair or check mainnet.

**Quote reverts** — The pool exists but doesn't have enough liquidity for the requested amount. Try a smaller amount.

**Swap reverts with "STF"** — "Safe Transfer From" failed. The approval didn't go through, or the wallet doesn't have enough tokens. Check allowance and balance.

**Swap reverts with "TF"** — "Transfer Failed" on output. Rare — usually means the output token has transfer restrictions.

**Wrong decimals** — Always call `decimals()` on both tokens. Getting this wrong sends the wrong amount (potentially 10^12x off).

**"Too little received"** — `amountOutMinimum` was set too high relative to actual pool price. This can happen if the quote is stale. Re-quote and use a slightly larger slippage tolerance.

---

## Security Notes

- Always show the user the expected output amount and get confirmation before executing swaps.
- Never set `amountOutMinimum` to `0` except on testnets with trivial amounts.
- Verify token addresses — do not guess or hallucinate addresses. Use known addresses from this skill or ask the user.
- On testnets, pools may have extremely thin liquidity. Large swaps will have high slippage.
- The approve amount should match `amountIn` exactly, not `type(uint256).max`, to limit exposure if the router contract is compromised.
