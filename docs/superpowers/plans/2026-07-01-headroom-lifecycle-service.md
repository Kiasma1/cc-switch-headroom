# Headroom 生命周期服务 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 cc-switch 后端新增一个自包含的 `HeadroomManager`，能启动、健康检查、按进程树停止本地 Headroom 压缩代理进程，并暴露为 Tauri 命令。

**架构：** 移植现有 Go 版 tray-tool 的进程管理逻辑（`Proxy` 的 Start/Stop/Observe + `ports.go` 的端口归属判断）到 Rust。`HeadroomManager` 是独立 struct（不依赖数据库或 `AppState`），内部状态用 `Mutex` 保护，镜像 tray-tool 的 `Proxy` 设计。链路方向与开关接线属于后续子系统，本计划不涉及——本计划只交付"能可靠地起停一个 Headroom 进程"这一块可独立测试的能力。详见 `docs/adr/0001-headroom-orchestration-integration.md`。

**技术栈：** Rust、Tauri 2、`std::process::Command`（Windows `creation_flags` 抑制窗口，与 `commands/misc.rs` 一致）、`thiserror`（`AppError`）、`reqwest`/`ureq` 或标准库做 `/livez` 健康探测。

**关键常量（本计划固定值）：**
- Headroom 可执行文件：`~/.headroom-venv/Scripts/headroom.exe`
- Headroom 监听端口：`8787`
- 上游（写死指向 cc-switch 代理）：`http://127.0.0.1:15721`

---

## 文件结构

| 文件 | 职责 |
|------|------|
| 创建 `src-tauri/src/services/headroom.rs` | `HeadroomManager` 及其配置、纯逻辑函数、生命周期方法、内联单元测试 |
| 修改 `src-tauri/src/services/mod.rs` | 声明 `pub mod headroom;` 并 re-export `HeadroomManager` |
| 创建 `src-tauri/src/commands/headroom.rs` | `headroom_start` / `headroom_stop` / `headroom_status` 三个 Tauri 命令 |
| 修改 `src-tauri/src/commands/mod.rs` | 声明 `pub mod headroom;` 并 re-export 命令 |
| 修改 `src-tauri/src/store.rs` | `AppState` 增加 `headroom_manager` 字段 |
| 修改 `src-tauri/src/lib.rs` | 在 `generate_handler!` 注册三个命令 |
| 创建 `src-tauri/tests/headroom_service.rs` | 集成测试：真实启动 Headroom、健康检查、停止 |

**移植来源（只读参考，不修改）：**
- `C:\Users\wsqzlzc\.headroom\tray-tool\proxy.go` — Start/Stop/Observe/pidMatchesProxy/buildEnv
- `C:\Users\wsqzlzc\.headroom\tray-tool\ports.go` — isPortOpen/pidOnPort/commandLineForPID

---

## 任务 1：HeadroomConfig 与参数构造（纯函数）

**文件：**
- 创建：`src-tauri/src/services/headroom.rs`
- 修改：`src-tauri/src/services/mod.rs`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/src/services/headroom.rs` 写入：

```rust
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
}
```

在 `src-tauri/src/services/mod.rs` 的模块声明区（第 1–30 行的 `pub mod ...` 列表中，按字母序）加入：

```rust
pub mod headroom;
```

并在 re-export 区（`pub use ...` 段）加入：

```rust
pub use headroom::{HeadroomConfig, HeadroomManager};
```

> 注意：`HeadroomManager` 在任务 5 才定义。若本任务先编译，临时只 re-export `HeadroomConfig`，任务 5 完成后补上 `HeadroomManager`。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::build_args_contains_port_host_and_upstream`
预期：编译失败或断言前的路径未建立——先确认测试被发现。

- [ ] **步骤 3：实现已在步骤 1 内联**

步骤 1 已写入 `build_args` 实现，无需额外代码。

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::build_args_contains_port_host_and_upstream`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs src-tauri/src/services/mod.rs
git commit -m "feat(headroom): add HeadroomConfig and build_args"
```

---

## 任务 2：进程归属判断 pid_matches_headroom（纯函数，安全关键）

> 这是整个服务里最关键的安全检查：Stop() 只允许杀"确实是我们的 Headroom"的进程。判断错误会误杀端口上的陌生进程。移植自 tray-tool proxy.go 的 `pidMatchesProxy`。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

在 `headroom.rs` 的 `impl HeadroomConfig` 中新增方法：

```rust
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
```

