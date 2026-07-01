# 1. Headroom 以编排层方式集成进 cc-switch

日期:2026-07-01
状态:已接受

## 背景

目标:做一个个人定制版 cc-switch,用一个 GUI 同时管理**供应商切换**和**上下文压缩**。压缩能力由现有的 Headroom(独立 Python 进程 + ML 模型)提供;供应商切换与故障转移由 cc-switch 内置的 Rust 代理提供。

现状:用户当前用一个 Go 编写的托盘工具(tray-tool)启动 Headroom 并设置 `ANTHROPIC_BASE_URL`。cc-switch 与之独立。

约束(均由源码确认):
- cc-switch 代理默认监听 `127.0.0.1:15721`,持有跨请求的熔断器状态,是发往供应商的最后一跳。
- cc-switch 对 Claude Code 有"代理接管模式"(配置永久指向 :15721,占位符凭据,内部热切换)与"直连模式"(配置直指供应商真实 URL)两种。
- Claude Code 只在启动时读取 `ANTHROPIC_BASE_URL`,变更该值需重启客户端。
- 接管模式下写入客户端的是占位符令牌 `PROXY_MANAGED`,真实凭据由 cc-switch 在最后一跳注入。

## 决策

**以编排层方式集成(方案 A),而非把压缩嵌入 cc-switch 的 Rust 代理(方案 B)。**

规范压缩链路:

```
Claude Code → Headroom(:8787 压缩) → cc-switch 代理(:15721 选供应商+故障转移) → 供应商
```

关键设计点:
1. **链路方向 P**:Claude Code 直接指向 Headroom;Headroom 上游写死指向 cc-switch 代理。切供应商全在 cc-switch 内部,Headroom 无感。
2. **仅在代理接管模式下成立**。集成的接入点是 cc-switch 写入客户端配置时:压缩开启则写 Headroom 地址(:8787),而非 cc-switch 代理地址(:15721)。
3. **生命周期归 cc-switch**:由 cc-switch 拉起、守护、停止 Headroom 进程;tray-tool 退役。
4. **压缩开关采用旁路语义(Y)**:关 = 停 Headroom + 客户端配置改回 :15721 + 重启 Claude;开 = 客户端指 :8787,Headroom 上游 :15721。
5. **故障恢复为半自动旁路**:Headroom 失活时 cc-switch 改回配置并通知用户,用户重启 Claude 生效。
6. **凭据透传**:占位符令牌穿过 Headroom,cc-switch 注入真实 key;真实凭据不经过压缩层。
7. **范围**:仅 Claude Code;为 Codex 扩展预留但不实现。

## 被否决的替代方案

- **方案 B(把压缩嵌入 Rust 代理)**:需在 forwarder 中每请求调用 Headroom subprocess,或用 Rust 原生重写整套 ML 压缩管线。前者增加延迟与复杂度,后者工作量不现实。否决。

- **链路方向 Q(反转:Claude → cc-switch,内部绕道 Headroom)**:能让故障旁路与压缩开关都做到零重启,但要求 cc-switch 代理承担"压缩前置"角色,每请求两次穿过 cc-switch 进程,侵入度接近方案 B。鉴于压缩常开、故障罕见,不值得为罕见事件反转架构。否决,选 P。

## 后果

**正面**:
- cc-switch 与 Headroom 各司其职,配置互不理解,耦合最小。
- 真实凭据对压缩层不可见,安全性提升。
- 切供应商保持热切换(该场景下 :8787 地址不变)。

**负面**:
- Headroom 常开时是压缩链路的单点故障。
- 压缩开关与故障旁路均需重启 Claude Code(因 `ANTHROPIC_BASE_URL` 变更)——但压缩常开,触发罕见。

## 已验证前提(2026-07-01,实测)

用捕获服务器充当假上游(:19001)、Headroom 指向它(:18787)、发真实 Anthropic `/v1/messages` 请求实测,证据如下:

1. **Headroom 保持合法 Anthropic 线格式 — 确认。** 转发给上游的请求路径为 `/v1/messages`,body 为结构合法的 Anthropic JSON(`model` / `max_tokens` / `messages`(数组)/ `system` 字段齐全),cc-switch 可正常解析。
2. **Headroom 无需真实 API key 即可启动 — 确认。** 未设任何 key 环境变量时,proxy 正常启动,`/livez` 返回 200,端口 LISTENING,日志无 key 缺失报错。`headroom proxy` 命令本身无 `--api-key` 选项。tray-tool 原有的 key 预检是其自加门槛,新架构应移除。
3. **占位符凭据透传 — 附带确认。** 客户端发的 `x-api-key: PROXY_MANAGED` 原样穿过 Headroom 到达上游,未被剥离或改写,证明 cc-switch 的占位符注入方案在链路中可行。

补充发现:Headroom 有 `--no-optimize` 直通模式;默认会注入 CCR retrieve 工具与 memory 工具(可用 `--no-ccr-inject-tool` / `--no-memory-tools` 关闭)——是否保留由实现阶段调参决定,不影响架构。
