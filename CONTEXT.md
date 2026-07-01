# CONTEXT — Headroom × cc-switch 集成术语表

个人定制版 cc-switch 的领域术语。本文件只定义概念,不含实现细节。

## 核心概念

- **压缩链路(Compression Chain)** — 一次 Claude Code 请求从客户端到供应商所经过的完整路径。集成后的规范链路为:`Claude Code → Headroom → cc-switch 代理 → 供应商`。

- **Headroom** — 外部的上下文压缩层(独立 Python 进程,依赖 ML 模型)。职责单一:在请求送达供应商前削减 token。它**不**做供应商选择,也**不**持有真实凭据。

- **cc-switch 代理(cc-switch Proxy)** — cc-switch 内置的进程内 Rust 代理。职责:供应商选择、故障转移、熔断、格式转换、真实凭据注入。它是**发往供应商的最后一跳**。

- **代理接管模式(Proxy Takeover Mode)** — cc-switch 的一种工作模式:客户端配置被写成永久指向 cc-switch 代理的本地地址,切换供应商在代理内部发生,客户端无感、无需重启。**压缩链路只在此模式下成立。**

- **直连模式(Direct Mode)** — cc-switch 的另一种工作模式:客户端配置被直接写成供应商真实地址,链路中没有 cc-switch 代理。**此模式下压缩链路不成立。**

- **压缩开关(Compression Switch)** — 用户对"是否启用压缩"的控制。语义遵循**旁路语义**:开 = Headroom 在链路中;关 = Headroom 移出链路。

- **旁路(Bypass)** — 把 Headroom 移出压缩链路、让客户端直连 cc-switch 代理的动作。用于两种场景:用户主动关闭压缩,或 Headroom 故障时的恢复。

- **半自动旁路(Semi-automatic Bypass)** — 故障恢复策略:cc-switch 检测到 Headroom 失活后,自动把客户端配置改回直指 cc-switch 代理,并通知用户;由于客户端只在启动时读取上游地址,该切换需用户重启 Claude Code 才生效。

- **占位符凭据(Placeholder Credential)** — 客户端配置中写入的假令牌(而非真实 API key)。客户端携带它发出请求,由 cc-switch 代理在最后一跳替换为供应商真实凭据。**真实凭据从不经过 Headroom。**

## 关键不变量

- 真实 API 凭据只存在于 cc-switch 代理与供应商之间,压缩层不可见。
- cc-switch 代理始终是发往供应商的最后一跳;Headroom 只在其之前。
- 集成范围当前限定为 Claude Code 一个工具;架构为将来扩展 Codex 预留但不实现。