在 `#[cfg(test)] mod tests` 中追加：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::cmdline`
预期：编译通过前若方法未加则失败；加方法后 4 个测试应全绿。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::cmdline`
预期：4 个测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs
git commit -m "feat(headroom): add safety-critical cmdline_matches process ownership check"
```

---

## 任务 3：端口与 PID 探测（Windows）

> 移植 tray-tool ports.go。`is_port_open` 用 TCP 连接探测；`pid_on_port` 解析 `netstat`；`command_line_for_pid` 用 PowerShell 查命令行。全部用 `CREATE_NO_WINDOW` 抑制窗口（与 `commands/misc.rs:19,239` 一致）。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

在 `headroom.rs` 顶部补充 import：

```rust
use std::io::Write;
use std::net::TcpStream;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x08000000;
```

在模块内（`impl` 之外）新增自由函数：

```rust
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
```

在 `#[cfg(test)] mod tests` 追加（用真实临时监听端口验证 `is_port_open`）：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::is_port_open_true_when_listening_false_when_not`
预期：先因函数未定义编译失败，加入后转为 PASS。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::is_port_open_true_when_listening_false_when_not`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs
git commit -m "feat(headroom): port is_port_open/pid_on_port/command_line_for_pid from tray-tool"
```

---

## 任务 4：健康探测 health_check（/livez）

> 验证阶段确认 Headroom 就绪后 `GET /livez` 返回 200。用它做"是否活着"的判定，比端口+命令行匹配更直接。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

先确认 `src-tauri/Cargo.toml` 已有 HTTP 客户端依赖（cc-switch 使用 `reqwest`；若 `[dependencies]` 中已存在 `reqwest`，复用之）。运行：

```bash
cd src-tauri && grep -n "reqwest" Cargo.toml
```

在 `headroom.rs` 新增函数：

```rust
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
```

在 `#[cfg(test)] mod tests` 追加（用标准库起一个返回 200 的最小服务器）：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::health_check_true_on_200_false_on_no_server`
预期：函数未定义时失败；加入后 PASS。若 `reqwest` 无 `blocking` feature，在 `Cargo.toml` 的 `reqwest` 依赖启用 `features = ["blocking"]`（dev 环境足够）。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::health_check_true_on_200_false_on_no_server`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs src-tauri/Cargo.toml
git commit -m "feat(headroom): add /livez health_check"
```

---

## 任务 5：HeadroomManager 与 start()

> `HeadroomManager` 持有配置与内部状态（子进程句柄、owned_pid、last_error），用 `Mutex` 保护。`start()` 移植 tray-tool proxy.go 的 Start：端口已被占用且命令行匹配 → 接管；被陌生进程占用 → 报冲突不强杀；否则 spawn。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

在 `headroom.rs` 顶部补充：

```rust
use std::fs::OpenOptions;
use std::process::Child;
use std::sync::Mutex;

use crate::error::AppError;
```

新增状态枚举与结构：

```rust
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

    fn set_error(&self, msg: &str) {
        if let Ok(mut st) = self.state.lock() {
            st.last_error = Some(msg.to_string());
        }
    }
}
```

在 `#[cfg(test)] mod tests` 追加（不依赖真实 headroom：用一个不存在的 exe 路径验证预检失败分支）：

```rust
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
```

同时在 `services/mod.rs` 的 re-export 补上 `HeadroomManager`（若任务 1 暂缓）。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::start_fails_when_exe_missing`
预期：结构未定义时编译失败；补齐后 PASS。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::start_fails_when_exe_missing`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs src-tauri/src/services/mod.rs
git commit -m "feat(headroom): add HeadroomManager and start() with takeover/conflict handling"
```

---

## 任务 6：stop()（按进程树终止）

> 移植 tray-tool proxy.go 的 Stop：`taskkill /PID <pid> /T /F` 终止整个进程树（Headroom 会派生子/孙进程真正监听端口），再按匹配端口二次清理，避免孤儿监听进程占用端口。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

在 `impl HeadroomManager` 新增：

```rust
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
```

在模块内新增自由函数：

```rust
/// taskkill /T /F 终止进程树。对应 tray-tool 的 kill := exec.Command("taskkill"...)。
fn kill_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}
```

在 `#[cfg(test)] mod tests` 追加（无进程时 stop 应幂等成功）：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::stop_is_idempotent_when_nothing_running`
预期：方法未定义时失败；加入后 PASS。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::stop_is_idempotent_when_nothing_running`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs
git commit -m "feat(headroom): add stop() with process-tree kill and port re-cleanup"
```

