# 子系统2：Headroom 链路接线 设计规格

日期：2026-07-02
状态：待实现（brainstorming 产出，下一步 writing-plans）
前置：子系统1「Headroom 生命周期服务」已合并（HeadroomManager + headroom_start/stop/status）。架构见 `docs/adr/0001-headroom-orchestration-integration.md`。

## 目标

把已有的「起停 Headroom 进程」能力接成真正的压缩链路：一个**压缩开关**，开启时让 Claude Code 走 `Claude → Headroom(:8787) → cc-switch 代理(:15721) → 供应商`，关闭时回到 `Claude → cc-switch 代理(:15721) → 供应商`。仅后端 + Tauri 命令；前端 UI 属子系统3，半自动旁路看门狗属子系统2b。

## 范围

**本规格做：** 压缩开关状态持久化、开关门禁（仅接管模式可开）、配置写入分支（:8787 / :15721 单一真源）、开关编排 Headroom 生命周期、ANTHROPIC_BASE_URL 回环防护、重启提示、Tauri 命令。

**明确不做（后续子系统）：**
- 半自动旁路看门狗（后台轮询 Headroom 存活、失活自动写回 :15721 + 通知）→ 子系统2b。
- 前端压缩开关 UI → 子系统3。
- Codex 支持 → 架构预留，不实现。

## 关键约束（源码确认）

- Claude 接管时写 `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`，运行时的值由 `ProxyService::build_proxy_urls()`（`src-tauri/src/services/proxy.rs:1238`）从 DB `listen_address`/`listen_port` 构造（回环回退：0.0.0.0→127.0.0.1），返回 `(claude_proxy_url, codex_proxy_url)`。`claude_proxy_url` 即写入 `ANTHROPIC_BASE_URL` 的值（经 `apply_claude_takeover_fields_with_policy_and_models`，proxy.rs:154）。
- 接管开/关状态存 DB `proxy_config` 表 `enabled` 列（按 app_type）。读写经 `get_proxy_config_for_app` / `update_proxy_config_for_app`（`database/dao/proxy.rs`）。命令层 `get_proxy_takeover_status` / `set_proxy_takeover_for_app`（`commands/proxy.rs:44,52`），核心逻辑 `set_takeover_for_app`（`services/proxy.rs:618`）。
- 热切换供应商是代理内部内存改写（`ProxyServer.current_providers`），base URL 保持不变。因此**切供应商不需重启客户端**；但**压缩开关改的是 base URL 本身（:8787↔:15721），Claude 只在启动读 ANTHROPIC_BASE_URL → 必须重启客户端生效**。
- cc-switch 的 `restart_app`（`commands/settings.rs:175`）重启的是 **cc-switch 自身**，不是 Claude Code（独立 CLI 进程）。客户端重启只能提示用户手动执行。

## 设计

### ① 状态存储

`proxy_config` 表新增 `compression_enabled` 布尔列（按 app_type，镜像现有 `enabled`）。默认 false。DAO 层 `get_proxy_config_for_app` / `update_proxy_config_for_app` 读写该列（`AppProxyConfig` struct 加字段）。当前仅 Claude 行会被置 true；per-app schema 为 Codex 预留。

### ② 开关门禁

`compression_enabled` 只在同 app 的 `enabled`（接管）为 true 时允许置 true。在编排命令入口校验：接管关时拒绝开压缩，返回明确错误（前端据此禁用开关，属子系统3）。接管从开转关时，若压缩仍开，需一并关压缩并写回 :15721（避免遗留指向 :8787 的死配置）。

### ③ 配置写入分支（方案 A：单一真源）

集中「Claude base URL 选择」到一处：

```
claude_base_url(app=claude) =
    if compression_enabled(claude) { "http://127.0.0.1:8787" }
    else { build_proxy_urls().0 }   // 现有 :15721 逻辑
```

所有写/重写 Claude live 配置的运行时路径都经此决策，不新增第二真源：
- 接管应用路径（`build_proxy_urls` → `apply_claude_takeover_fields_*`）。
- 热切换同步路径（`sync_claude_live_from_provider_while_proxy_active` → `apply_claude_takeover_fields_for_provider`，当前硬编码/传入 :15721 处改为读该决策）。

