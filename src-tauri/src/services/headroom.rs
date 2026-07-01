//! Headroom 压缩代理进程的生命周期管理。
//!
//! 移植自 Go 版 tray-tool 的 Proxy/ports 逻辑。本模块自包含，
//! 不依赖数据库或 AppState，便于独立测试。

use std::path::PathBuf;

/// Headroom 进程的启动配置。
#[derive(Debug, Clone)]
pub struct HeadroomConfig {
    /// headroom.exe 的完整路径。
    pub exe_path: PathBuf,
    /// 本地监听端口（默认 8787）。
    pub port: u16,
    /// 上游地址：写死指向 cc-switch 代理（http://127.0.0.1:15721）。
    pub upstream_url: String,
}

impl HeadroomConfig {
    /// 构造传给 headroom.exe 的命令行参数（不含 exe 本身）。
    ///
    /// 对应 tray-tool proxy.go 的 exec.Command 参数。
    pub fn build_args(&self) -> Vec<String> {
        vec![
            "proxy".to_string(),
            "--port".to_string(),
            self.port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--anthropic-api-url".to_string(),
            self.upstream_url.clone(),
            "--no-subscription-tracking".to_string(),
        ]
    }

    /// 判断给定的进程命令行是否属于"我们的" Headroom 代理。
    ///
    /// 全部条件满足才算匹配，任一不满足即视为陌生进程，禁止终止。
    /// 对应 tray-tool proxy.go 的 pidMatchesProxy。
    pub fn cmdline_matches(&self, cmdline: &str) -> bool {
        let port_arg = format!("--port {}", self.port);
        cmdline.contains("headroom.exe")
            && cmdline.contains(" proxy ")
            && cmdline.contains(&port_arg)
            && cmdline.contains("--anthropic-api-url")
            && cmdline.contains(&self.upstream_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> HeadroomConfig {
        HeadroomConfig {
            exe_path: PathBuf::from(r"C:\Users\me\.headroom-venv\Scripts\headroom.exe"),
            port: 8787,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        }
    }

    #[test]
    fn build_args_contains_port_host_and_upstream() {
        let args = sample_config().build_args();
        assert_eq!(args[0], "proxy");
        // 端口成对出现
        let port_idx = args.iter().position(|a| a == "--port").unwrap();
        assert_eq!(args[port_idx + 1], "8787");
        // 上游成对出现且指向 cc-switch 代理
        let up_idx = args.iter().position(|a| a == "--anthropic-api-url").unwrap();
        assert_eq!(args[up_idx + 1], "http://127.0.0.1:15721");
        // 绑定回环
        let host_idx = args.iter().position(|a| a == "--host").unwrap();
        assert_eq!(args[host_idx + 1], "127.0.0.1");
    }

    #[test]
    fn cmdline_matches_our_headroom() {
        let cfg = sample_config();
        let cmd = r"headroom.exe proxy --port 8787 --host 127.0.0.1 --anthropic-api-url http://127.0.0.1:15721";
        assert!(cfg.cmdline_matches(cmd));
    }

    #[test]
    fn cmdline_rejects_different_port() {
        let cfg = sample_config();
        let cmd = r"headroom.exe proxy --port 9999 --host 127.0.0.1 --anthropic-api-url http://127.0.0.1:15721";
        assert!(!cfg.cmdline_matches(cmd));
    }

    #[test]
    fn cmdline_rejects_stranger_process() {
        let cfg = sample_config();
        // 端口相同但不是 headroom —— 绝不能误判为我们的进程
        let cmd = r"python.exe -m http.server 8787";
        assert!(!cfg.cmdline_matches(cmd));
    }

    #[test]
    fn cmdline_rejects_wrong_upstream() {
        let cfg = sample_config();
        let cmd = r"headroom.exe proxy --port 8787 --anthropic-api-url https://api.anthropic.com";
        assert!(!cfg.cmdline_matches(cmd));
    }
}
