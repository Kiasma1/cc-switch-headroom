//! Headroom 压缩代理进程的生命周期管理。
//!
//! 移植自 Go 版 tray-tool 的 Proxy/ports 逻辑。本模块自包含，
//! 不依赖数据库或 AppState，便于独立测试。

use std::fs::OpenOptions;
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use crate::error::AppError;

const CREATE_NO_WINDOW: u32 = 0x08000000;

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

/// TCP 连接探测端口是否有监听。对应 ports.go isPortOpen。
pub fn is_port_open(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    matches!(addr.parse(), Ok(sockaddr)
        if TcpStream::connect_timeout(&sockaddr, Duration::from_secs(1)).is_ok())
}

/// 返回监听该端口的进程 PID 字符串；找不到返回 None。
/// 仅用于判断端口归属，绝不据此直接 taskkill。对应 ports.go pidOnPort。
pub fn pid_on_port(port: u16) -> Option<String> {
    let output = Command::new("cmd")
        .args(["/c", "netstat -ano -p tcp"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let want = format!(":{port}");
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        if !fields[0].eq_ignore_ascii_case("TCP") || !fields[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        if fields[1].ends_with(&want) {
            return Some(fields[fields.len() - 1].to_string());
        }
    }
    None
}

/// 用 PowerShell 查询指定 PID 的命令行。对应 ports.go commandLineForPID。
pub fn command_line_for_pid(pid: &str) -> String {
    if pid.is_empty() {
        return String::new();
    }
    let ps = format!(
        "(Get-CimInstance Win32_Process -Filter \"ProcessId={pid}\").CommandLine"
    );
    let output = match Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(o) => o,
        Err(_) => return String::new(),
    };
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// 探测 Headroom 的 /livez 端点是否返回 2xx。
/// 用于判定进程是否已就绪 / 仍存活。
pub fn health_check(port: u16, timeout: Duration) -> bool {
    let url = format!("http://127.0.0.1:{port}/livez");
    let client = match reqwest::blocking::Client::builder().timeout(timeout).build() {
        Ok(c) => c,
        Err(_) => return false,
    };
    matches!(client.get(&url).send(), Ok(resp) if resp.status().is_success())
}

/// Headroom 进程的观测状态。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadroomStatus {
    Stopped,
    Running,
    PortConflict,
    Failed,
}

struct ManagerState {
    child: Option<Child>,
    owned_pid: Option<u32>,
    last_error: Option<String>,
}

/// Headroom 进程的生命周期管理器。自包含，可独立构造与测试。
pub struct HeadroomManager {
    config: HeadroomConfig,
    log_path: PathBuf,
    state: Mutex<ManagerState>,
}

impl HeadroomManager {
    pub fn new(config: HeadroomConfig, log_path: PathBuf) -> Self {
        Self {
            config,
            log_path,
            state: Mutex::new(ManagerState {
                child: None,
                owned_pid: None,
                last_error: None,
            }),
        }
    }

    /// 启动 Headroom 进程。若端口已被"我们的"进程占用则视为接管成功。
    /// 若被陌生进程占用则返回 PortConflict 错误，不强杀。
    pub fn start(&self) -> Result<(), AppError> {
        // exe 存在性预检
        if !self.config.exe_path.exists() {
            let msg = format!("找不到 headroom.exe: {}", self.config.exe_path.display());
            self.set_error(&msg);
            return Err(AppError::Config(msg));
        }

        // 端口归属判断
        if is_port_open("127.0.0.1", self.config.port) {
            let pid = pid_on_port(self.config.port).unwrap_or_default();
            let cmdline = command_line_for_pid(&pid);
            if self.config.cmdline_matches(&cmdline) {
                // 已有匹配进程 —— 接管
                let mut st = self.state.lock()?;
                st.owned_pid = pid.parse().ok();
                st.last_error = None;
                return Ok(());
            }
            let msg = format!("端口 {} 被其他进程占用 (PID {})", self.config.port, pid);
            self.set_error(&msg);
            return Err(AppError::Message(msg));
        }

        // 打开日志文件（追加）
        if let Some(parent) = self.log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| AppError::io(&self.log_path, e))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| AppError::io(&self.log_path, e))?;

        // spawn
        let child = Command::new(&self.config.exe_path)
            .args(self.config.build_args())
            .env("HEADROOM_MODE", "token")
            .env("HEADROOM_PORT", self.config.port.to_string())
            .stdout(log_file)
            .stderr(log_file_err)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| {
                let msg = format!("启动失败: {e}");
                self.set_error(&msg);
                AppError::Message(msg)
            })?;

        let mut st = self.state.lock()?;
        st.owned_pid = Some(child.id());
        st.child = Some(child);
        st.last_error = None;
        Ok(())
    }

    /// 停止我们启动/接管的 Headroom 进程树。
    /// 只终止确认归属我们的进程，绝不误杀陌生 PID。
    pub fn stop(&self) -> Result<(), AppError> {
        let pid = {
            let st = self.state.lock()?;
            st.owned_pid
        };

        if let Some(pid) = pid {
            kill_process_tree(pid);
        }

        // 二次清理：若启动器已退出但监听子/孙进程仍在，按端口匹配再清一次
        if let Some(port_pid) = pid_on_port(self.config.port) {
            let cmdline = command_line_for_pid(&port_pid);
            if self.config.cmdline_matches(&cmdline) {
                if let Ok(p) = port_pid.parse::<u32>() {
                    kill_process_tree(p);
                }
            }
        }

        let mut st = self.state.lock()?;
        if let Some(mut child) = st.child.take() {
            let _ = child.wait();
        }
        st.owned_pid = None;
        st.last_error = None;
        Ok(())
    }

    fn set_error(&self, msg: &str) {
        if let Ok(mut st) = self.state.lock() {
            st.last_error = Some(msg.to_string());
        }
    }

    /// 计算当前观测状态。
    pub fn status(&self) -> HeadroomStatus {
        // /livez 通了即认定 Running
        if health_check(self.config.port, Duration::from_secs(1)) {
            return HeadroomStatus::Running;
        }
        // 端口被占用但非我们的进程 → 冲突
        if is_port_open("127.0.0.1", self.config.port) {
            let pid = pid_on_port(self.config.port).unwrap_or_default();
            let cmdline = command_line_for_pid(&pid);
            if !self.config.cmdline_matches(&cmdline) {
                return HeadroomStatus::PortConflict;
            }
        }
        // 有记录的错误 → Failed
        if let Ok(st) = self.state.lock() {
            if st.last_error.is_some() {
                return HeadroomStatus::Failed;
            }
        }
        HeadroomStatus::Stopped
    }

    /// 返回最近一次错误信息（供前端展示）。
    pub fn last_error(&self) -> Option<String> {
        self.state.lock().ok().and_then(|st| st.last_error.clone())
    }
}

