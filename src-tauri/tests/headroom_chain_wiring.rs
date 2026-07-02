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

/// 验证 headroom spawn 源码中设 ANTHROPIC_BASE_URL=:15721 防回环。
/// 本测试用 grep 方式检查源码不变量（真实进程测试在子系统1 的 real_headroom_start_health_stop 覆盖）。
#[test]
#[ignore]
fn headroom_spawn_sets_anthropic_base_url_env() {
    let src = std::fs::read_to_string("src/services/headroom.rs").unwrap();
    assert!(
        src.contains(r#"ANTHROPIC_BASE_URL"#) && src.contains("127.0.0.1:15721"),
        "headroom spawn 必须设 ANTHROPIC_BASE_URL=http://127.0.0.1:15721 防回环"
    );
}
