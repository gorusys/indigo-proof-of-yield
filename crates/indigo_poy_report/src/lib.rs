//! Static HTML report generation from proof-of-yield evidence bundle.

use indigo_poy::ReportData;
use std::io::Write;
use std::path::Path;

/// Render a static HTML report to `out_path`. Embeds the full report JSON for verification.
pub fn render_report(data: &ReportData, out_path: impl AsRef<Path>) -> Result<(), ReportError> {
    let html = build_html(data)?;
    let mut f = std::fs::File::create(out_path.as_ref()).map_err(ReportError::Io)?;
    f.write_all(html.as_bytes()).map_err(ReportError::Io)?;
    Ok(())
}

/// Build HTML string from report data (for testing or in-memory use).
pub fn build_html(data: &ReportData) -> Result<String, ReportError> {
    let json_embed = serde_json::to_string(&data).map_err(ReportError::Json)?;
    let json_escaped = escape_json_in_html(&json_embed);
    let addr_escaped = escape_html(&data.bundle.address);
    let hash_escaped = escape_html(&data.reproducibility_hash_sha256);

    let metrics = &data.bundle.metrics;
    let sp = &metrics.stability_pool;
    let rob = &metrics.rob;
    let indy = &metrics.indy_staking;
    let comb = &metrics.combined;
    let avg_liq_price = if sp.liquidation_count > 0 {
        let ada = sp.total_liquidations_ada_received_lovelace as f64 / 1_000_000.0;
        format!("{:.2}", ada / sp.liquidation_count as f64)
    } else {
        "—".to_string()
    };
    let rob_avg_pct_snippet = rob
        .avg_premium_pct
        .map(|x| format!("{:.1}%", x))
        .unwrap_or_else(|| "—".to_string());
    let apr_snippet = comb
        .apr_pct
        .map(|x| format!("{:.1}", x))
        .unwrap_or_else(|| "—".to_string());

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Proof of Yield – {addr}</title>
<style>
:root {{ font-family: system-ui, sans-serif; background: #0f1419; color: #e6edf3; }}
body {{ max-width: 720px; margin: 0 auto; padding: 1.5rem; }}
h1 {{ font-size: 1.4rem; margin-bottom: 0.5rem; }}
h2 {{ font-size: 1.1rem; margin-top: 1.5rem; color: #8b949e; }}
.mono {{ font-family: ui-monospace, monospace; font-size: 0.9em; word-break: break-all; }}
.card {{ background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 1rem; margin: 0.5rem 0; }}
.grid {{ display: grid; grid-template-columns: auto 1fr; gap: 0.25rem 1rem; }}
.label {{ color: #8b949e; }}
.hash {{ font-size: 0.85em; }}
.footer {{ margin-top: 2rem; font-size: 0.85rem; color: #8b949e; }}
.snippet {{ font-size: 0.95rem; line-height: 1.5; }}
</style>
</head>
<body>
<h1>Proof of Yield Report</h1>
<p class="mono">{addr}</p>
<p>Generated: {created}</p>

<h2>At a glance</h2>
<div class="card snippet">
  <p><strong>SP:</strong> {sp_count} liquidations, realized premium {apr_snippet}% (annualized), avg ADA liquidation price {avg_liq_price}, dilution-adjusted.</p>
  <p><strong>ROB:</strong> {rob_fill_count} partial fills, reimbursement premium captured {rob_avg_pct_snippet}.</p>
</div>

<h2>Reproducibility</h2>
<div class="card">
  <div class="mono hash">SHA-256: {hash}</div>
  <p class="footer">Anyone can verify this report by re-running <code>indigo-poy verify --bundle &lt;file&gt;</code> and comparing the hash.</p>
</div>

<h2>Summary</h2>
<div class="card">
  <div class="grid">
    <span class="label">Net PnL (lovelace)</span><span class="mono">{net_pnl}</span>
    <span class="label">Total ADA in</span><span class="mono">{total_in}</span>
    <span class="label">Total ADA out</span><span class="mono">{total_out}</span>
    <span class="label">APR %</span><span class="mono">{apr}</span>
  </div>
</div>

<h2>Stability Pool</h2>
<div class="card">
  <div class="grid">
    <span class="label">Deposits (lovelace)</span><span>{sp_deposits}</span>
    <span class="label">Withdrawals (lovelace)</span><span>{sp_withdrawals}</span>
    <span class="label">Liquidations (ADA received)</span><span>{sp_liq}</span>
    <span class="label">Realized premium</span><span>{sp_premium}</span>
    <span class="label">Liquidation count</span><span>{sp_count}</span>
  </div>
</div>

<h2>ROB (Redemption Order Book)</h2>
<div class="card">
  <div class="grid">
    <span class="label">Total placed (lovelace)</span><span>{rob_placed}</span>
    <span class="label">Total filled (lovelace)</span><span>{rob_filled}</span>
    <span class="label">Premium received</span><span>{rob_premium}</span>
    <span class="label">Avg premium %</span><span>{rob_avg_pct}</span>
    <span class="label">Fill count</span><span>{rob_fill_count}</span>
  </div>
</div>

<h2>INDY Staking</h2>
<div class="card">
  <div class="grid">
    <span class="label">Total rewards (lovelace)</span><span>{indy_rewards}</span>
    <span class="label">SP premium (lovelace)</span><span>{indy_sp}</span>
    <span class="label">Reward tx count</span><span>{indy_count}</span>
  </div>
</div>

<h2>Evidence bundle (embedded)</h2>
<div class="card">
  <p class="footer">The full evidence bundle is embedded below for verification. Do not edit.</p>
  <script type="application/json" id="evidence-bundle">{json_embed}</script>
</div>

<div class="footer">
  <p>Generated by <a href="https://github.com/gorusys/indigo-proof-of-yield" style="color:#58a6ff">indigo-proof-of-yield</a>. Read-only tool; no seeds; no signing.</p>
</div>
</body>
</html>"#,
        addr = addr_escaped,
        created = escape_html(&data.bundle.created_utc_rfc3339),
        hash = hash_escaped,
        avg_liq_price = avg_liq_price,
        apr_snippet = apr_snippet,
        rob_avg_pct_snippet = rob_avg_pct_snippet,
        net_pnl = comb.net_pnl_lovelace,
        total_in = comb.total_ada_in_lovelace,
        total_out = comb.total_ada_out_lovelace,
        apr = comb
            .apr_pct
            .map(|x| format!("{:.2}%", x))
            .unwrap_or_else(|| "—".to_string()),
        sp_deposits = sp.total_deposits_lovelace,
        sp_withdrawals = sp.total_withdrawals_lovelace,
        sp_liq = sp.total_liquidations_ada_received_lovelace,
        sp_premium = sp.total_realized_premium_lovelace,
        sp_count = sp.liquidation_count,
        rob_placed = rob.total_placed_lovelace,
        rob_filled = rob.total_filled_lovelace,
        rob_premium = rob.total_premium_received_lovelace,
        rob_avg_pct = rob
            .avg_premium_pct
            .map(|x| format!("{:.2}%", x))
            .unwrap_or_else(|| "—".to_string()),
        rob_fill_count = rob.fill_count,
        indy_rewards = indy.total_rewards_lovelace,
        indy_sp = indy.total_sp_premium_lovelace,
        indy_count = indy.reward_tx_count,
        json_embed = json_escaped,
    );
    Ok(html)
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_json_in_html(s: &str) -> String {
    escape_html(s)
}

#[derive(Debug)]
pub enum ReportError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::Io(e) => write!(f, "io: {}", e),
            ReportError::Json(e) => write!(f, "json: {}", e),
        }
    }
}

impl std::error::Error for ReportError {}

#[cfg(test)]
mod tests {
    use super::*;
    use indigo_poy::{EvidenceBundle, IndigoEvents};

    #[test]
    fn build_html_does_not_panic() {
        let bundle = EvidenceBundle::new(
            "addr1_test".into(),
            vec![],
            vec![],
            vec![],
            IndigoEvents::default(),
            Default::default(),
            vec![],
        );
        let data = ReportData {
            bundle,
            reproducibility_hash_sha256: "a".repeat(64),
        };
        let html = build_html(&data).unwrap();
        assert!(html.contains("Proof of Yield"));
        assert!(html.contains("addr1_test"));
        assert!(html.contains("evidence-bundle"));
    }
}
