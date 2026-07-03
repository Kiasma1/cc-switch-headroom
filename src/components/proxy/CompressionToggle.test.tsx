import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CompressionToggle } from "./CompressionToggle";
import { toast } from "sonner";

// mock i18n：t 直接返回 defaultValue 或 key
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) =>
      opts?.defaultValue ?? key,
  }),
}));

// mock sonner toast
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), warning: vi.fn(), error: vi.fn() },
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

// mock 设置（默认本地代理开 + 接管开，save 返回 resolved）
let localProxyOn = true;
const saveSettingsAsync = vi.fn().mockResolvedValue(undefined);
vi.mock("@/lib/query", () => ({
  useSettingsQuery: () => ({
    data: { enableLocalProxy: localProxyOn, enableCompressionToggle: true },
  }),
  useSaveSettingsMutation: () => ({
    mutateAsync: saveSettingsAsync,
    isPending: false,
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
    saveSettingsAsync.mockClear();
    (toast.info as ReturnType<typeof vi.fn>).mockClear();
    compressionEnabled = false;
    takeoverOn = true;
    localProxyOn = true;
  });

  it("全部就绪时，点击开关调用 set_compression_for_app", async () => {
    renderToggle("claude");
    const sw = screen.getByRole("switch");
    await fireEvent.click(sw);
    expect(setCompressionMutate).toHaveBeenCalledWith({
      appType: "claude",
      enabled: true,
    });
  });

  it("本地代理关闭时，点击自动开启本地路由并提示接管", async () => {
    localProxyOn = false;
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    expect(saveSettingsAsync).toHaveBeenCalledWith(
      expect.objectContaining({ enableLocalProxy: true }),
    );
    expect(toast.info).toHaveBeenCalled();
    // 本地代理未就绪时不触达后端压缩命令
    expect(setCompressionMutate).not.toHaveBeenCalled();
  });

  it("接管关闭时，点击提示接管 Claude，不触达后端", async () => {
    takeoverOn = false;
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    expect(toast.info).toHaveBeenCalled();
    expect(setCompressionMutate).not.toHaveBeenCalled();
  });
});