实现建议：在 `build_proxy_urls`（或其 Claude 消费点）读取 `compression_enabled(claude)` 并据此返回 Claude URL；Codex URL 不受影响。具体注入点由 writing-plans 阶段对照调用图确定，但决策函数只有一个。

### ④ 开关编排 Headroom 生命周期

新命令 `set_compression_for_app(app_type, enabled)` 编排：

**开（enabled=true）：**
1. 门禁校验（接管须为开，否则 Err）。
2. `HeadroomManager.start()`。
3. 轮询 `HeadroomManager.status()` 直到 `Running`（就绪超时沿用子系统1，约30s；失败则回滚：不改配置、不置位、Err）。
4. DB 置 `compression_enabled(claude)=true`。
5. 重写 Claude live 配置（经 ③，此时算出 :8787）。
6. 返回需重启标记 → 前端 toast「请重启 Claude Code 生效」。

**关（enabled=false）：**
1. DB 置 `compression_enabled(claude)=false`。
2. 重写 Claude live 配置（经 ③，算出 :15721）。
3. `HeadroomManager.stop()`。
4. 返回需重启标记 → toast。

顺序原则：开时「先起进程、就绪后才改配置」（不让客户端指向死 :8787）；关时「先改配置回 :15721、再停进程」（不留指向已停 :8787 的窗口）。

### ⑤ 回环防护

子系统1 的 `HeadroomManager.start()` spawn Headroom 时，显式设 `ANTHROPIC_BASE_URL=http://127.0.0.1:15721` 到子进程环境（与现有 `--anthropic-api-url` flag 双保险），防止 Headroom 继承到指向 :8787 的 ANTHROPIC_BASE_URL 形成自环。一处小改（`services/headroom.rs` 的 `start()` 的 `.env(...)` 链）。见 ADR「后续注意事项」。

### ⑥ Tauri 命令

- `set_compression_for_app(state, app_type, enabled) -> Result<CompressionResult, String>`：编排 ④，`CompressionResult { needs_restart: bool }`。镜像 `set_proxy_takeover_for_app`。
- `get_compression_status(state) -> Result<..., String>`：读各 app `compression_enabled`。镜像 `get_proxy_takeover_status`。
- 注册进 `lib.rs` 的 `generate_handler!`。
- 子系统2 用控制台 `window.__TAURI__.core.invoke(...)` 验证（同子系统1），前端开关 = 子系统3。

## 测试策略

- 单元：`compression_enabled` DAO 读写往返；门禁（接管关时 set 压缩→Err）；base URL 决策纯函数（压缩开→:8787、关→build_proxy_urls 值）；接管转关时联动关压缩。
- 编排：开/关序列的状态迁移与回滚（start 失败不改配置）——用可注入的 HeadroomManager 或 `#[ignore]` 真实进程集成测试。
- 回环防护：断言 spawn env 含 `ANTHROPIC_BASE_URL=http://127.0.0.1:15721`。
- 回归：现有 proxy takeover / 热切换测试保持绿（③ 的改动不破坏非压缩路径——压缩关时 base URL 必须与改动前逐字节一致）。

## 单元边界

| 单元 | 职责 | 依赖 |
|------|------|------|
| DAO `compression_enabled` 列 | 持久化压缩状态 | SQLite proxy_config |
| Claude base URL 决策 | 压缩状态 → :8787/:15721 单一真源 | build_proxy_urls + compression 读取 |
| `set_compression_for_app` 编排 | 门禁+进程+配置+重启标记 | HeadroomManager、决策、DAO |
| 回环防护 env | Headroom 子进程环境 | headroom.rs start() |
| Tauri 命令层 | 暴露给前端/控制台 | 编排 |

## 未决小前提（writing-plans 阶段核对）

- ③ 的精确注入点：`build_proxy_urls` 内分支，还是其 Claude 消费点分支——对照 `apply_claude_takeover_fields_*` 全部运行时调用方后定。约束：决策函数唯一、Codex 不受影响、压缩关时 base URL 与现状逐字节一致。
- `AppProxyConfig` struct 与 `proxy_config` 建表 SQL 加列的迁移方式（是否需 schema migration）。
