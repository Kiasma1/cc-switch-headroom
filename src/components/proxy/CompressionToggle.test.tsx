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
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
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

// mock 设置（默认本地代理开）
vi.mock("@/lib/query", () => ({
  useSettingsQuery: () => ({ data: { enableLocalProxy: true } }),
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
