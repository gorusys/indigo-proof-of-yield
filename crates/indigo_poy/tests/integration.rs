//! Integration tests using saved Koios-like fixtures.

use indigo_poy::compute::{compute_metrics, ComputeInput};
use indigo_poy::indigo::reconstruct_all_events;
use indigo_poy::verify::{reproducibility_hash, EvidenceBundle};
use std::collections::HashMap;
use std::path::Path;

fn load_fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let full = root.join(path);
    let s =
        std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("read {}: {}", full.display(), e));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse {}: {}", path, e))
}

#[test]
fn integration_fixture_account_txs_parse() {
    let txs: Vec<indigo_poy::KoiosAccountTx> = load_fixture("account_txs.json");
    assert_eq!(txs.len(), 2);
    assert_eq!(txs[0].tx_hash, "abc123def456");
    assert_eq!(txs[0].slot_no, Some(100000));
}

#[test]
fn integration_fixture_tx_utxos_parse() {
    let utxos: indigo_poy::KoiosTxUtxos = load_fixture("tx_utxos_abc123.json");
    let inputs = utxos.inputs.as_ref().unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].value, "5000000");
    assert!(inputs[0].asset_list.as_ref().unwrap().len() == 1);
}

#[test]
fn integration_reconstruct_from_fixtures() {
    let txs: Vec<indigo_poy::KoiosAccountTx> = load_fixture("account_txs.json");
    let utxos_abc: indigo_poy::KoiosTxUtxos = load_fixture("tx_utxos_abc123.json");
    let mut map: HashMap<String, _> = HashMap::new();
    map.insert("abc123def456".to_string(), utxos_abc);
    let get = |h: &str| map.get(h).cloned();
    let now = time::OffsetDateTime::from_unix_timestamp(1700000000).unwrap();
    let events = reconstruct_all_events(&txs, get, now, None);
    assert!(!events.stability_pool.is_empty() || events.rob.is_empty());
}

#[test]
fn integration_bundle_hash_deterministic() {
    let bundle = EvidenceBundle::new(
        "addr1".to_string(),
        vec!["tx1".into(), "tx2".into()],
        vec![],
        vec![],
        indigo_poy::IndigoEvents::default(),
        Default::default(),
        vec![100, 101],
    );
    let h1 = reproducibility_hash(&bundle).unwrap();
    let h2 = reproducibility_hash(&bundle).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn integration_compute_from_events() {
    let input = ComputeInput {
        events: indigo_poy::IndigoEvents::default(),
        period_start_ts: Some(0),
        period_end_ts: Some(365 * 24 * 3600),
        current_ada_position: Some(1_000_000),
    };
    let m = compute_metrics(&input);
    assert_eq!(m.combined.total_ada_in_lovelace, 0);
    assert!(m.combined.apr_pct.is_some());
}
