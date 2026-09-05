// ============================================================================
// src/stores/settings.ts —— 应用设置状态（结构占位）
//
// 职责（M1/M2 装 pinia 后填充，defineStore('settings', ...)）：
//   state : settings(Settings) / warning(SettingsStatus.warning)
//   actions: load()   get_settings_status（启动加载）
//            update() set_settings（改默认目录 / concurrency / download_images）
//            pickDir() 调 dialog 选目录 + api/settings.ts preflight_output_dir 校验
//
// 说明：当前不 import pinia（依赖未装，避免破基线）。
// ============================================================================

export {};
