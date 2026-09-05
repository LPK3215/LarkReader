// ============================================================================
// src/api/settings.ts —— 设置 / 输出目录命令封装（结构占位）
//
// 对应后端命令：
//   get_settings()                  -> Settings { output_dir, concurrency, download_images }
//   set_settings(settings)          -> ()      写前先调 validate
//   get_settings_status()           -> SettingsStatus { settings, warning }
//   preflight_output_dir(path)      -> OutputPreflight { path, writable, available_bytes }
//   open_output_dir(path)           -> ()      打开系统文件管理器
//
// 填充时机：M1（默认输出目录引导）+ M2（工作台选目录）实现。
// 约定：目录选择走 @tauri-apps/plugin-dialog 的 open() 拿到路径后再 preflight 校验；
//       open 按钮调 open_output_dir。入参 camelCase（outputDir 等）。
// ============================================================================

export {};
