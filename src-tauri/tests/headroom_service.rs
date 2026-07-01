use cc_switch_lib::{HeadroomConfig, HeadroomManager};
use std::path::PathBuf;

#[test]
fn manager_reports_stopped_for_free_port() {
    let cfg = HeadroomConfig {
        exe_path: PathBuf::from(r"C:\does\not\exist\headroom.exe"),
        port: 59790,
        upstream_url: "http://127.0.0.1:15721".to_string(),
    };
    let mgr = HeadroomManager::new(cfg, std::env::temp_dir().join("hr-it.log"));
    let json = serde_json::to_string(&mgr.status()).unwrap();
    assert_eq!(json, "\"stopped\"");
}
