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
