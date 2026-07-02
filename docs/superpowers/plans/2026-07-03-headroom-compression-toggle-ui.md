# Headroom 压缩开关 UI 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 cc-switch 顶栏加一个 Headroom 压缩开关，镜像现有代理接管/故障转移开关——用户一键开/关压缩，看到状态，接管未开时灰显，切换后 toast 提示重启 Claude Code。

**架构：** 纯前端（React + TS + @tanstack/react-query + sonner + react-i18next + radix Switch）+ 一个 `enableCompressionToggle` 设置字段（前后端）。`CompressionToggle` 组件镜像 `FailoverToggle`；API/hook 镜像 `lib/api/proxy.ts` 与 `lib/query/failover.ts`。后端命令 `set_compression_for_app` / `get_compression_status` 已就绪（子系统2）。

**技术栈：** React、TypeScript、@tanstack/react-query、sonner、react-i18next、lucide-react、vitest + @testing-library/react。

**镜像来源（只读参考）：**
- `src/components/proxy/FailoverToggle.tsx`（组件主模板，含"接管关时 disabled"逻辑）
- `src/lib/api/proxy.ts`（API 封装模板）
- `src/lib/query/failover.ts`（hook + mutation onSuccess/onError 模板）
- `src/components/settings/ProxyTabContent.tsx:154-162`（设置 ToggleRow 模板）
- `src/App.tsx:1246-1249`（顶栏挂载 + gate 模板）
- `src-tauri/src/settings.rs:340-505`（后端 AppSettings）

---

## 文件结构

| 文件 | 职责 |
|------|------|
| 修改 `src/i18n/locales/{en,zh,zh-TW,ja}.json` | `proxy.compression.*` + `settings.advanced.proxy.enableCompressionToggle(+Description)` |
| 创建 `src/lib/api/compression.ts` | `compressionApi.getCompressionStatus/setCompressionForApp` |
| 修改 `src/lib/api/index.ts` | 导出 `compressionApi` |
| 创建 `src/lib/query/compression.ts` | `useCompressionStatus` + `useSetCompressionForApp` |
| 创建 `src/components/proxy/CompressionToggle.tsx` | 顶栏压缩开关组件 |
| 修改 `src/components/proxy/index.ts` | 导出 `CompressionToggle` |
| 修改 `src-tauri/src/settings.rs` | `AppSettings` 加 `enable_compression_toggle`（默认 true） |
| 修改 `src/types.ts` | `AppSettings` 加 `enableCompressionToggle?: boolean` |
| 修改 `src/components/settings/ProxyTabContent.tsx` | 加压缩开关设置行 |
| 修改 `src/App.tsx` | 顶栏 FailoverToggle 之后挂 `CompressionToggle` + gate |
| 创建 `src/components/proxy/CompressionToggle.test.tsx` | 组件行为测试（仓库首个组件测试） |

---

## 任务 1：i18n 文案（4 语言）

**文件：**
- 修改：`src/i18n/locales/en.json`
- 修改：`src/i18n/locales/zh.json`
- 修改：`src/i18n/locales/zh-TW.json`
- 修改：`src/i18n/locales/ja.json`

- [ ] **步骤 1：在 4 个 locale 的 `proxy` 对象下加 `compression` 子对象**

在每个文件的 `"proxy": { ... }` 内（`takeover` 同级）加入。**zh.json**：

```json
    "compression": {
      "enabled": "{{app}} 压缩已启用",
      "disabled": "{{app}} 压缩已关闭",
      "failed": "切换压缩失败: {{detail}}",
      "restartRequired": "压缩配置已更新，请重启 Claude Code 生效",
      "takeoverRequired": "请先开启代理接管，再启用压缩",
      "tooltip": {
        "enabled": "点击关闭 Headroom 压缩",
        "disabled": "点击启用 Headroom 压缩"
      }
    },
```

**en.json**：

```json
    "compression": {
      "enabled": "{{app}} compression enabled",
      "disabled": "{{app}} compression disabled",
      "failed": "Failed to toggle compression: {{detail}}",
      "restartRequired": "Compression config updated. Restart Claude Code to take effect.",
      "takeoverRequired": "Enable proxy takeover first, then enable compression",
      "tooltip": {
        "enabled": "Click to disable Headroom compression",
        "disabled": "Click to enable Headroom compression"
      }
    },
```

