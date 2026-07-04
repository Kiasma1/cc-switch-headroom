/**
 * Headroom 压缩切换开关组件
 *
 * 放置在主界面头部，用于启用/关闭 Headroom 上下文压缩。当前仅 Claude 支持。
 *
 * 关键语义（务必理解，避免重蹈覆辙）：
 * - 开压缩 = **全局立即生效的危险动作**：它会重写共享的 ~/.claude/settings.json，
 *   把 ANTHROPIC_BASE_URL 指向 Headroom 压缩代理（:8787）。本机**所有** Claude Code
 *   会话（含你正在用的这个）会立刻改走压缩链路，因此必须**显式确认**，绝不静默。
 * - 关压缩 = **逃生阀**：把路由退回已知可用的直连代理（:15721），零摩擦、无需确认。
 * - 就绪条件 = 已接管 Claude（takeover）。与后端 set_compression_for_app 的门禁一致。
 *   未接管时点击 → 弹确认对话框，同意后自动「接管 + 开压缩」一步到位
 *   （后端 set_takeover_for_app 会自动拉起代理服务）。
 */

import { useEffect, useRef, useState } from "react";
import { Minimize2, Loader2 } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import {
  useCompressionStatus,
  useSetCompressionForApp,
} from "@/lib/query/compression";
import { compressionApi } from "@/lib/api/compression";
import { useProxyStatus } from "@/hooks/useProxyStatus";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import type { AppId } from "@/lib/api";

interface CompressionToggleProps {
  className?: string;
  activeApp: AppId;
}

const HEADROOM_ADDRESS = "http://127.0.0.1:8787";
const PROXY_ADDRESS = "http://127.0.0.1:15721";