---

## 任务 7：status()

> 综合健康探测与进程归属，给出 `HeadroomStatus`。供前端轮询显示。

**文件：**
- 修改：`src-tauri/src/services/headroom.rs`

- [ ] **步骤 1：编写失败的测试**

在 `impl HeadroomManager` 新增：

```rust
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
```

在 `#[cfg(test)] mod tests` 追加：

```rust
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
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::status_stopped_when_nothing_on_port`
预期：方法未定义时失败；加入后 PASS。

- [ ] **步骤 3：实现已在步骤 1 内联**

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib services::headroom::tests::status_stopped_when_nothing_on_port`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/services/headroom.rs
git commit -m "feat(headroom): add status() and last_error()"
```

---

## 任务 8：Tauri 命令与 AppState 接线

> 把 `HeadroomManager` 挂到 `AppState`，暴露三个命令。命令签名遵循 `commands/proxy.rs` 的 `Result<T, String>` 模式（`AppError` 已实现 `From<AppError> for String`）。

**文件：**
- 创建：`src-tauri/src/commands/headroom.rs`
- 修改：`src-tauri/src/commands/mod.rs`
- 修改：`src-tauri/src/store.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：编写失败的测试**

在 `src-tauri/tests/headroom_service.rs` 写一个"构造 manager 并读取 Stopped 状态"的黑盒测试（命令层依赖 Tauri State，改为直接测 manager，命令仅薄封装）：

```rust
use cc_switch_lib::services::{HeadroomConfig, HeadroomManager};
use std::path::PathBuf;

#[test]
fn manager_reports_stopped_for_free_port() {
    let cfg = HeadroomConfig {
        exe_path: PathBuf::from(r"C:\does\not\exist\headroom.exe"),
        port: 59790,
        upstream_url: "http://127.0.0.1:15721".to_string(),
    };
    let mgr = HeadroomManager::new(cfg, std::env::temp_dir().join("hr-it.log"));
    // 独立进程外可见：状态为 stopped 序列化为 "stopped"
    let json = serde_json::to_string(&mgr.status()).unwrap();
    assert_eq!(json, "\"stopped\"");
}
```

> 前置：`cc_switch_lib` 需 re-export `services`。确认 `src-tauri/src/lib.rs` 顶部有 `pub mod services;`（cc-switch 已有）。若 `HeadroomConfig`/`HeadroomManager` 未通过 `cc_switch_lib::services::` 可见，检查任务 1/5 的 re-export。

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --test headroom_service manager_reports_stopped_for_free_port`
预期：因可见性或类型缺失失败。

- [ ] **步骤 3：编写命令、AppState 接线、注册**

`src-tauri/src/commands/headroom.rs`：

```rust
//! Headroom 压缩代理的生命周期命令。

use crate::services::HeadroomStatus;
use crate::store::AppState;

#[tauri::command]
pub async fn headroom_start(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.headroom_manager.start().map_err(Into::into)
}

#[tauri::command]
pub async fn headroom_stop(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.headroom_manager.stop().map_err(Into::into)
}

#[tauri::command]
pub async fn headroom_status(state: tauri::State<'_, AppState>) -> Result<HeadroomStatus, String> {
    Ok(state.headroom_manager.status())
}
```

在 `src-tauri/src/commands/mod.rs` 增加模块声明与 re-export（模仿现有 `pub mod proxy;` 与 `pub use proxy::*;` 行）：

```rust
pub mod headroom;
pub use headroom::{headroom_start, headroom_status, headroom_stop};
```

`src-tauri/src/services/mod.rs` 补充 re-export（若尚未）：

```rust
pub use headroom::{HeadroomConfig, HeadroomManager, HeadroomStatus};
```

`src-tauri/src/store.rs` —— 给 `AppState` 增加字段并在 `new` 构造。将现有内容改为：

```rust
use crate::database::Database;
use crate::services::{HeadroomConfig, HeadroomManager, ProxyService, UsageCache};
use crate::config::get_home_dir;
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
            port: 8787,
            upstream_url: "http://127.0.0.1:15721".to_string(),
        };
        let headroom_log = home.join(".headroom").join("logs").join("claude-proxy.log");
        let headroom_manager = Arc::new(HeadroomManager::new(headroom_cfg, headroom_log));

        Self {
            db,
            proxy_service,
            usage_cache: Arc::new(UsageCache::new()),
            headroom_manager,
        }
    }
}
```

