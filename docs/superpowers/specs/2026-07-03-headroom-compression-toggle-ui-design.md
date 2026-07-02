# 子系统3：Headroom 压缩开关 UI 设计规格

日期：2026-07-03
状态：待实现（brainstorming 产出，下一步 writing-plans）
前置：子系统2「链路接线」已合并（后端命令 `set_compression_for_app` / `get_compression_status` 可用）。

## 目标

为已合并的压缩后端做前端 UI：顶栏一个 Headroom 压缩开关，镜像现有代理接管开关。用户点一下即可开/关压缩，看到当前状态，接管未开时开关灰显，切换后 toast 提示重启 Claude Code。

## 范围

**做：** 顶栏压缩开关组件、压缩状态 react-query hook、API 封装、i18n（4 语言）、`enableCompressionToggle` 设置项（前后端）、组件/hook 单元测试。

**不做：** 压缩后端逻辑（子系统2 已完成）、常驻"待重启"角标、多 app 压缩（当前仅 Claude）。

## 设计决策（brainstorming 已定）

- **位置**：顶栏工具区，紧接 `FailoverToggle` 之后（`App.tsx`），受 `settingsData?.enableCompressionToggle` gate。
- **`enableCompressionToggle` 默认值**：`true`（压缩开关默认可见，便于发现；对比 `enableFailoverToggle` 默认 false）。
- **禁用态**：接管关 或 activeApp 非 claude 时，开关灰显不可点 + tooltip "请先开启代理接管"（镜像 `FailoverToggle.tsx:81` 的 `disabled` 逻辑）。
- **重启提示**：切换成功且返回 `needs_restart=true` 时，sonner `toast.warning(..., {duration:5000, closeButton:true})`。不做常驻角标。
- **状态呈现**：mutation pending 时显示 `Loader2` 转圈；压缩运行时图标高亮脉冲（镜像 `ProxyToggle` 的 `Radio` + `text-emerald-500 animate-pulse`）。
- **图标**：lucide `Minimize2`。

## 单元与文件

| 单元 | 文件 | 职责 | 模仿对象 |
|------|------|------|----------|
| API 封装 | 创建 `src/lib/api/compression.ts` | `compressionApi.getCompressionStatus()` / `setCompressionForApp(appType, enabled)` | `lib/api/proxy.ts` |
| react-query hook | 创建 `src/lib/query/compression.ts` | `useCompressionStatus()` + `useSetCompressionForApp()` | `lib/query/proxy.ts` |
| 开关组件 | 创建 `src/components/proxy/CompressionToggle.tsx` | 顶栏压缩开关 | `ProxyToggle.tsx` + `FailoverToggle.tsx` |
| 组件导出 | 修改 `src/components/proxy/index.ts` | 导出 CompressionToggle | 现有导出行 |
| 挂载 | 修改 `src/App.tsx`（约 1249 行，FailoverToggle 之后） | 渲染 `<CompressionToggle activeApp={activeApp} />` + gate | `App.tsx:1248` FailoverToggle |
| 设置字段（前端） | 修改 `AppSettings` 类型 + 设置页开关行 | `enableCompressionToggle` | `enableFailoverToggle` |
| 设置字段（后端） | 修改 `src-tauri` 的 `AppSettings`（`settings.rs`） | `enable_compression_toggle: bool`（默认 true） | `enable_failover_toggle` |
| i18n | 修改 `src/i18n/locales/{en,zh,zh-TW,ja}.json` | `proxy.compression.*` + `settings.advanced.proxy.enableCompressionToggle(+Description)` | `proxy.takeover.*` / `enableFailoverToggle` |
| 测试 | 创建 `src/components/proxy/CompressionToggle.test.tsx` | 组件行为测试 | 现有前端 vitest 测试 |

## 数据流

