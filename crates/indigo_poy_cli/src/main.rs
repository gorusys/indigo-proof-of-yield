//! indigo-poy CLI: fetch, compute, report, verify.

use clap::{Parser, Subcommand};
use indigo_poy::chain::{Cache, FetchConfig, Fetcher};
use indigo_poy::compute::{compute_metrics, ComputeInput};
use indigo_poy::indigo::reconstruct_all_events;
use indigo_poy::report::ReportData;
use indigo_poy::verify::{reproducibility_hash, EvidenceBundle, VerificationResult};
use indigo_poy_report::render_report;
use std::collections::HashMap;
use std::path::PathBuf;
use time::OffsetDateTime;
use tracing::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();
    let cli = Cli::parse();
    match cli.command {
        Command::Fetch(args) => run_fetch(args),
        Command::Compute(args) => run_compute(args),
        Command::Report(args) => run_report(args),
        Command::Verify(args) => run_verify(args),
    }
}

#[derive(Parser)]
#[command(name = "indigo-poy")]
#[command(author = "gorusys <goru.connector@outlook.com>")]
#[command(about = "Proof of yield for Indigo Protocol (Stability Pool, ROB, INDY staking)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch on-chain data for an address and cache it.
    Fetch(FetchArgs),
    /// Compute metrics from fetched/cached data.
    Compute(ComputeArgs),
    /// Generate HTML report and bundle.
    Report(ReportArgs),
    /// Verify a bundle's reproducibility hash.
    Verify(VerifyArgs),
}

#[derive(Parser)]
struct FetchArgs {
    #[arg(long)]
    address: String,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value = "./data/cache")]
    cache_dir: PathBuf,
    #[arg(long)]
    offline: bool,
}

#[derive(Parser)]
struct ComputeArgs {
    #[arg(long)]
    address: String,
    #[arg(long)]
    since_last_claim: bool,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value = "./data/cache")]
    cache_dir: PathBuf,
    #[arg(long)]
    offline: bool,
}

#[derive(Parser)]
struct ReportArgs {
    #[arg(long)]
    address: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value = "./reports")]
    reports_dir: PathBuf,
    #[arg(long, default_value = "./data/cache")]
    cache_dir: PathBuf,
    #[arg(long)]
    offline: bool,
    /// Generate a demo report with example metrics (for screenshots / Discord pitch).
    #[arg(long)]
    demo: bool,
}

#[derive(Parser)]
struct VerifyArgs {
    #[arg(long)]
    bundle: PathBuf,
}

fn cache_path(cache_dir: &std::path::Path) -> PathBuf {
    cache_dir.join("cache.sqlite")
}

fn run_fetch(args: FetchArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cache = Cache::open(cache_path(&args.cache_dir))?;
    let config = FetchConfig {
        offline: args.offline,
        ..Default::default()
    };
    let fetcher = Fetcher::new(config, Some(cache))?;
    let rt = tokio::runtime::Runtime::new()?;
    let txs = rt.block_on(async {
        fetcher
            .account_txs(&args.address, args.from.as_deref(), args.to.as_deref())
            .await
    })?;
    info!(count = txs.len(), "fetched account_txs");
    for tx in &txs {
        let _ = rt.block_on(async { fetcher.tx_utxos(&tx.tx_hash).await });
    }
    info!(requests = fetcher.request_count(), "fetch complete");
    Ok(())
}

fn run_compute(args: ComputeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cache = Cache::open(cache_path(&args.cache_dir))?;
    let config = FetchConfig {
        offline: args.offline,
        ..Default::default()
    };
    let fetcher = Fetcher::new(config, Some(cache))?;
    let rt = tokio::runtime::Runtime::new()?;
    let from = args.from.as_deref();
    let to = args.to.as_deref();
    let txs = rt.block_on(async { fetcher.account_txs(&args.address, from, to).await })?;
    let tx_utxos: HashMap<String, _> = rt.block_on(async {
        let mut map = HashMap::new();
        for tx in &txs {
            if let Ok(u) = fetcher.tx_utxos(&tx.tx_hash).await {
                map.insert(tx.tx_hash.clone(), u);
            }
        }
        map
    });
    let get_tx_utxos = |hash: &str| tx_utxos.get(hash).cloned();
    let now = OffsetDateTime::now_utc();
    let events = reconstruct_all_events(&txs, get_tx_utxos, now);
    let period_start = txs.iter().filter_map(|t| t.block_time).min();
    let period_end = txs.iter().filter_map(|t| t.block_time).max();
    let input = ComputeInput {
        events: events.clone(),
        period_start_ts: period_start,
        period_end_ts: period_end,
        current_ada_position: None,
    };
    let metrics = compute_metrics(&input);
    let tx_hashes: Vec<String> = txs.iter().map(|t| t.tx_hash.clone()).collect();
    let mut sorted_hashes = tx_hashes.clone();
    sorted_hashes.sort();
    let bundle = EvidenceBundle::new(
        args.address.clone(),
        sorted_hashes,
        vec![],
        vec![],
        events,
        metrics,
        txs.iter().filter_map(|t| t.slot_no).collect(),
    );
    let hash = reproducibility_hash(&bundle)?;
    let reports_dir = PathBuf::from("./reports");
    std::fs::create_dir_all(&reports_dir)?;
    let addr_suffix = args
        .address
        .chars()
        .take(20)
        .collect::<String>()
        .replace([' ', ':'], "_");
    let bundle_path = reports_dir.join(format!("{}.bundle.json", addr_suffix));
    let hash_path = reports_dir.join(format!("{}.sha256", addr_suffix));
    std::fs::write(&bundle_path, serde_json::to_string_pretty(&bundle)?)?;
    std::fs::write(&hash_path, format!("{}\n", hash))?;
    info!(?bundle_path, ?hash_path, "compute complete");
    println!("{}", hash);
    Ok(())
}

