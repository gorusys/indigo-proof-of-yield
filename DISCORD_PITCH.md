# How to pitch Indigo Proof-of-Yield in Indigo Discord

Post **Message 1** in #general or #dev, then **immediately** follow with **Message 2** (screenshot + snippet + link + command).

---

## Message 1 (paste in Discord)

I built a small Rust tool: **Indigo Proof-of-Yield** — it reconstructs what actually happened to your wallet in Indigo (SP liquidations/premium, ROB fills, realized APR vs displayed, dilution over time) and outputs a shareable HTML report + a reproducible "proof manifest".

If you drop a wallet address (or DM isn’t needed—just paste an address publicly if you’re ok), I can generate a report so we can validate the math against the UI.

---

## Message 2 (post right after)

1. **Screenshot** of the HTML report summary page  
   - Generate it locally:  
     `indigo-poy report --address demo --demo`  
   - Open `./reports/demo.html` in a browser and take a screenshot (include the "At a glance" + Summary + Stability Pool + ROB sections).

2. **Short snippet** (paste as plain text under the screenshot):

```
SP: 23 liquidations, realized premium 9.7% (annualized), avg ADA liquidation price 0.49, dilution-adjusted.
ROB: 4 partial fills, reimbursement premium captured 1%.
```

3. **Link + one command**:

**Repo:** https://github.com/gorusys/indigo-proof-of-yield

**Single command to run (after clone + build):**
```bash
cargo run --release -- report --address <YOUR_ADDRESS>
```
Or with Docker:
```bash
docker run --rm -v $(pwd)/data:/data -v $(pwd)/reports:/reports indigo-poy report --address <YOUR_ADDRESS> --cache-dir /data/cache --reports-dir /reports
```

---

## Why this works

It turns the "idea" into **proof**, and gives the core team something concrete to review without debate. The report is deterministic (SHA-256 over inputs + outputs), so anyone can re-run and verify.
