# Keypo Checkout Demo

Ask Claude Code to buy something from a Shopify store. Your credit card details
stay locked in the Keypo vault — the agent never sees them. When it's time to
pay, Touch ID appears on your Mac and you approve the transaction with your
fingerprint.

## Store

This demo purchases real products from `shop.keypo.io` — **you will be charged**.

| | |
|---|---|
| **Store** | `shop.keypo.io` |
| **Keypo Logo Art** | [shop.keypo.io/products/keypo-logo-art](https://shop.keypo.io/products/keypo-logo-art?variant=44740698996759) ($1.00) |
| **Bot bought - Garment washed baseball cap** | [shop.keypo.io/products/bot-bought-garment-washed-baseball-cap](https://shop.keypo.io/products/bot-bought-garment-washed-baseball-cap?variant=44741188059159) ($30.00) |
| **YubiKey 5C NFC Security Key** | [shop.keypo.io/products/yubikey-5c-nfc-security-key](https://shop.keypo.io/products/yubikey-5c-nfc-security-key?variant=44741031985175) ($60.00) |

The checkout logic is generic Shopify — to point it at a different store,
just change the product URL.

## How It Works

You say something like **"Buy the Keypo Logo Art"** in Claude Code. The agent
creates a checkout task, then runs a wrapper script that calls
`keypo-signer vault exec`. You're prompted by Touch ID to decrypt your card details, 
which are then injected into a headless browser process to complete the
Shopify checkout. You get an order confirmation email from Shopify.

```
┌─────────────────────────────────────────────┐
│  Claude Code (agent)                        │
│                                             │
│  "Buy the Keypo Logo Art"                   │
│       │                                     │
│       ▼                                     │
│  ./run-with-vault.sh <TASK_ID>              │
│       │                                     │
│  (waits for exit code + reads stdout)       │
│  (sees only status messages, never secrets) │
└───────┬─────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────┐
│  Child process (invisible to agent)         │
│                                             │
│  keypo-signer vault exec                    │
│       │                                     │
│       ├── open tier (no auth)               │
│       │   PORT, DB_*, NODE_ENV              │
│       │                                     │
│       ├── biometric tier (Touch ID)         │
│       │   CARD_NUMBER, NAME_ON_CARD, …      │
│       │                                     │
│       ▼                                     │
│  Secrets injected into process.env          │
│       │                                     │
│       ▼                                     │
│  node start-task.js                         │
│  └── Puppeteer (headless browser)           │
│      └── Shopify checkout                   │
│          └── Types card into payment fields │
└─────────────────────────────────────────────┘
```

Claude Code cannot inspect the child process. The agent sees the process's
stdout (status messages like "Entering card details") and its exit code —
nothing else. The secret values exist only inside the child process.

## Setup

### Prerequisites

- **macOS** with Apple Silicon (Secure Enclave required)
- **[keypo-signer](https://github.com/keypo-us/keypo-wallet/tree/main/keypo-signer)** installed with vault initialized
- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** installed
- **Node 18** (via [nvm](https://github.com/nvm-sh/nvm))
- **PostgreSQL** (via Homebrew or Docker)

### 1. Clone the repo

```bash
git clone --recurse-submodules https://github.com/keypo-us/keypo-wallet.git
cd keypo-wallet/demo/checkout
```

If you already have the repo, make sure the submodule is initialized:

```bash
git submodule update --init demo/checkout/bot
```

### 2. Start PostgreSQL

Using Homebrew:

```bash
brew install postgresql@14
brew services start postgresql@14
```

Or using Docker:

```bash
docker compose up -d
```

### 3. Create the database

```bash
createdb checkout_demo
psql checkout_demo -c "CREATE USER checkout WITH PASSWORD 'localdev';"
psql checkout_demo -c "GRANT ALL ON DATABASE checkout_demo TO checkout;"
psql checkout_demo -c "GRANT ALL ON SCHEMA public TO checkout;"
```

> If using Docker, the database is created automatically — skip this step.

### 4. Install dependencies

```bash
cd bot
nvm install 18
nvm use 18
npm install
```

### 5. Run migrations and seed data

```bash
NODE_ENV=local npx knex migrate:latest
NODE_ENV=local npx knex seed:run
cd ..
```

### 6. Initialize the vault

If you haven't already set up the Keypo vault:

```bash
keypo-signer vault init
```

Now import your secrets. Create two temporary files:

**.env.open** — non-sensitive config (open tier, no auth required):
```
PORT=8080
DB_USERNAME=checkout
DB_PASSWORD=localdev
DB_NAME=checkout_demo
DB_PORT=5432
DB_HOST=localhost
NODE_ENV=local
```

**.env.card** — card details (biometric tier, Touch ID required).
Replace the placeholders with your actual card details:
```
CARD_NUMBER=<your card number>
NAME_ON_CARD=<name as printed on card>
EXPIRATION_MONTH=<MM>
EXPIRATION_YEAR=<YY>
SECURITY_CODE=<CVV>
```

> The biometric vault ensures card details can only be accessed with Touch ID.

Import them into the vault:

```bash
keypo-signer vault import .env.open --vault open
keypo-signer vault import .env.card --vault biometric
```

Delete the temporary files — your secrets are now in the vault:

```bash
rm .env.open .env.card
```

Verify everything is stored:

```bash
keypo-signer vault list
# Should show 7 secrets in "open" and 5 secrets in "biometric"
```

### 7. Lock down the code

This prevents the agent from modifying the checkout scripts to exfiltrate
your card details. See [Tamper protection](#tamper-protection) for why this
matters.

```bash
sudo chown -R root:wheel run-with-vault.sh bot/
sudo chmod -R a+rX,go-w run-with-vault.sh bot/
```

### 8. Try it

Open Claude Code in the project directory:

```bash
claude
```

Then ask it to buy something:

```
> Buy the Keypo Logo Art
```

Claude Code reads `SKILL.md` and follows it automatically — it starts
Postgres and the API server, discovers products from the store, creates
a checkout task, and runs `run-with-vault.sh`. Touch ID will appear —
authenticate, and the agent completes the purchase. You'll get an order
confirmation email from Shopify.

## Usage

The `SKILL.md` file in this directory is a Claude Code skill definition.
When Claude Code is launched from this directory, it reads the skill and
knows how to orchestrate the full checkout flow — starting services,
discovering products, creating tasks, and running the vault-protected
checkout. You just ask it what to buy.

Once set up, the typical flow is:

1. Open Claude Code in this directory
2. Ask it to buy a product (e.g., "Buy the Keypo Logo Art")
3. Approve with Touch ID when prompted
4. Check your email for the order confirmation

Claude Code handles starting and stopping services. If you want to shut
things down manually:

```bash
# Stop the API server
lsof -ti:8080 | xargs kill

# Stop Postgres (Homebrew)
brew services stop postgresql@14

# Or stop Postgres (Docker)
docker compose down
```

## Security Model

Your card details are protected by three layers:

1. **Biometric vault** — card secrets are encrypted with a Secure Enclave key
   that requires Touch ID to access. The agent cannot decrypt them without your
   fingerprint.

2. **Process isolation** — `vault exec` injects secrets into the child process
   environment only. They are never written to disk, logged, or returned to the
   agent. The agent sees stdout from the checkout process, which only contains
   status messages ("Entering card details"), not the values themselves.

3. **Tamper protection** — the wrapper script and bot code are root-owned, so
   the agent cannot modify them. See below.

### Tamper protection

An AI agent with file-write access could theoretically modify the checkout
scripts to exfiltrate secrets from `process.env` at runtime. There are three
files in the trust chain that must be protected:

1. **`run-with-vault.sh`** — pins the child command to `node ./scripts/start-task.js`.
   Without this, the agent could call `vault exec` with an arbitrary command
   like `env` or `bash -c 'echo $CARD_NUMBER'` and read secrets from stdout.
2. **`bot/scripts/start-task.js`** — the entry point that runs inside `vault exec`
   with secrets in `process.env`. If modified, it could log secrets before
   executing checkout logic.
3. **`bot/`** (all JS files) — any file in the execution chain (`cluster.js`,
   `shopify.js`, etc.) has access to `process.env` and could be modified to
   exfiltrate values.

Lock all three down by setting root ownership:

```bash
sudo chown -R root:wheel run-with-vault.sh bot/
sudo chmod -R a+rX,go-w run-with-vault.sh bot/
```

This works because Claude Code runs as your user and cannot modify root-owned
files. The agent can still *execute* `run-with-vault.sh` (it's world-readable
and executable), but cannot alter the wrapper, the entry point, or any code in
the execution chain.

When you need to update the bot code, temporarily reclaim ownership:

```bash
sudo chown -R $(whoami) run-with-vault.sh bot/
# ... make changes ...
sudo chown -R root:wheel run-with-vault.sh bot/
sudo chmod -R a+rX,go-w run-with-vault.sh bot/
```

## Files

| File | Purpose |
|---|---|
| `run-with-vault.sh` | Wrapper: `vault exec --env` → checkout process |
| `.env.vault-template` | Key-name manifest for `vault exec --env` |
| `SKILL.md` | Claude Code agent skill definition |
| `docker-compose.yml` | Postgres-only compose (alternative to Homebrew) |
| `seed-data/` | Address and site reference data |
| `bot/` | Checkout bot (based on [SneakerBot](https://github.com/samc621/SneakerBot)) |

## Next Steps

This demo handles checkout — the agent completes a purchase for a product you
specify. A natural next step is **product discovery**: let the agent browse and
find products before buying them.

Shopify now offers MCP servers purpose-built for this:

- **[Storefront MCP](https://shopify.dev/docs/agents/catalog/storefront-mcp)** —
  search products, manage carts, and answer policy questions for a single store.
  Each Shopify store exposes its own MCP endpoint.
- **[Catalog MCP](https://shopify.dev/docs/agents/catalog/mcp)** — search
  products across all Shopify merchants globally. An agent could find the best
  price for a product across hundreds of millions of listings.

Combining Shopify's discovery MCP with this demo's vault-protected checkout
would give an agent the full shopping loop — find a product, then buy it with
Touch ID approval — without ever handling card data.

## Credits

The checkout automation is built on [SneakerBot](https://github.com/samc621/SneakerBot)
by [Samuel Corso](https://github.com/samc621), modified for modern Shopify
single-page checkout and headless operation.
