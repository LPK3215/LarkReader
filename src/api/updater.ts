// ============================================================================
// src/api/updater.ts —— 应用更新检查与安装
//
// 依赖 tauri-plugin-updater（Rust 侧已注册，端点/公钥在 tauri.conf.json）：
//   - 端点：GitHub Releases 的 latest.json（tauri-action 每次发版自动上传）
//   - 校验：内嵌公钥验证下载包签名，公钥在仓库内、私钥只存 GitHub secret
//
// 用法：
//   checkForUpdate()           手动检查，返回 up-to-date / available / error
//   downloadAndInstallUpdate() 下载并安装（Win 会自动弹安装器并退出；mac/Linux 安装后自动重启）
//   notifyUpdateOnce()         启动时静默检查一次，有新版本时回调提示（防重复打扰）
// ============================================================================

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/api/process";
import { check } from "@tauri-apps/plugin-updater";

/** 检查结果：与后端约定互不依赖，纯粹消费 GitHub latest.json */
export type UpdateCheckResult =
  | { kind: "up-to-date"; currentVersion: string }
  | { kind: "available"; currentVersion: string; nextVersion: string }
  | { kind: "error"; currentVersion: string; message: string };

export interface UpdateProgress {
  /** 已下载字节数 */
  downloaded: number;
  /** 总量（部分平台/阶段可能未知） */
  total?: number;
  /** 是否已完成 */
  finished: boolean;
}

const CHECK_TIMEOUT_MS = 10_000;

/** 当前运行版本（读自打包进应用的版本号） */
export async function getCurrentVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch {
    return "";
  }
}

/** 把 Tauri/网络抛出的错误整理成人类可读文案 */
function humanizeError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  const m = raw.match(/"message":"([^"]*)"/);
  return m ? m[1] : raw;
}

/** 手动检查一次是否有新版本。超时/网络失败都归为 error，不向上抛。 */
export async function checkForUpdate(): Promise<UpdateCheckResult> {
  const currentVersion = await getCurrentVersion();
  try {
    const update = await check({ timeout: CHECK_TIMEOUT_MS });
    if (!update) return { kind: "up-to-date", currentVersion };
    return {
      kind: "available",
      currentVersion,
      nextVersion: update.version,
    };
  } catch (err) {
    return { kind: "error", currentVersion, message: humanizeError(err) };
  }
}

/**
 * 下载并安装新版本。
 * - Windows：安装器接管后会退出应用（此函数可能不返回）；
 * - macOS / Linux：安装完成后调用 relaunch 重启生效。
 */
export async function downloadAndInstallUpdate(
  onProgress?: (progress: UpdateProgress) => void
): Promise<void> {
  const update = await check({ timeout: CHECK_TIMEOUT_MS });
  if (!update) return; // 竞态：检查后已被别处消费，视为无需处理

  let downloaded = 0;
  let total: number | undefined;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength;
      onProgress?.({ downloaded: 0, total, finished: false });
    } else if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress?.({ downloaded, total, finished: false });
    }
  });
  onProgress?.({ downloaded, total, finished: true });
  await relaunch();
}

let notifyOnceRan = false;

/**
 * 启动时静默检查一次：有新版本才回调 onAvailable，其它情况（无更新/失败）一律静默。
 * 用于应用壳层的“内部提示更新”，整个进程生命周期只提示一次，避免每次启动打扰。
 */
export async function notifyUpdateOnce(
  onAvailable: (currentVersion: string, nextVersion: string) => void
): Promise<void> {
  if (notifyOnceRan) return;
  notifyOnceRan = true;
  const result = await checkForUpdate();
  if (result.kind === "available") {
    onAvailable(result.currentVersion, result.nextVersion);
  }
}