**zh-TW.json**：

```json
    "compression": {
      "enabled": "{{app}} 壓縮已啟用",
      "disabled": "{{app}} 壓縮已關閉",
      "failed": "切換壓縮失敗: {{detail}}",
      "restartRequired": "壓縮設定已更新，請重新啟動 Claude Code 生效",
      "takeoverRequired": "請先開啟代理接管，再啟用壓縮",
      "tooltip": {
        "enabled": "點擊關閉 Headroom 壓縮",
        "disabled": "點擊啟用 Headroom 壓縮"
      }
    },
```

**ja.json**：

```json
    "compression": {
      "enabled": "{{app}} 圧縮が有効になりました",
      "disabled": "{{app}} 圧縮が無効になりました",
      "failed": "圧縮の切り替えに失敗しました: {{detail}}",
      "restartRequired": "圧縮設定を更新しました。Claude Code を再起動してください。",
      "takeoverRequired": "先にプロキシ引き継ぎを有効にしてください",
      "tooltip": {
        "enabled": "クリックして Headroom 圧縮を無効化",
        "disabled": "クリックして Headroom 圧縮を有効化"
      }
    },
```

- [ ] **步骤 2：在 4 个 locale 的 `settings.advanced.proxy` 对象下加两个 key**

紧挨现有 `enableFailoverToggle` / `enableFailoverToggleDescription`（各文件约 342-343 行）加入。**zh.json**：

```json
        "enableCompressionToggle": "显示压缩开关",
        "enableCompressionToggleDescription": "在顶栏显示 Headroom 压缩开关",
```

**en.json**：

```json
        "enableCompressionToggle": "Show Compression Toggle on Main Page",
        "enableCompressionToggleDescription": "When enabled, the Headroom compression toggle appears at the top of the main page",
```

**zh-TW.json**：

```json
        "enableCompressionToggle": "顯示壓縮開關",
        "enableCompressionToggleDescription": "在頂欄顯示 Headroom 壓縮開關",
```

**ja.json**：

```json
        "enableCompressionToggle": "圧縮トグルをメイン画面に表示",
        "enableCompressionToggleDescription": "有効にすると、Headroom 圧縮トグルがメイン画面上部に表示されます",
```

- [ ] **步骤 3：验证 JSON 合法 + 构建通过**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无 JSON 语法错误（tsc 会因 import json 报错则说明 JSON 坏了；主要靠下一步）。
运行：`node -e "['en','zh','zh-TW','ja'].forEach(l=>JSON.parse(require('fs').readFileSync('src/i18n/locales/'+l+'.json','utf8')))"`
预期：无输出（4 个 JSON 均合法）。

- [ ] **步骤 4：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/i18n/locales/en.json src/i18n/locales/zh.json src/i18n/locales/zh-TW.json src/i18n/locales/ja.json && git commit -m "i18n(compression): add compression toggle strings (4 locales)"
```

---

## 任务 2：API 封装层

**文件：**
- 创建：`src/lib/api/compression.ts`
- 修改：`src/lib/api/index.ts`

- [ ] **步骤 1：创建 compression.ts**

`src/lib/api/compression.ts`：

```ts
import { invoke } from "@tauri-apps/api/core";

/** 各应用 Headroom 压缩状态（当前仅 Claude） */
export interface CompressionStatus {
  claude: boolean;
}

export const compressionApi = {
  /** 获取各应用压缩状态 */
  async getCompressionStatus(): Promise<CompressionStatus> {
    return invoke("get_compression_status");
  },

  /** 为指定应用开启/关闭压缩；返回 needs_restart（是否需要重启客户端生效） */
  async setCompressionForApp(
    appType: string,
    enabled: boolean,
  ): Promise<boolean> {
    return invoke("set_compression_for_app", { appType, enabled });
  },
};
```

- [ ] **步骤 2：在 index.ts 导出**

`src/lib/api/index.ts` 在 `export { proxyApi } from "./proxy";` 附近加：

```ts
export { compressionApi } from "./compression";
export type { CompressionStatus } from "./compression";
```

- [ ] **步骤 3：验证编译**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无类型错误。

- [ ] **步骤 4：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/lib/api/compression.ts src/lib/api/index.ts && git commit -m "feat(compression): add compressionApi wrapper"
```

