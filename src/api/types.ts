// ============================================================================
// src/api/types.ts —— IPC 契约类型层（结构占位）
//
// 职责：镜像后端 `src-tauri/src/models.rs` 中所有跨 IPC 的类型定义。
//       业务组件与各 api 模块都应从这里 import 类型，禁止在别处手写重复定义。
//
// 命名铁律（联调时最容易踩坑，务必遵守）：
//   - 后端普通 struct 字段未配 #[serde(rename_all)]，序列化后仍是 snake_case
//     （如 task_id / node_token / current_doc / download_images / obj_type ...）
//   - 后端枚举已配 `#[serde(rename_all = "snake_case")]`
//     （如 TaskPhase: Queued/CheckingOutput/ScanningWiki/ExportingDocument/...）
//   - invoke 的【入参】key 才是 camelCase（wikiUrl / outputDir / selectedTokens ...）
//   - 即：进 camelCase，出（字段名）snake_case。
//
// 填充时机：M0（契约冻结后即可把 models.rs 的公共类型搬运到此）。
// 参考对象：models.rs 中的 EnvStatus / DeviceInfo / LoginResult / Settings /
//           SettingsStatus / OutputPreflight / WikiNode / Progress / WikiTaskResult /
//           PreviewResult / ExtractResult / WikiExtractResult 及各类枚举。
// ============================================================================

export {};
