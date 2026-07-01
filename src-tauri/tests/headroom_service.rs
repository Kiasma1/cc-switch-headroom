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

use std::time::Duration;

/// 需要本机 ~/.headroom-venv/Scripts/headroom.exe。手动运行：
/// cargo test --test headroom_service real_headroom_start_health_stop -- --ignored --nocapture
#[test]
#[ignore]
fn real_headroom_start_health_stop() {
    use cc_switch_lib::HeadroomStatus;

    let home = PathBuf::from(std::env::var("USERPROFILE").expect("USERPROFILE 环境变量"));
    let exe = home.join(".headroom-venv").join("Scripts").join("headroom.exe");
    assert!(exe.exists(), "本测试需要已安装的 headroom.exe: {}", exe.display());

    let cfg = HeadroomConfig {
        exe_path: exe,
        // 用非默认端口，避免撞上正在运行的 8787
        port: 18799,
        // 指向一个不必存在的上游：本测试只验证进程起停与 /livez，不发真实请求
        upstream_url: "http://127.0.0.1:15721".to_string(),
    };
    let mgr = HeadroomManager::new(cfg, std::env::temp_dir().join("hr-real-it.log"));

    mgr.start().expect("启动 headroom");

    // 轮询等待就绪（最多 ~30s，冷启动含 ML 模型加载）
    let mut ready = false;
    for _ in 0..30 {
        if mgr.status() == HeadroomStatus::Running {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    assert!(ready, "headroom 未在 30s 内就绪");

    mgr.stop().expect("停止 headroom");
    std::thread::sleep(Duration::from_secs(2));
    assert_ne!(mgr.status(), HeadroomStatus::Running, "停止后不应仍在运行");
}