---

## 任务 3：react-query hooks

**文件：**
- 创建：`src/lib/query/compression.ts`

- [ ] **步骤 1：创建 compression.ts**

`src/lib/query/compression.ts`：

```ts
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { compressionApi } from "@/lib/api/compression";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { extractErrorMessage } from "@/utils/errorUtils";

/** 获取各应用压缩状态（轮询 2s，防闪烁保留上次数据） */
export function useCompressionStatus() {
  return useQuery({
    queryKey: ["compressionStatus"],
    queryFn: () => compressionApi.getCompressionStatus(),
    refetchInterval: 2000,
    placeholderData: (previousData) => previousData,
  });
}

/** 为指定应用开启/关闭压缩；onSuccess 读 needs_restart 决定是否提示重启 */
export function useSetCompressionForApp() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: ({ appType, enabled }: { appType: string; enabled: boolean }) =>
      compressionApi.setCompressionForApp(appType, enabled),

    onSuccess: (needsRestart: boolean, variables) => {
      const appLabel = variables.appType === "claude" ? "Claude" : variables.appType;
      toast.success(
        variables.enabled
          ? t("proxy.compression.enabled", {
              app: appLabel,
              defaultValue: `${appLabel} 压缩已启用`,
            })
          : t("proxy.compression.disabled", {
              app: appLabel,
              defaultValue: `${appLabel} 压缩已关闭`,
            }),
        { closeButton: true },
      );
      if (needsRestart) {
        toast.warning(
          t("proxy.compression.restartRequired", {
            defaultValue: "压缩配置已更新，请重启 Claude Code 生效",
          }),
          { duration: 5000, closeButton: true },
        );
      }
      queryClient.invalidateQueries({ queryKey: ["compressionStatus"] });
    },

    onError: (error: Error) => {
      const detail =
        extractErrorMessage(error) ||
        t("common.unknown", { defaultValue: "未知错误" });
      toast.error(
        t("proxy.compression.failed", {
          detail,
          defaultValue: `切换压缩失败: ${detail}`,
        }),
      );
    },
  });
}
```

- [ ] **步骤 2：验证编译**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无类型错误。确认 `@/utils/errorUtils` 的 `extractErrorMessage` 存在（`lib/query/failover.ts:5` 已用同款 import）。

- [ ] **步骤 3：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/lib/query/compression.ts && git commit -m "feat(compression): add useCompressionStatus/useSetCompressionForApp hooks"
```

---

## 任务 4：CompressionToggle 组件

**文件：**
- 创建：`src/components/proxy/CompressionToggle.tsx`
- 修改：`src/components/proxy/index.ts`

- [ ] **步骤 1：创建组件**

`src/components/proxy/CompressionToggle.tsx`（镜像 `FailoverToggle.tsx`，图标换 `Minimize2`，加 `activeApp !== "claude"` 禁用）：

```tsx
/**
 * Headroom 压缩切换开关组件
 *
 * 放置在主界面头部，用于一键启用/关闭 Headroom 上下文压缩。
 * 压缩依赖代理接管：接管未开启时开关灰显不可点。当前仅 Claude 支持。
 */

import { Minimize2, Loader2 } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import {
  useCompressionStatus,
  useSetCompressionForApp,
} from "@/lib/query/compression";
import { useProxyStatus } from "@/hooks/useProxyStatus";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";
import type { AppId } from "@/lib/api";

interface CompressionToggleProps {
  className?: string;
  activeApp: AppId;
}

