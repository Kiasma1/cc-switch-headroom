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
