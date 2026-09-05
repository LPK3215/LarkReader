// ============================================================================
// src/stores/settings.ts —— 应用设置状态
//
// state : settings(Settings) / warning / preflight(OutputPreflight)
// actions: load()      get_settings_status（Tauri 环境启动加载）
//          save()      set_settings
//          pickDir()   dialog 选目录 + preflight_output_dir 校验
//          openDir()   open_output_dir 打开系统文件管理器
//
// 环境检测：isTauri() 为 false 时保留默认值（dev demo）。
//           concurrency 合法范围 1–32（后端 Settings::validate 强制）。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { isTauri } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { OutputPreflight, Settings } from "../api/types";
import {
  getSettingsStatus,
  openOutputDir,
  preflightOutputDir,
  setSettings,
} from "../api/settings";

const DEFAULT_SETTINGS: Settings = {
  output_dir: "D:\\Documents\\LarkReader",
  concurrency: 5,
  download_images: true,
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>({ ...DEFAULT_SETTINGS });
  const warning = ref<string | null>(null);
  const preflight = ref<OutputPreflight | null>({
    path: DEFAULT_SETTINGS.output_dir,
    writable: true,
    available_bytes: 137_438_953_472,
  });

  const availableText = computed(() => {
    const bytes = preflight.value?.available_bytes ?? 0;
    const gb = bytes / 1024 ** 3;
    return gb >= 1024 ? `${(gb / 1024).toFixed(1)} TB` : `${Math.round(gb)} GB`;
  });

  /** 启动时调一次：Tauri 拉真实设置；dev 演示保留默认。 */
  async function load() {
    if (!isTauri()) return;
    try {
      const status = await getSettingsStatus();
      settings.value = status.settings;
      warning.value = status.warning;
      await refreshPreflight(status.settings.output_dir);
    } catch (err) {
      warning.value = String(err);
    }
  }

  /** 写盘 + 更新本地状态。 */
  async function save(next?: Partial<Settings>) {
    const merged = { ...settings.value, ...next };
    if (isTauri()) {
      try {
        await setSettings(merged);
        await refreshPreflight(merged.output_dir);
        settings.value = merged;
        warning.value = null;
      } catch (err) {
        warning.value = String(err);
        throw err;
      }
    } else {
      settings.value = merged;
    }
  }

  /** 仅更新本地草稿，不写盘（用于表单实时预览）。 */
  function draft(patch: Partial<Settings>) {
    settings.value = { ...settings.value, ...patch };
  }

  /** dialog 选目录 → preflight 校验 → 写 settings。 */
  async function pickDir() {
    if (!isTauri()) {
      warning.value = "浏览器演示模式下无法弹出系统目录选择器";
      return;
    }
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    await refreshPreflight(picked);
    await save({ output_dir: picked });
  }

  /** 重新做 preflight（路径切换后）。 */
  async function refreshPreflight(path: string) {
    if (!isTauri()) return;
    try {
      preflight.value = await preflightOutputDir(path);
    } catch (err) {
      warning.value = String(err);
      preflight.value = null;
    }
  }

  /** 在系统文件管理器里打开输出目录。 */
  async function openDir() {
    if (!isTauri()) {
      warning.value = "浏览器演示模式下无法打开系统目录";
      return;
    }
    try {
      await openOutputDir(settings.value.output_dir);
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
    draft,
    pickDir,
    openDir,
  };
});