export function CompressionToggle({
  className,
  activeApp,
}: CompressionToggleProps) {
  const { t } = useTranslation();
  const { data: status, isLoading } = useCompressionStatus();
  const isEnabled = status?.claude ?? false;
  const setCompression = useSetCompressionForApp();
  const { takeoverStatus } = useProxyStatus();
  const takeoverEnabled = takeoverStatus?.[activeApp] ?? false;

  // 当前仅 Claude 支持压缩
  const supported = activeApp === "claude";
  const disabled =
    setCompression.isPending || isLoading || !takeoverEnabled || !supported;

  const handleToggle = (checked: boolean) => {
    if (checked && (!takeoverEnabled || !supported)) return;
    setCompression.mutate({ appType: activeApp, enabled: checked });
  };

  const appLabel = activeApp === "claude" ? "Claude" : activeApp;

  const tooltipText = !takeoverEnabled
    ? t("proxy.compression.takeoverRequired", {
        app: appLabel,
        defaultValue: `请先开启代理接管，再启用压缩`,
      })
    : isEnabled
      ? t("proxy.compression.tooltip.enabled", {
          defaultValue: "点击关闭 Headroom 压缩",
        })
      : t("proxy.compression.tooltip.disabled", {
          defaultValue: "点击启用 Headroom 压缩",
        });

  return (
    <div
      className={cn(
        "flex items-center gap-1 px-1.5 h-8 rounded-lg bg-muted/50 transition-all",
        className,
      )}
      title={tooltipText}
    >
      {setCompression.isPending || isLoading ? (
        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
      ) : (
        <Minimize2
          className={cn(
            "h-4 w-4 transition-colors",
            isEnabled
              ? "text-emerald-500 animate-pulse"
              : "text-muted-foreground",
          )}
        />
      )}
      <Switch
        checked={isEnabled}
        onCheckedChange={handleToggle}
        disabled={disabled}
        aria-label={t("proxy.compression.tooltip.disabled", {
          defaultValue: "点击启用 Headroom 压缩",
        })}
      />
    </div>
  );
}
```

- [ ] **步骤 2：导出**

`src/components/proxy/index.ts` 加（照现有导出行风格）：

```ts
export { CompressionToggle } from "./CompressionToggle";
```

- [ ] **步骤 3：验证编译**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无类型错误。

- [ ] **步骤 4：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/components/proxy/CompressionToggle.tsx src/components/proxy/index.ts && git commit -m "feat(compression): add CompressionToggle header component"
```

---

## 任务 5：enableCompressionToggle 设置字段（前后端）

**文件：**
- 修改：`src-tauri/src/settings.rs`
- 修改：`src/types.ts`
- 修改：`src/components/settings/ProxyTabContent.tsx`

- [ ] **步骤 1：后端 AppSettings 加字段（默认 true）**

`src-tauri/src/settings.rs` 的 `pub struct AppSettings`，在 `pub enable_failover_toggle: bool,`（约 373-374 行）之后加：

```rust
    /// Whether to show the Headroom compression toggle on the main page (default on)
    #[serde(default = "default_true")]
    pub enable_compression_toggle: bool,
```

> 说明：`default_true`（`settings.rs:21`）已存在。用 `#[serde(default = "default_true")]` 让缺该字段的旧 settings.json 也默认 true。

在 `impl Default for AppSettings`（约 505 行 `enable_failover_toggle: false,` 之后）加：

```rust
            enable_compression_toggle: true,
```

- [ ] **步骤 2：前端 AppSettings 类型加字段**

`src/types.ts` 在 `enableFailoverToggle?: boolean;`（约 365 行）之后加：

```ts
  enableCompressionToggle?: boolean;
```

- [ ] **步骤 3：设置页加压缩开关行**

`src/components/settings/ProxyTabContent.tsx`：

先在文件顶部 import 里补 `Minimize2`（若 lucide import 行没有）：找到从 `"lucide-react"` 的 import，加入 `Minimize2`。

在 `enableFailoverToggle` 的 `<ToggleRow ... />`（约 154-162 行）之后加：

```tsx
              <ToggleRow
                icon={<Minimize2 className="h-4 w-4 text-emerald-500" />}
                title={t("settings.advanced.proxy.enableCompressionToggle")}
                description={t(
                  "settings.advanced.proxy.enableCompressionToggleDescription",
                )}
                checked={settings?.enableCompressionToggle ?? true}
                onCheckedChange={(checked) =>
                  void onAutoSave({ enableCompressionToggle: checked })
                }
              />
```

