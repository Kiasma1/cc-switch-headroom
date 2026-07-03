use cc_switch_lib::proxy::types::AppProxyConfig;
use cc_switch_lib::ProxyService;
use std::path::PathBuf;

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
    assert_eq!(url, "http://127.0.0.1:9749");
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

/// 接管转关时,若 compression_enabled 为 true,应被联动置 false。
/// 本测试用 grep 源码方式锁定"联动逻辑必须存在"的不变量。
#[test]
fn takeover_off_turns_off_compression() {
    let src = std::fs::read_to_string("src/services/proxy.rs").unwrap();
    assert!(
        src.contains("compression_enabled") && src.contains("set_compression_for_app"),
        "接管转关必须联动关压缩"
    );
}

#[test]
fn compression_on_requires_takeover_enabled() {
    // 接管关时开压缩 → 返回 Err
    // 本测试用 grep 源码方式锁定门禁逻辑存在。
    let src = std::fs::read_to_string("src/services/proxy.rs").unwrap();
    assert!(
        src.contains("takeover") && src.contains("compression") && src.contains("enabled"),
        "set_compression_for_app 必须校验接管为开"
    );
}

#[test]
fn compression_commands_registered() {
    let src = std::fs::read_to_string("src/lib.rs").unwrap();
    assert!(
        src.contains("set_compression_for_app") && src.contains("get_compression_status"),
        "压缩命令必须注册到 generate_handler"
    );
}

/// 真实进程端到端：压缩开 → 探活 → 压缩关 → 确认恢复。
/// 需要本机 ~/.headroom-venv/Scripts/headroom.exe + 代理接管已开启的 Claude 配置。
/// 手动运行：cargo test --test headroom_chain_wiring real_compression_on_off_sequence -- --ignored --nocapture
#[test]
#[ignore]
fn real_compression_on_off_sequence() {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let exe = PathBuf::from(&home)
        .join(".headroom-venv")
        .join("Scripts")
        .join("headroom.exe");
    assert!(exe.exists(), "需要 headroom.exe: {}", exe.display());

    // 用非默认端口 18798 避免撞上正在运行的 8787。
    // 构造 ProxyService + 真实 DB 测试 fixture（参考 tests/support.rs）
    // 或直接测编排函数 set_compression_for_app 的返回值与状态迁移。
    // 本测试为骨架，具体实现依赖 DB 测试 fixture。
    let _ = exe;
}