```
CompressionToggle
  ├─ useCompressionStatus()  → get_compression_status（轮询 2s）→ { claude: bool }
  ├─ useProxyStatus()        → 取 takeoverStatus[activeApp]（判断 disabled）
  └─ onCheckedChange
       → useSetCompressionForApp().mutateAsync({ appType, enabled })
       → invoke("set_compression_for_app", { appType, enabled }) → needs_restart: bool
       → onSuccess: 若 needs_restart 弹 toast.warning；invalidate ["compressionStatus"]
       → onError: toast.error(t("proxy.compression.failed", { detail }))
```

## 组件规格（CompressionToggle.tsx）

- **props**：`{ className?: string; activeApp: AppId }`
- **状态**：
  - `compressionStatus = useCompressionStatus()` → `enabled = compressionStatus?.claude ?? false`
  - `{ takeoverStatus } = useProxyStatus()` → `takeoverEnabled = takeoverStatus?.[activeApp] ?? false`
  - `{ mutateAsync: setCompression, isPending } = useSetCompressionForApp()`
- **disabled**：`isPending || !takeoverEnabled || activeApp !== "claude"`
- **handleToggle(checked)**：
  ```ts
  if (checked && !takeoverEnabled) return;           // 双重保险
  try {
    const needsRestart = await setCompression({ appType: activeApp, enabled: checked });
    // toast 在 hook 的 onSuccess 内处理（读 needsRestart）
  } catch (e) { console.error(e); }                  // 错误 toast 由 hook onError 处理
  ```
- **渲染**：`isPending` → `<Loader2 className="animate-spin"/>`；否则 `<Minimize2 className={enabled ? "text-emerald-500 animate-pulse" : ""}/>`；外层 `<div title={tooltipText}>` + `<Switch checked={enabled} onCheckedChange={handleToggle} disabled={disabled} aria-label={...}/>`。
- **tooltipText**：接管关 → t("proxy.compression.takeoverRequired")；否则 → t("proxy.compression.tooltip.enabled/disabled")。

## i18n key（4 locale 同步，带 defaultValue 兜底）

```
proxy.compression.enabled            "{{app}} 压缩已启用"
proxy.compression.disabled           "{{app}} 压缩已关闭"
proxy.compression.failed             "切换压缩失败: {{detail}}"
proxy.compression.restartRequired    "压缩配置已更新,请重启 Claude Code 生效"
proxy.compression.takeoverRequired   "请先开启代理接管,再启用压缩"
proxy.compression.tooltip.enabled    "点击关闭 Headroom 压缩"
proxy.compression.tooltip.disabled   "点击启用 Headroom 压缩"
settings.advanced.proxy.enableCompressionToggle             "显示压缩开关"
settings.advanced.proxy.enableCompressionToggleDescription  "在顶栏显示 Headroom 压缩开关"
```

## 测试策略

- **CompressionToggle.test.tsx**（vitest + @testing-library/react，mock hooks/invoke）：
  - 接管关时开关 `disabled`。
  - 接管开 + 点击 → 调用 `set_compression_for_app({appType:"claude", enabled:true})`。
  - mutation 返回 `needs_restart=true` → 弹 `toast.warning`（spy toast）。
  - activeApp 非 claude → disabled。
- **hook 测试**（可选，若现有 hook 有测试先例）：`useSetCompressionForApp` 的 onSuccess/onError 分支。
- **回归**：现有前端 `vitest run` 保持绿。

## 未决小前提（writing-plans 阶段核对）

- 后端 `AppSettings`（`settings.rs`）加 `enable_compression_toggle` 字段的序列化命名（camelCase via serde？照 `enable_failover_toggle`）。
- 前端 `AppSettings` 类型与 `useSettings`/settings query 的读取路径。
- `App.tsx` 顶栏 gate 的确切写法（对照 `enableLocalProxy` / `enableFailoverToggle` gate）。
- 现有前端是否已有组件测试基建（`@testing-library/react` 已在 devDependencies，确认 test setup）。