/// taskkill /T /F 终止进程树。对应 tray-tool 的 kill := exec.Command("taskkill"...)。
fn kill_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
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

    use std::net::TcpListener;

    #[test]
    fn is_port_open_true_when_listening_false_when_not() {
        // 绑定一个临时端口
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(is_port_open("127.0.0.1", port), "监听中应为 open");
        drop(listener);
        // 端口释放后应为 closed（给系统一点时间）
        std::thread::sleep(Duration::from_millis(200));
        assert!(!is_port_open("127.0.0.1", port), "释放后应为 closed");
    }

    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener as StdTcpListener;
    use std::thread;

    #[test]
    fn health_check_true_on_200_false_on_no_server() {
        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
                );
            }
        });
        assert!(health_check(port, Duration::from_secs(2)), "200 应判为健康");
        let _ = handle.join();

        // 无服务器的端口
        let dead = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let dead_port = dead.local_addr().unwrap().port();
        drop(dead);
        assert!(!health_check(dead_port, Duration::from_millis(500)), "无服务应判为不健康");
    }

    #[test]
    fn start_fails_when_exe_missing() {
        let cfg = HeadroomConfig {
            exe_path: PathBuf::from(r"C:\does\not\exist\headroom.exe"),
            port: 8787,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        };
        let mgr = HeadroomManager::new(cfg, PathBuf::from(std::env::temp_dir()).join("hr-test.log"));
        let err = mgr.start().unwrap_err();
        match err {
            AppError::Config(m) => assert!(m.contains("找不到 headroom.exe")),
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn stop_is_idempotent_when_nothing_running() {
        let cfg = HeadroomConfig {
            exe_path: PathBuf::from(r"C:\does\not\exist\headroom.exe"),
            // 用一个几乎不可能被占用的高位端口，避免误触真实进程
            port: 59787,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        };
        let mgr = HeadroomManager::new(cfg, std::env::temp_dir().join("hr-test.log"));
        // 从未 start，stop 不应 panic 也不应报错
        mgr.stop().expect("stop 在无进程时应成功");
    }

    #[test]
    fn status_stopped_when_nothing_on_port() {
        let cfg = HeadroomConfig {
            exe_path: PathBuf::from(r"C:\does\not\exist\headroom.exe"),
            port: 59788,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        };
        let mgr = HeadroomManager::new(cfg, std::env::temp_dir().join("hr-test.log"));
        assert_eq!(mgr.status(), HeadroomStatus::Stopped);
    }
}
