import { getVersion } from "@tauri-apps/api/app";

export type UpdateChannel = "stable" | "beta";

export interface UpdateInfo {
  currentVersion: string;
  availableVersion: string;
  notes?: string;
  pubDate?: string;
}

export interface CheckOptions {
  timeout?: number;
  channel?: UpdateChannel;
}

export async function getCurrentVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch {
    return "";
  }
}

export async function checkForUpdate(
  _opts: CheckOptions = {},
): Promise<
  { status: "up-to-date" } | { status: "available"; info: UpdateInfo }
> {
  // 个人定制版（cc-switch-headroom）已彻底禁用 in-app 自动更新：
  // 上游 updater endpoints 指向 farion1231/cc-switch，若走更新会把带 Headroom
  // 压缩功能的定制版覆盖成上游普通版。此处恒返回 up-to-date，
  // 使 hasUpdate 永为 false —— header 更新徽标自动隐藏、
  // AboutSection 的 installUpdateAndRestart 分支成为不可达死代码。
  // 更新方式改为：git pull + 本地 rebuild。
  return { status: "up-to-date" };
}
