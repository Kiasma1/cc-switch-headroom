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

  /**
   * 预热 Headroom：仅 spawn/复用进程（start() 不等待就绪），把 Python 冷启动
   * （实测约 17s）挪到后台提前跑，使随后开压缩时 set_compression 的就绪轮询能
   * 立刻命中、开关秒过。幂等：已在跑则直接复用。
   */
  async prewarmHeadroom(): Promise<void> {
    return invoke("headroom_start");
  },
};
