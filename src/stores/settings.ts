// ============================================================================
// src/stores/settings.ts —— 应用设置状态
//
// state : settings(Settings) / warning / preflight(OutputPreflight)
// actions: load()      get_settings_status（Tauri 启动时拉真实设置）
//          save()      set_settings
//          pickDir()   dialog 选目录 + preflight_output_dir 校验
//          openDir(p)  open_output_dir 打开系统文件管理器（可指定具体目录）
//
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// concurrency 合法范围 1–32（后端 Settings::validate 强制）。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
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
  const preflight = ref<OutputPreflight | null>(null);

  const availableText = computed(() => {
    const bytes = preflight.value?.available_bytes ?? 0;
    const gb = bytes / 1024 ** 3;
    return gb >= 1024 ? `${(gb / 1024).toFixed(1)} TB` : `${Math.round(gb)} GB`;
  });

  /** 启动时调一次：拉真实设置并预检输出目录。 */
  async function load() {
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
    try {
      await setSettings(merged);
      await refreshPreflight(merged.output_dir);
      settings.value = merged;
      warning.value = null;
    } catch (err) {
      warning.value = String(err);
      throw err;
    }
  }

  /** 仅更新本地草稿，不写盘（用于表单实时预览）。 */
  function draft(patch: Partial<Settings>) {
    settings.value = { ...settings.value, ...patch };
  }

  /** dialog 选目录 → preflight 校验 → 写 settings。 */
  async function pickDir() {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    await refreshPreflight(picked);
    await save({ output_dir: picked });
  }

  /** 重新做 preflight（路径切换后）。 */
  async function refreshPreflight(path: string) {
    try {
      preflight.value = await preflightOutputDir(path);
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
    draft,
    pickDir,
    openDir,
  };
});