> 注意：`onAutoSave` 是本组件已有的 prop（`enableFailoverToggle` 行的 `handleFailoverToggleChange` 内部调 `onAutoSave({ enableFailoverToggle: checked })`）。压缩开关无首次确认弹窗，直接内联 `onAutoSave`。`checked` 兜底用 `?? true`（与默认 true 一致）。

- [ ] **步骤 4：验证编译**

运行：`cd /c/Users/wsqzlzc/cc-switch/src-tauri && cargo build --lib 2>&1 | grep -iE "error|Finished" | head`
预期：Finished（无 error）。
运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无类型错误。

- [ ] **步骤 5：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src-tauri/src/settings.rs src/types.ts src/components/settings/ProxyTabContent.tsx && git commit -m "feat(compression): add enableCompressionToggle setting (default on)"
```

---

## 任务 6：顶栏挂载

**文件：**
- 修改：`src/App.tsx`

- [ ] **步骤 1：import CompressionToggle**

`src/App.tsx` 找到 import `FailoverToggle` 的地方（从 `@/components/proxy` 或具体路径），加入 `CompressionToggle`。若 FailoverToggle 从 `"@/components/proxy"` barrel 导入，则加进同一 import；否则新增一行 `import { CompressionToggle } from "@/components/proxy/CompressionToggle";`。

- [ ] **步骤 2：在 FailoverToggle 之后挂载**

`src/App.tsx` 的 `FailoverToggle` 渲染块（约 1246-1249 行）之后加：

```tsx
                  {activeApp !== "claude-desktop" &&
                    settingsData?.enableCompressionToggle && (
                      <CompressionToggle activeApp={activeApp} />
                    )}
```

> 说明：gate 用 `settingsData?.enableCompressionToggle`（默认 true，故默认可见）。`activeApp !== "claude-desktop"` 与 FailoverToggle 一致（claude-desktop 用独立路由开关，无代理接管）。

- [ ] **步骤 3：验证编译**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run typecheck`
预期：无类型错误。

- [ ] **步骤 4：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/App.tsx && git commit -m "feat(compression): mount CompressionToggle in header toolbar"
```

---

## 任务 7：组件测试（vitest + @testing-library）

> 这是仓库**首个组件测试**。现有测试均为纯逻辑（`src/lib/version.test.ts`）。vitest 已配 jsdom + setupFiles（`vitest.config.ts:13-14`），`@testing-library/react` 在 devDependencies。本任务先建立最小渲染冒烟测试，再加行为断言。

**文件：**
- 创建：`src/components/proxy/CompressionToggle.test.tsx`

- [ ] **步骤 1：编写测试**

`src/components/proxy/CompressionToggle.test.tsx`：

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CompressionToggle } from "./CompressionToggle";

// mock i18n：t 直接返回 defaultValue 或 key
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) =>
      opts?.defaultValue ?? key,
  }),
}));

// mock sonner toast
const toastWarning = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    warning: (...args: unknown[]) => toastWarning(...args),
    error: vi.fn(),
  },
}));

// mock 压缩状态 + mutation
const setCompressionMutate = vi.fn();
let compressionEnabled = false;
vi.mock("@/lib/query/compression", () => ({
  useCompressionStatus: () => ({
    data: { claude: compressionEnabled },
    isLoading: false,
  }),
  useSetCompressionForApp: () => ({
    mutate: setCompressionMutate,
    isPending: false,
  }),
}));

// mock 接管状态（默认接管开）
let takeoverOn = true;
vi.mock("@/hooks/useProxyStatus", () => ({
  useProxyStatus: () => ({
    takeoverStatus: { claude: takeoverOn },
  }),
}));

function renderToggle(activeApp: "claude" | "codex" = "claude") {
  const qc = new QueryClient();
  return render(
    <QueryClientProvider client={qc}>
      <CompressionToggle activeApp={activeApp} />
    </QueryClientProvider>,
  );
}

describe("CompressionToggle", () => {
  beforeEach(() => {
    setCompressionMutate.mockClear();
    toastWarning.mockClear();
    compressionEnabled = false;
    takeoverOn = true;
  });

  it("接管开启时，点击开关调用 set_compression_for_app", () => {
    renderToggle("claude");
    const sw = screen.getByRole("switch");
    fireEvent.click(sw);
    expect(setCompressionMutate).toHaveBeenCalledWith({
      appType: "claude",
      enabled: true,
    });
  });

  it("接管关闭时开关 disabled", () => {
    takeoverOn = false;
    renderToggle("claude");
    expect(screen.getByRole("switch")).toBeDisabled();
  });

  it("activeApp 非 claude 时 disabled", () => {
    renderToggle("codex");
    expect(screen.getByRole("switch")).toBeDisabled();
  });
});
```