fn run_report(args: ReportArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.demo {
        return run_report_demo(&args);
    }
    let cache = Cache::open(cache_path(&args.cache_dir))?;
    let config = FetchConfig {
        offline: args.offline,
        ..Default::default()
    };
    let fetcher = Fetcher::new(config, Some(cache))?;
    let rt = tokio::runtime::Runtime::new()?;
    let txs = rt.block_on(async { fetcher.account_txs(&args.address, None, None).await })?;
    let tx_utxos: HashMap<String, _> = rt.block_on(async {
        let mut map = HashMap::new();
        for tx in &txs {
            if let Ok(u) = fetcher.tx_utxos(&tx.tx_hash).await {
                map.insert(tx.tx_hash.clone(), u);
            }
        }
        map
    });
    let get_tx_utxos = |hash: &str| tx_utxos.get(hash).cloned();
    let now = OffsetDateTime::now_utc();
    let events = reconstruct_all_events(&txs, get_tx_utxos, now);
    let period_start = txs.iter().filter_map(|t| t.block_time).min();
    let period_end = txs.iter().filter_map(|t| t.block_time).max();
    let input = ComputeInput {
        events: events.clone(),
        period_start_ts: period_start,
        period_end_ts: period_end,
        current_ada_position: None,
    };
    let metrics = compute_metrics(&input);
    let mut sorted_hashes: Vec<String> = txs.iter().map(|t| t.tx_hash.clone()).collect();
    sorted_hashes.sort();
    let bundle = EvidenceBundle::new(
        args.address.clone(),
        sorted_hashes,
        vec![],
        vec![],
        events,
        metrics,
        txs.iter().filter_map(|t| t.slot_no).collect(),
    );
    let reproducibility_hash_sha256 = reproducibility_hash(&bundle)?;
    let data = ReportData {
        bundle,
        reproducibility_hash_sha256: reproducibility_hash_sha256.clone(),
    };
    std::fs::create_dir_all(&args.reports_dir)?;
    let addr_suffix = args
        .address
        .chars()
        .take(20)
        .collect::<String>()
        .replace([' ', ':'], "_");
    let html_path = args
        .out
        .unwrap_or_else(|| args.reports_dir.join(format!("{}.html", addr_suffix)));
    let bundle_path = args
        .reports_dir
        .join(format!("{}.bundle.json", addr_suffix));
    let hash_path = args.reports_dir.join(format!("{}.sha256", addr_suffix));
    render_report(&data, &html_path)?;
    std::fs::write(&bundle_path, serde_json::to_string_pretty(&data.bundle)?)?;
    std::fs::write(&hash_path, format!("{}\n", reproducibility_hash_sha256))?;
    info!(?html_path, ?bundle_path, ?hash_path, "report complete");
    Ok(())
}

fn run_report_demo(args: &ReportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bundle = EvidenceBundle::demo();
    let reproducibility_hash_sha256 = reproducibility_hash(&bundle)?;
    let data = ReportData {
        bundle,
        reproducibility_hash_sha256: reproducibility_hash_sha256.clone(),
    };
    std::fs::create_dir_all(&args.reports_dir)?;
    let html_path = args
        .out
        .clone()
        .unwrap_or_else(|| args.reports_dir.join("demo.html"));
    let bundle_path = args.reports_dir.join("demo.bundle.json");
    let hash_path = args.reports_dir.join("demo.sha256");
    render_report(&data, &html_path)?;
    std::fs::write(&bundle_path, serde_json::to_string_pretty(&data.bundle)?)?;
    std::fs::write(&hash_path, format!("{}\n", reproducibility_hash_sha256))?;
    info!(?html_path, ?bundle_path, ?hash_path, "demo report complete");
    println!("Demo report written to {}", html_path.display());
    Ok(())
}

fn run_verify(args: VerifyArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_json = std::fs::read_to_string(&args.bundle)?;
    let bundle: EvidenceBundle = serde_json::from_str(&bundle_json)?;
    let computed = reproducibility_hash(&bundle)?;
    let sha256_path = args
        .bundle
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(format!(
            "{}.sha256",
            args.bundle
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        ));
    let expected = std::fs::read_to_string(sha256_path)
        .ok()
        .map(|s| s.trim().to_string());
    let result = if let Some(ref exp) = expected {
        VerificationResult {
            bundle_hash: computed.clone(),
            expected_hash: Some(exp.clone()),
            matches: computed.to_lowercase() == exp.to_lowercase(),
        }
    } else {
        VerificationResult {
            bundle_hash: computed.clone(),
            expected_hash: None,
            matches: false,
        }
    };
    if result.matches {
        println!("OK\t{}", result.bundle_hash);
    } else {
        eprintln!(
            "MISMATCH\tcomputed={}\texpected={:?}",
            result.bundle_hash, result.expected_hash
        );
        std::process::exit(1);
    }
    Ok(())
}
