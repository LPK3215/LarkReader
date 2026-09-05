// ============================================================================
// src/stores/settings.ts —— 应用设置状态
//
// state : settings(Settings) / warning / preflight(OutputPreflight)
// actions: load()      get_settings_status（Tauri 启动时拉真实设置）
//          save()      set_settings（成功后再预检新目录）
//          openDir(p)  open_output_dir 打开系统文件管理器（可指定具体目录）
//          refreshPreflight(p) 目录可写性/可用空间预检（可选保留旧告警）
//
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// concurrency 合法范围 1–32（后端 Settings::validate 强制）。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import type { OutputPreflight, Settings } from "../api/types";
import {
  getSettingsStatus,
  openOutputDir,
  preflightOutputDir,
  setSettings,
} from "../api/settings";

export const DEFAULT_SETTINGS: Settings = {
  output_dir: "D:\\Documents\\LarkReader",
  concurrency: 5,
  download_images: true,
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>({ ...DEFAULT_SETTINGS });
  const warning = ref<string | null>(null);
  const preflight = ref<OutputPreflight | null>(null);

  /** 可读的可用空间文案；未预检时为 null（界面隐藏该行）。 */
  const availableText = computed<string | null>(() => {
    if (!preflight.value) return null;
    const bytes = preflight.value.available_bytes ?? 0;
    const gb = bytes / 1024 ** 3;
    if (gb >= 1024) return `${(gb / 1024).toFixed(1)} TB`;
    // 小盘/近满时保留 1 位小数，避免 0.3GB 被四舍五入成误导性的「0 GB」
    return gb < 10 ? `${gb.toFixed(1)} GB` : `${Math.round(gb)} GB`;
  });

  /** 启动时调一次：拉真实设置并预检输出目录。 */
  async function load() {
    try {
      const status = await getSettingsStatus();
      settings.value = status.settings;
      warning.value = status.warning;
      // keepWarning：后端可能带配置层告警（如配置损坏已恢复默认），不能被预检成功清掉
      await refreshPreflight(status.settings.output_dir, true);
    } catch (err) {
      warning.value = String(err);
    }
  }

  /** 写盘 + 更新本地状态。 */
  async function save(next?: Partial<Settings>) {
    const merged = { ...settings.value, ...next };
    try {
      await setSettings(merged);
      settings.value = merged;
      warning.value = null;
      // 预检放最后：它只在失败时写 warning，成功只更新空间/可写性，不会清掉上面的结果
      await refreshPreflight(merged.output_dir);
    } catch (err) {
      warning.value = String(err);
      throw err;
    }
  }

  /**
   * 重新做 preflight（路径切换后）。
   * @param keepWarning 预检成功时是否保留现有告警（load() 传 true 以保留后端配置层告警）
   */
  async function refreshPreflight(path: string, keepWarning = false) {
    try {
      preflight.value = await preflightOutputDir(path);
      if (!keepWarning) warning.value = null;
    } catch (err) {
      warning.value = String(err);
      preflight.value = null;
    }
  }

  /** 在系统文件管理器里打开目录；不传路径时打开当前输出目录。 */
  async function openDir(path?: string) {
    try {
      await openOutputDir(path ?? settings.value.output_dir);
    } catch (err) {
      warning.value = String(err);
    }
  }

  return {
    settings,
    warning,
    preflight,
    availableText,
    load,
    save,
    refreshPreflight,
    openDir,
  };
});