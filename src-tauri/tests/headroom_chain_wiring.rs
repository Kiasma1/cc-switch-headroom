use cc_switch_lib::proxy::types::AppProxyConfig;

#[test]
fn app_proxy_config_default_compression_disabled() {
    let cfg = AppProxyConfig {
        app_type: "claude".to_string(),
        enabled: true,
        ..Default::default()
    };
    assert!(!cfg.compression_enabled, "默认 compression_enabled 应为 false");
}
