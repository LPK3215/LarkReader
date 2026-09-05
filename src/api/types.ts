// ============================================================================
// src/api/types.ts —— IPC 契约类型层
//
// 镜像后端 `src-tauri/src/models.rs` 中所有跨 IPC 的类型定义。
// 业务组件与各 api 模块都从这里 import，禁止在别处手写重复定义。
//
// 命名铁律（联调最容易踩坑）：
//   - invoke 的【入参】key 是 camelCase（wikiUrl / outputDir / selectedTokens）
//   - 返回的【struct 字段】是 snake_case（models.rs 未配 rename_all）
//   - 返回的【枚举值】是 snake_case（枚举配了 rename_all）
//   即：进 camelCase，出 snake_case。
//
// 因此下面的 interface 直接用 snake_case 字段名，与后端 JSON 逐字对齐；
// 只有准备 invoke 入参时才转成 camelCase（见各 api/*.ts 的封装）。
// ============================================================================

// ---------------------------------------------------------------------------
// 环境检测
// ---------------------------------------------------------------------------

export interface EnvCheckError {
  component: string;
  message: string;
}

export interface EnvStatus {
  node_installed: boolean;
  node_version: string | null;
  lark_cli_installed: boolean;
  lark_cli_version: string | null;
  lark_cli_compatible: boolean;
  app_configured: boolean;
  app_id: string | null;
  logged_in: boolean;
  user_name: string | null;
  token_status: string | null;
  check_errors: EnvCheckError[];
}

// ---------------------------------------------------------------------------
// 登录
// ---------------------------------------------------------------------------

export interface DeviceInfo {
  device_code: string;
  verification_url: string;
}

export interface LoginResult {
  success: boolean;
  user_name: string | null;
  error: string | null;
}

// ---------------------------------------------------------------------------
// 文档预览与单篇导出
// ---------------------------------------------------------------------------

export interface ImageRef {
  alt: string;
  url: string;
  file_token: string;
}

export type ExtractStatus = "success" | "partial" | "failed";

export interface PreviewResult {
  title: string;
  content_markdown: string;
  images: ImageRef[];
  char_count: number;
}

export interface ExtractResult {
  title: string;
  filename: string;
  char_count: number;
  image_count: number;
  images_downloaded: number;
  images_failed: number;
  filepath: string;
  status: ExtractStatus;
  errors: string[];
}

// ---------------------------------------------------------------------------
// Wiki 节点树
// ---------------------------------------------------------------------------

export type WikiNodeType = "doc" | "sheet" | "bitable" | "folder" | "file" | "other";

export interface WikiNode {
  node_token: string;
  title: string;
  obj_type: WikiNodeType;
  has_child: boolean;
  obj_token: string | null;
  position: number;
  depth: number;
  children: WikiNode[];
}

/** 勾选范围内真实会被导出的条目数（count_exportable 返回值），与任务进度 total 口径一致 */
export interface ExportableCount {
  total: number;
  doc: number;
  sheet: number;
  bitable: number;
  file: number;
  other: number;
}

// ---------------------------------------------------------------------------
// 批量导出结果
// ---------------------------------------------------------------------------

export type ExportItemStatus = "success" | "partial" | "failed" | "skipped";

export interface ExportItemResult {
  title: string;
  node_token: string | null;
  obj_type: WikiNodeType;
  status: ExportItemStatus;
  paths: string[];
  message: string | null;
}

export interface DocFailure {
  title: string;
  node_token: string;
  error: string;
}

export interface SpecialExport {
  title: string;
  node_token: string;
  obj_type: WikiNodeType;
  paths: string[];
}

export interface SpecialExportFailure {
  title: string;
  node_token: string;
  obj_type: WikiNodeType;
  error: string;
}

export interface SkippedNode {
  title: string;
  node_token: string;
  obj_type: WikiNodeType;
  reason: string;
}

export interface WikiExtractResult {
  wiki_name: string;
  output_root: string;
  total: number;
  success_count: number;
  failed_count: number;
  partial_count: number;
  results: ExtractResult[];
  failures: DocFailure[];
  skipped_count: number;
  skipped: SkippedNode[];
  exports: SpecialExport[];
  export_failures: SpecialExportFailure[];
  cancelled: boolean;
  completed_count: number;
  items: ExportItemResult[];
}

// ---------------------------------------------------------------------------
// 任务进度
// ---------------------------------------------------------------------------

export type TaskPhase =
  | "queued"
  | "checking_output"
  | "scanning_wiki"
  | "exporting_document"
  | "exporting_sheet"
  | "exporting_bitable"
  | "exporting_file"
  | "finalizing"
  | "finished";

export type TaskStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export interface Progress {
  task_id: string;
  total: number;
  done: number;
  current_doc: string | null;
  current_path: string | null;
  success_count: number;
  failed_count: number;
  errors: string[];
  status: TaskStatus;
  phase: TaskPhase;
  current_item_type: WikiNodeType | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  elapsed_seconds: number;
  estimated_remaining_seconds: number | null;
}

export interface WikiTaskResult {
  task_id: string;
  progress: Progress;
  result: WikiExtractResult | null;
  error: string | null;
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

export interface Settings {
  output_dir: string;
  concurrency: number;
  download_images: boolean;
}

export interface OutputPreflight {
  path: string;
  writable: boolean;
  available_bytes: number;
}

export interface SettingsStatus {
  settings: Settings;
  warning: string | null;
}

// ---------------------------------------------------------------------------
// 运行日志
// ---------------------------------------------------------------------------

export interface LogFileMeta {
  name: string;
  size_bytes: number;
  modified_at: string | null;
}

/** 飞书应用创建向导实时状态（start_app_init / get_app_init_status） */
export interface AppInitStatus {
  running: boolean;
  stage: string;
  url: string | null;
  message: string | null;
  error: string | null;
}

export interface LogFileContent {
  name: string;
  content: string;
  size_bytes: number;
  truncated: boolean;
}

// ---------------------------------------------------------------------------
// 本地阅读（Reader）
// ---------------------------------------------------------------------------

export type ReaderEntryKind = "dir" | "md" | "other";

/** list_reader_dir 返回：目录中的一项（一次一层，惰性加载） */
export interface ReaderEntry {
  name: string;
  path: string;
  kind: ReaderEntryKind;
  size_bytes: number | null;
}

/** read_reader_binary 返回：可直接赋给 <img src> 的 data URL */
export interface ReaderBinary {
  data_url: string;
}

// ---------------------------------------------------------------------------
// 统一错误（error.rs 的结构化协议）
// ---------------------------------------------------------------------------

export interface AppErrorPayload {
  code: string;
  message: string;
  retryable: boolean;
}
