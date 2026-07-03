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
import { useSettingsQuery } from "@/lib/query";
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
  const { data: settings } = useSettingsQuery();
  const localProxyEnabled = settings?.enableLocalProxy ?? false;

  // 当前仅 Claude 支持压缩
  const supported = activeApp === "claude";
  const disabled =
    setCompression.isPending || isLoading || !takeoverEnabled || !supported;
  // 压缩真正生效需同时满足：应用受支持 + 已接管 + 开关已开
  const active = supported && takeoverEnabled && isEnabled;

  const handleToggle = (checked: boolean) => {
    if (checked && (!takeoverEnabled || !supported)) return;
    setCompression.mutate({ appType: activeApp, enabled: checked });
  };

  const headroomAddress = "http://127.0.0.1:9749";
  const tooltipText = !supported
    ? t("proxy.compression.unsupported", {
        defaultValue: "当前仅 Claude 支持 Headroom 压缩",
      })
    : !localProxyEnabled
      ? t("proxy.compression.localProxyRequired", {
          defaultValue:
            "请先在「设置 → 高级 → 本地代理」开启「在主页面显示本地路由开关」",
        })
      : !takeoverEnabled
        ? t("proxy.compression.takeoverRequired", {
            defaultValue: "请先打开左侧「代理」开关接管 Claude，再启用压缩",
          })
        : active
          ? t("proxy.compression.tooltip.enabled", {
              headroomAddress,
              defaultValue:
                "当前走 Headroom（http://127.0.0.1:9749），点击关闭压缩",
            })
          : t("proxy.compression.tooltip.disabled", {
              headroomAddress,
              defaultValue: "点击启用 Headroom 压缩（http://127.0.0.1:9749）",
            });

  return (
    <div
      className={cn(
        "flex items-center gap-1 px-1.5 h-8 rounded-lg bg-muted/50 transition-all",
        disabled && "opacity-50",
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
            active
              ? "text-emerald-500 animate-pulse"
              : "text-muted-foreground",
          )}
        />
      )}
      <Switch
        checked={active}
        onCheckedChange={handleToggle}
        disabled={disabled}
        aria-label={t("proxy.compression.tooltip.disabled", {
          defaultValue: "点击启用 Headroom 压缩",
        })}
      />
    </div>
  );
}
