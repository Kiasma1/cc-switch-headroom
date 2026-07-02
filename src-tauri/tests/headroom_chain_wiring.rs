use cc_switch_lib::proxy::types::AppProxyConfig;
use cc_switch_lib::ProxyService;

#[test]
fn app_proxy_config_default_compression_disabled() {
    let cfg = AppProxyConfig {
        app_type: "claude".to_string(),
        enabled: true,
        ..Default::default()
    };
    assert!(!cfg.compression_enabled, "默认 compression_enabled 应为 false");
}

#[test]
fn claude_base_url_returns_headroom_when_compression_on() {
    let cfg = AppProxyConfig {
        app_type: "claude".to_string(),
        enabled: true,
        compression_enabled: true,
        ..Default::default()
    };
    let url = ProxyService::claude_base_url(&cfg, "http://127.0.0.1:15721");
    assert_eq!(url, "http://127.0.0.1:8787");
}

#[test]
fn claude_base_url_returns_proxy_when_compression_off() {
    let cfg = AppProxyConfig {
        app_type: "claude".to_string(),
        enabled: true,
        compression_enabled: false,
        ..Default::default()
    };
    let url = ProxyService::claude_base_url(&cfg, "http://127.0.0.1:15721");
    assert_eq!(url, "http://127.0.0.1:15721");
}
