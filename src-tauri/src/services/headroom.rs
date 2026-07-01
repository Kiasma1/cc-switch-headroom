//! Headroom 压缩代理进程的生命周期管理。
//!
//! 移植自 Go 版 tray-tool 的 Proxy/ports 逻辑。本模块自包含，
//! 不依赖数据库或 AppState，便于独立测试。

use std::io::Write;
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

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
}