> 说明：本测试聚焦组件的新增行为（disabled 门禁 + 点击调命令）。`needs_restart → toast.warning` 的逻辑在 hook 的 `onSuccess` 内（任务3），因组件已 mock 掉 hook，故不在组件测试覆盖，属 hook 层职责；组件测试不强行穿透 mock。radix `Switch` 渲染为 `role="switch"`，`disabled` 反映到 DOM。

- [ ] **步骤 2：运行测试验证**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run test:unit -- CompressionToggle`
预期：3 个测试通过。若 `toBeDisabled` 不可用，确认 `tests/setupTests.ts` 引入了 `@testing-library/jest-dom`（若未引入，测试改用 `expect(sw.getAttribute("disabled")).not.toBeNull()` 或检查 `aria-disabled`/`data-disabled`）。

- [ ] **步骤 3：全量前端测试回归**

运行：`cd /c/Users/wsqzlzc/cc-switch && pnpm run test:unit`
预期：全部通过（原有 + 新增）。

- [ ] **步骤 4：Commit**

```bash
cd /c/Users/wsqzlzc/cc-switch && git add src/components/proxy/CompressionToggle.test.tsx && git commit -m "test(compression): add CompressionToggle component tests"
```

---

## 自检

**1. 规格覆盖度：**
- ① API 层 → 任务 2 ✅
- ② react-query hook → 任务 3 ✅
- ③ 组件 → 任务 4 ✅
- ④ 挂载 → 任务 6 ✅
- ⑤ 设置字段（前后端）→ 任务 5 ✅
- ⑥ i18n（4 语言）→ 任务 1 ✅
- ⑦ 测试 → 任务 7 ✅
- 禁用态灰显 + tooltip → 任务 4（disabled + tooltipText）✅
- 重启 toast → 任务 3（onSuccess needs_restart 分支）✅
- 状态呈现（Loader2/脉冲）→ 任务 4 ✅

**2. 占位符扫描：** 无 TODO/待定；每个代码步骤含完整代码。✅

**3. 类型/命名一致性：**
- `CompressionStatus.claude`（任务2）→ `useCompressionStatus` data（任务3）→ 组件 `status?.claude`（任务4）一致。
- `setCompressionForApp(appType, enabled) → boolean`（任务2）→ mutation `needsRestart`（任务3）一致。
- `enableCompressionToggle`（前端 types.ts/App.tsx gate/ProxyTabContent，任务5/6）与 `enable_compression_toggle`（后端 serde 默认 camelCase 映射，任务5）一致。
- i18n key `proxy.compression.*` / `settings.advanced.proxy.enableCompressionToggle`（任务1）与组件/设置页引用（任务4/5）一致。✅

**4. 未决小前提（实现期核对）：**
- 后端 `AppSettings` 序列化是否 camelCase（`enable_compression_toggle` → `enableCompressionToggle`）——照现有 `enable_failover_toggle` ↔ `enableFailoverToggle` 已验证一致，按同款。
- `ProxyTabContent` 的 `onAutoSave` prop 签名——照 `enableFailoverToggle` 行现有用法。
- `tests/setupTests.ts` 是否引入 `@testing-library/jest-dom`（任务7 步骤2 有兜底）。

---

## 执行交接

计划已完成并保存到 `docs/superpowers/plans/2026-07-03-headroom-compression-toggle-ui.md`。两种执行方式：

**1. 子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**
