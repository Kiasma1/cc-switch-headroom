import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

// mock 压缩 api（预热）—— 用 vi.hoisted 避免 vi.mock 提升导致的 TDZ
const { prewarmHeadroom } = vi.hoisted(() => ({
  prewarmHeadroom: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@/lib/api/compression", () => ({
  compressionApi: { prewarmHeadroom },
}));

// mock 压缩状态 + mutation
const setCompressionMutate = vi.fn();
const setCompressionAsync = vi.fn().mockResolvedValue(true);
let compressionEnabled = false;
vi.mock("@/lib/query/compression", () => ({
  useCompressionStatus: () => ({
    data: { claude: compressionEnabled },
    isLoading: false,
  }),
  useSetCompressionForApp: () => ({
    mutate: setCompressionMutate,
    mutateAsync: setCompressionAsync,
    isPending: false,
  }),
}));

// mock 代理/接管状态
let takeoverOn = true;
const setTakeoverForApp = vi.fn().mockResolvedValue(undefined);
vi.mock("@/hooks/useProxyStatus", () => ({
  useProxyStatus: () => ({
    takeoverStatus: { claude: takeoverOn },
    setTakeoverForApp,
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

/** 找确认对话框里的「确认开启」按钮 */
function getConfirmButton() {
  return screen.getByRole("button", { name: "确认开启" });
}

describe("CompressionToggle", () => {
  beforeEach(() => {
    setCompressionMutate.mockClear();
    setCompressionAsync.mockClear();
    setTakeoverForApp.mockClear();
    prewarmHeadroom.mockClear();
    (toast.info as ReturnType<typeof vi.fn>).mockClear();
    compressionEnabled = false;
    takeoverOn = true;
  });

  it("就绪且未压缩时：挂载即预热 Headroom", async () => {
    takeoverOn = true;
    compressionEnabled = false;
    renderToggle("claude");
    await waitFor(() => expect(prewarmHeadroom).toHaveBeenCalledTimes(1));
  });

  it("未接管（未就绪）时：不预热", async () => {
    takeoverOn = false;
    renderToggle("claude");
    // 给 effect 一个执行窗口
    await new Promise((r) => setTimeout(r, 0));
    expect(prewarmHeadroom).not.toHaveBeenCalled();
  });

  it("已压缩(active)时：不重复预热", async () => {
    takeoverOn = true;
    compressionEnabled = true;
    renderToggle("claude");
    await new Promise((r) => setTimeout(r, 0));
    expect(prewarmHeadroom).not.toHaveBeenCalled();
  });

  it("开压缩：先弹确认对话框，不直接触达后端", async () => {
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    // 点开关只弹确认框，尚未调用后端
    expect(setCompressionAsync).not.toHaveBeenCalled();
    expect(getConfirmButton()).toBeInTheDocument();
  });

  it("确认后（已接管）：只开压缩，不重复接管", async () => {
    takeoverOn = true;
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    await fireEvent.click(getConfirmButton());
    await waitFor(() =>
      expect(setCompressionAsync).toHaveBeenCalledWith({
        appType: "claude",
        enabled: true,
      }),
    );
    expect(setTakeoverForApp).not.toHaveBeenCalled();
  });

  it("确认后（未接管）：先自动接管再开压缩", async () => {
    takeoverOn = false;
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    await fireEvent.click(getConfirmButton());
    await waitFor(() =>
      expect(setTakeoverForApp).toHaveBeenCalledWith({
        appType: "claude",
        enabled: true,
      }),
    );
    expect(setCompressionAsync).toHaveBeenCalledWith({
      appType: "claude",
      enabled: true,
    });
  });

  it("关压缩：逃生阀，秒关不弹确认", async () => {
    compressionEnabled = true;
    takeoverOn = true;
    renderToggle("claude");
    await fireEvent.click(screen.getByRole("switch"));
    expect(setCompressionMutate).toHaveBeenCalledWith({
      appType: "claude",
      enabled: false,
    });
    // 关不弹确认框
    expect(
      screen.queryByRole("button", { name: "确认开启" }),
    ).not.toBeInTheDocument();
  });

  it("非 Claude：点击提示不支持，不触达后端", async () => {
    renderToggle("codex");
    await fireEvent.click(screen.getByRole("switch"));
    expect(toast.info).toHaveBeenCalled();
    expect(setCompressionMutate).not.toHaveBeenCalled();
    expect(setCompressionAsync).not.toHaveBeenCalled();
  });
});