export function CompressionToggle({
  className,
  activeApp,
}: CompressionToggleProps) {
  const { t } = useTranslation();
  const { data: status, isLoading } = useCompressionStatus();
  const isEnabled = status?.claude ?? false;
  const setCompression = useSetCompressionForApp();
  const { takeoverStatus, setTakeoverForApp, isPending: isProxyPending } =
    useProxyStatus();
  const takeoverEnabled = takeoverStatus?.[activeApp] ?? false;

  const [showConfirm, setShowConfirm] = useState(false);

  // 当前仅 Claude 支持压缩
  const supported = activeApp === "claude";
  // 就绪 = 受支持 + 已接管（与后端门禁一致）
  const ready = supported && takeoverEnabled;
  // 压缩真正生效需同时满足：受支持 + 已接管 + 开关已开
  const active = supported && takeoverEnabled && isEnabled;
  // 正在执行开启流程（自动接管 + 起压缩），用于"启动中…"反馈
  const enabling = setCompression.isPending || isProxyPending;
  const busy = enabling || isLoading;

  // A. 预热 Headroom：一旦就绪且压缩未开，后台提前 spawn Headroom，
  //    把 ~17s 的 Python 冷启动挪到用户点开关之前，使随后开压缩秒过。
  //    只在 ready 上升沿触发一次；关压缩(逃生阀)后不自动重热，尊重用户"停"的意图。
  const prewarmedRef = useRef(false);
  useEffect(() => {
    if (!ready) {
      prewarmedRef.current = false;
      return;
    }
    if (active || prewarmedRef.current) return;
    prewarmedRef.current = true;
    compressionApi.prewarmHeadroom().catch((e) => {
      // 预热失败不打扰用户（开压缩时仍会正常尝试启动），仅允许下次重试
      console.warn("[CompressionToggle] prewarm headroom failed:", e);
      prewarmedRef.current = false;
    });
  }, [ready, active]);

  const handleToggle = (checked: boolean) => {
    if (!supported) {
      toast.info(
        t("proxy.compression.unsupported", {
          defaultValue: "当前仅 Claude 支持 Headroom 压缩",
        }),
        { closeButton: true },
      );
      return;
    }
    // 关：逃生阀，秒关不确认
    if (!checked) {
      setCompression.mutate({ appType: activeApp, enabled: false });
      return;
    }
    // 开：全局立即生效的危险动作，必须显式确认
    setShowConfirm(true);
  };

  const handleConfirm = async () => {
    setShowConfirm(false);
    try {
      // 未接管则先接管（后端会自动拉起代理服务），再开压缩
      if (!takeoverEnabled) {
        await setTakeoverForApp({ appType: "claude", enabled: true });
      }
      await setCompression.mutateAsync({ appType: "claude", enabled: true });
    } catch (error) {
      console.error("[CompressionToggle] enable compression failed:", error);
    }
  };

  const tooltipText = !supported
    ? t("proxy.compression.unsupported", {
        defaultValue: "当前仅 Claude 支持 Headroom 压缩",
      })
    : active
      ? t("proxy.compression.tooltip.enabled", {
          headroomAddress: HEADROOM_ADDRESS,
          defaultValue: `当前流量：Claude → Headroom（${HEADROOM_ADDRESS}）→ 本地代理（${PROXY_ADDRESS}）→ 供应商。点击关闭压缩`,
        })
      : ready
        ? t("proxy.compression.tooltip.readyOff", {
            headroomAddress: HEADROOM_ADDRESS,
            defaultValue: `点击开启 Headroom 压缩（会改走 ${HEADROOM_ADDRESS}，影响本机所有 Claude 会话）`,
          })
        : t("proxy.compression.tooltip.needsTakeover", {
            defaultValue:
              "点击后将确认「接管 Claude + 开启压缩」，这会改写本机所有 Claude 会话的路由",
          });

  return (
    <>
      <div
        className={cn(
          "flex items-center gap-1 px-1.5 h-8 rounded-lg transition-all",
          active
            ? "bg-emerald-500/10 ring-1 ring-emerald-500/30"
            : "bg-muted/50",
          !ready && !active && "opacity-50",
          className,
        )}
        title={tooltipText}
      >
        {busy ? (
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
        ) : (
          <Minimize2
            className={cn(
              "h-4 w-4 transition-colors",
              active ? "text-emerald-500 animate-pulse" : "text-muted-foreground",
            )}
          />
        )}
        {enabling ? (
          <span className="text-xs font-medium text-muted-foreground whitespace-nowrap">
            {t("proxy.compression.starting", { defaultValue: "启动中…" })}
          </span>
        ) : active ? (
          <span className="text-xs font-medium text-emerald-600 dark:text-emerald-400 whitespace-nowrap">
            {t("proxy.compression.activeBadge", { defaultValue: "压缩中" })}
          </span>
        ) : null}
        <Switch
          checked={active}
          onCheckedChange={handleToggle}
          aria-label={t("proxy.compression.tooltip.readyOff", {
            defaultValue: "点击开启 Headroom 压缩",
          })}
        />
      </div>

      <ConfirmDialog
        isOpen={showConfirm}
        variant="destructive"
        title={t("proxy.compression.confirm.title", {
          defaultValue: "开启 Headroom 压缩？",
        })}
        message={
          takeoverEnabled
            ? t("proxy.compression.confirm.messageCompressionOnly", {
                headroomAddress: HEADROOM_ADDRESS,
                defaultValue: `确认后将启动 Headroom 压缩代理，并把 Claude 的全局路由切到 ${HEADROOM_ADDRESS}。\n\n⚠️ 这会立即改写本机共享的 ~/.claude 配置，所有正在运行的 Claude Code 会话（包括你当前这个）会马上改走压缩链路。建议确认后重启 Claude Code 以干净生效。`,
              })
            : t("proxy.compression.confirm.messageWithTakeover", {
                headroomAddress: HEADROOM_ADDRESS,
                defaultValue: `确认后将依次：\n① 接管 Claude 全局配置（自动启动本地代理）\n② 启动 Headroom 压缩代理并把路由切到 ${HEADROOM_ADDRESS}\n\n⚠️ 这会立即改写本机共享的 ~/.claude 配置，所有正在运行的 Claude Code 会话（包括你当前这个）会马上改走压缩链路。建议确认后重启 Claude Code 以干净生效。`,
              })
        }
        confirmText={t("proxy.compression.confirm.confirm", {
          defaultValue: "确认开启",
        })}
        onConfirm={() => void handleConfirm()}
        onCancel={() => setShowConfirm(false)}
      />
    </>
  );
}
