# indigo-proof-of-yield

**Author:** gorusys · **Contact:** goru.connector@outlook.com

A local-first Rust tool for [Indigo Protocol](https://indigoprotocol.io/) users that reconstructs **realized outcomes** from on-chain data—no seeds, no signing, read-only.

## What it does

From Cardano chain data (via [Koios](https://koios.rest/) or cached JSON), the tool:

1. **Stability Pool** — Liquidation events, iAsset burnt, ADA received, realized premium, dilution effects.
2. **ROB (Redemption Order Book)** — Order placement, fills, premiums/reimbursement %, cooldown where inferable.
3. **INDY staking** — Rewards vs SP premium vs other flows, as far as on-chain data allows.

It produces:

- A **deterministic JSON evidence bundle** (tx hashes, UTxOs, datum hashes, policy IDs, timestamps, API response hashes).
- A **shareable static HTML report** built from that JSON.
- A **reproducibility hash** (SHA-256) over normalized inputs + outputs so anyone can verify the report.

## Why it matters

- **Self-custody** — You keep full control; the tool only reads public chain data.
- **Proof** — Evidence bundle + hash let you (or an auditor) re-run and confirm the same result.
- **Clarity** — One place to see SP liquidations, ROB fills, and INDY-related flows for an address.

## Quickstart

### Build and run (local)

```bash
cargo build --release
./target/release/indigo-poy --help
```

### Commands

```bash
# Fetch and cache on-chain data for an address (and optional slot/time range)
indigo-poy fetch --address <addr> [--from <slot_or_rfc3339>] [--to <slot_or_rfc3339>] [--cache-dir ./data/cache]

# Compute metrics from cached (or live) data; write bundle + .sha256 to ./reports
indigo-poy compute --address <addr> [--since-last-claim] [--offline] [--cache-dir ./data/cache]

# Generate HTML report (and bundle/sha256 if not already present)
indigo-poy report --address <addr> [--out ./reports/<addr>.html] [--reports-dir ./reports] [--offline]

# Verify a bundle against its .sha256 file
indigo-poy verify --bundle ./reports/<addr>.bundle.json
```

### Offline / reproducibility

- Use `--offline` to rely only on previously fetched data in `--cache-dir` (default `./data/cache`).
- All fetched data is cached in SQLite under the cache dir (content-hash keys).
- For full reproducibility, run with the same cache and same CLI args; the bundle hash should match.

### Docker

```bash
docker build -t indigo-poy .
docker run --rm -v $(pwd)/data:/data -v $(pwd)/reports:/reports indigo-poy fetch --address <addr>
docker run --rm -v $(pwd)/data:/data -v $(pwd)/reports:/reports indigo-poy report --address <addr> --cache-dir /data/cache --reports-dir /reports
```

## How to interpret the report

*(For non-developers.)*

- **Reproducibility hash** — A long hex string (SHA-256). If someone else runs the tool on the same address and range with the same data, they should get the same hash; that means the report is reproducible and not tampered with.
- **Summary** — “Net PnL” is total ADA out minus total ADA in over the period. “APR %” is an annualized return estimate based on that PnL and the time window.
- **Stability Pool** — Deposits (you put in ADA/iAsset), withdrawals (you took out), and **liquidations**: when the protocol burns iAsset and sends ADA to the pool; “ADA received” and “realized premium” are your share of that.
- **ROB** — “Placed” is ADA you committed to redemption orders; “filled” is what was actually redeemed; “premium” is the extra you received above face value.
- **INDY staking** — Rewards and any SP premium attributed to your address in the window.

Interpretation limits: the tool infers events from UTxO shapes and tx patterns. It does not replace the official Indigo UIs or docs; use it as an on-chain evidence and summary aid.

## Privacy and safety

- **Read-only** — The tool never signs transactions and never asks for seeds or private keys.
- **Local-first** — Fetched data is stored in a local SQLite cache; you choose when to hit the API and when to work offline.
- **No telemetry** — No data is sent except to the Cardano API (e.g. Koios) when you run fetch without `--offline`.

## Limitations

- Event reconstruction is **best-effort** from public chain data (tx structure, UTxOs, datum hashes). Some Indigo-specific logic (exact script hashes, cooldown rules) may not be fully reflected.
- APR and dilution are **estimates** from the reconstructed events and the chosen time window.
- Koios (or the configured API) is rate-limited and may be unavailable; use cache and `--offline` for reliability.

## Development

```bash
cargo fmt
cargo clippy --all-targets   # warnings denied in workspace
cargo test
```

## License

MIT OR Apache-2.0.