在 `src-tauri/src/lib.rs` 的 `tauri::generate_handler![...]` 列表中（`commands::get_proxy_status` 附近）加入三行：

```rust
            commands::headroom_start,
            commands::headroom_stop,
            commands::headroom_status,
```

- [ ] **步骤 4：运行测试与编译验证通过**

运行：`cd src-tauri && cargo test --test headroom_service manager_reports_stopped_for_free_port`
预期：PASS。
再运行：`cd src-tauri && cargo build`
预期：编译通过，无未使用 import 警告阻断。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/commands/headroom.rs src-tauri/src/commands/mod.rs src-tauri/src/services/mod.rs src-tauri/src/store.rs src-tauri/src/lib.rs src-tauri/tests/headroom_service.rs
git commit -m "feat(headroom): expose start/stop/status Tauri commands and wire into AppState"
```

---

## 任务 9：真实进程集成测试（手动 / 可选 CI 门控）

> 前 8 任务的自动化测试都不依赖真实 Headroom。本任务用真实 exe 端到端验证起-探-停闭环。因依赖本机安装 Headroom，标记 `#[ignore]`，手动运行。

**文件：**
- 修改：`src-tauri/tests/headroom_service.rs`

- [ ] **步骤 1：编写被忽略的集成测试**

```rust
use std::time::Duration;

/// 需要本机 ~/.headroom-venv/Scripts/headroom.exe。手动运行：
/// cargo test --test headroom_service real_headroom_start_health_stop -- --ignored --nocapture
#[test]
#[ignore]
fn real_headroom_start_health_stop() {
    use cc_switch_lib::services::{HeadroomConfig, HeadroomManager, HeadroomStatus};
    use std::path::PathBuf;

    let home = dirs::home_dir().expect("home dir");
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
```

> 若 `dirs` 不在 dev-dependencies，用 `std::env::var("USERPROFILE")` 代替：`PathBuf::from(std::env::var("USERPROFILE").unwrap())`。

- [ ] **步骤 2：手动运行集成测试**

运行：`cd src-tauri && cargo test --test headroom_service real_headroom_start_health_stop -- --ignored --nocapture`
预期：PASS——观察日志出现"启动→就绪→停止"，且结束后端口 18799 释放。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/tests/headroom_service.rs
git commit -m "test(headroom): add ignored real-process start/health/stop integration test"
```

---

## 自检

**1. 规格覆盖度：**
- 起 Headroom → 任务 5 `start()` ✅
- 停 Headroom（进程树）→ 任务 6 `stop()` ✅
- 健康检查 → 任务 4 `health_check` + 任务 7 `status()` ✅
- 进程归属安全判断（不误杀）→ 任务 2 `cmdline_matches` ✅
- 端口冲突处理 → 任务 5 冲突分支 + 任务 7 `PortConflict` ✅
- 暴露给前端 → 任务 8 三命令 ✅
- 隐藏控制台窗口 → 任务 3/5/6 全部 `creation_flags(CREATE_NO_WINDOW)` ✅
- 无 key 启动（已验证前提 2）→ `build_args`/env 不含真实 key ✅
- 真实端到端 → 任务 9 ✅
- 链路接线 / 前端 UI → **明确不在本计划**（后续子系统）✅

**2. 占位符扫描：** 无 TODO / 待定 / "类似任务 N"；每个代码步骤含完整代码。✅

**3. 类型一致性：**
- `HeadroomConfig`（任务1）/ `cmdline_matches`（任务2）/ `HeadroomManager`（任务5）/ `HeadroomStatus`（任务5/7）跨任务命名一致。
- `is_port_open` / `pid_on_port` / `command_line_for_pid` / `health_check` / `kill_process_tree` 命名在任务 3/4/6 与调用处（任务 5/6/7）一致。
- 命令 `headroom_start/stop/status`（任务8）与 lib.rs 注册、mod.rs re-export 一致。
- `AppState.headroom_manager`（任务8 store.rs）与命令内 `state.headroom_manager`（任务8）一致。✅

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-07-01-headroom-lifecycle-service.md`。

**已知实现期需确认的小前提（非阻塞，执行时核对）：**
- `reqwest` 的 `blocking` feature（任务4）——若主依赖未启用，dev/运行时按需开启。
- `cc_switch_lib::services::` 可见性（任务8）——依赖 `lib.rs` 已 `pub mod services;`（现状已满足）。
- `commands/mod.rs` 与 `lib.rs` 的 re-export/注册风格——按文件现有同类行照抄。
