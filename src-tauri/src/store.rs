use crate::config::get_home_dir;
use crate::database::Database;
use crate::services::{HeadroomConfig, HeadroomManager, ProxyService, UsageCache};
use std::sync::Arc;

/// 全局应用状态
pub struct AppState {
    pub db: Arc<Database>,
    pub proxy_service: ProxyService,
    pub usage_cache: Arc<UsageCache>,
    pub headroom_manager: Arc<HeadroomManager>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(db: Arc<Database>) -> Self {
        let proxy_service = ProxyService::new(db.clone());

        let home = get_home_dir();
        let headroom_cfg = HeadroomConfig {
            exe_path: home.join(".headroom-venv").join("Scripts").join("headroom.exe"),
            port: 9749,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        };
        let headroom_log = home.join(".headroom").join("logs").join("claude-proxy.log");
        let headroom_manager = Arc::new(HeadroomManager::new(headroom_cfg, headroom_log));

        // 将同一个 Headroom 管理器注入到 ProxyService，使 set_compression_for_app 可以编排 Headroom 生命周期。
        proxy_service.set_headroom_manager(headroom_manager.clone());

        Self {
            db,
            proxy_service,
            usage_cache: Arc::new(UsageCache::new()),
            headroom_manager,
        }
    }
}
