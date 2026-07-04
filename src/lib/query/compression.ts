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
      // 压缩切换是**全局立即生效**：base URL 已写入共享的 ~/.claude，
      // 本机所有 Claude 会话已改走新路由。needsRestart 仅表示"建议重启以
      // 干净生效"，不是"改动尚未生效"——文案必须诚实，不能谎报"请重启生效"。
      if (needsRestart && variables.enabled) {
        toast.warning(
          t("proxy.compression.globalRerouted", {
            defaultValue:
              "已切换全局路由：本机所有 Claude 会话现在改走压缩链路。建议重启 Claude Code 以干净生效。",
          }),
          { duration: 6000, closeButton: true },
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
