//! 数据结构定义
//!
//! 包含所有后端使用的数据结构，供 Tauri 命令序列化/反序列化。

use serde::{Deserialize, Serialize};

// ============================================================================
// 环境检测相关
// ============================================================================

/// 环境检测结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvStatus {
    /// Node.js 是否已安装
    pub node_installed: bool,
    /// Node.js 版本号
    pub node_version: Option<String>,
    /// lark-cli 是否已安装
    pub lark_cli_installed: bool,
    /// lark-cli 版本号
    pub lark_cli_version: Option<String>,
    pub lark_cli_compatible: bool,
    /// 飞书应用是否已配置（config show 成功）
    pub app_configured: bool,
    /// 飞书应用 ID
    pub app_id: Option<String>,
    /// 用户是否已登录（tokenStatus == ready 或 needs_refresh）
    pub logged_in: bool,
    /// 用户名
    pub user_name: Option<String>,
    /// token 状态（ready / needs_refresh / none）
    pub token_status: Option<String>,
    pub check_errors: Vec<EnvCheckError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheckError {
    pub component: String,
    pub message: String,
}

// ============================================================================
// 文档提取相关
// ============================================================================

/// 图片引用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    /// 图片描述（Markdown 中的 alt text）
    pub alt: String,
    /// 原始 URL
    pub url: String,
    /// 从 URL 中提取的 file_token
    pub file_token: String,
}

/// 文档预览结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResult {
    /// 文档标题
    pub title: String,
    /// Markdown 正文
    pub content_markdown: String,
    /// 文档中的图片引用列表
    pub images: Vec<ImageRef>,
    /// 字符数
    pub char_count: usize,
}

/// 提取状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractStatus {
    /// 完全成功
    Success,
    /// 部分成功（有图片下载失败等）
    Partial,
    /// 失败
    Failed,
}

/// 单文档提取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResult {
    /// 文档标题
    pub title: String,
    /// 保存的文件名
    pub filename: String,
    /// 字符数
    pub char_count: usize,
    /// 图片总数
    pub image_count: usize,
    /// 成功下载的图片数
    pub images_downloaded: usize,
    /// 下载失败的图片数
    pub images_failed: usize,
    /// 保存的文件路径
    pub filepath: String,
    /// 提取状态
    pub status: ExtractStatus,
    /// 错误信息列表（部分成功时有值）
    pub errors: Vec<String>,
}

// ============================================================================
// Wiki 知识库相关
// ============================================================================

/// Wiki 节点类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WikiNodeType {
    /// 文档
    Doc,
    /// 电子表格
    Sheet,
    /// 多维表格
    Bitable,
    /// 文件夹/目录
    Folder,
    /// 上传的普通文件（zip/pdf 等），底层为 Drive file
    File,
    /// 其他类型
    Other,
}

impl WikiNodeType {
    /// 从字符串解析节点类型
    pub fn from_api_value(s: &str) -> Self {
        match s {
            "doc" | "docx" => Self::Doc,
            "sheet" => Self::Sheet,
            "bitable" => Self::Bitable,
            "folder" | "" => Self::Folder,
            "file" => Self::File,
            _ => Self::Other,
        }
    }
}

/// Wiki 节点树结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiNode {
    /// 节点 token
    pub node_token: String,
    /// 节点标题
    pub title: String,
    /// 节点类型
    pub obj_type: WikiNodeType,
    /// 是否有子节点
    pub has_child: bool,
    /// 文档的实际 token（用于 docs +fetch）
    pub obj_token: Option<String>,
    /// 在同级节点中的排序位置（飞书返回的顺序，从 0 开始）
    pub position: usize,
    /// 在树中的深度（根节点 depth=0）
    pub depth: usize,
    /// 子节点列表
    pub children: Vec<WikiNode>,
}

impl WikiNode {
    /// 创建根节点
    pub fn new_root(node_token: String, title: String) -> Self {
        Self {
            node_token,
            title,
            obj_type: WikiNodeType::Folder,
            has_child: false,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![],
        }
    }

    /// 统计该节点下所有文档节点数量
    pub fn count_docs(&self) -> usize {
        let mut count = match self.obj_type {
            WikiNodeType::Doc => 1,
            _ => 0,
        };
        for child in &self.children {
            count += child.count_docs();
        }
        count
    }

    /// 遍历树，收集所有文档节点（保留目录顺序）
    pub fn collect_docs(&self) -> Vec<&WikiNode> {
        let mut docs = Vec::new();
        if matches!(self.obj_type, WikiNodeType::Doc) {
            docs.push(self);
        }
        for child in &self.children {
            docs.extend(child.collect_docs());
        }
        docs
    }
}

/// 批量提取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiExtractResult {
    /// 知识库名称
    pub wiki_name: String,
    /// 本次知识库导出的根目录
    pub output_root: String,
    /// 总文档数
    pub total: usize,
    /// 成功数
    pub success_count: usize,
    /// 失败数
    pub failed_count: usize,
    /// 部分成功数
    pub partial_count: usize,
    /// 每个文档的提取结果
    pub results: Vec<ExtractResult>,
    /// 失败的文档列表
    pub failures: Vec<DocFailure>,
    /// 因类型暂不支持而跳过的节点数
    pub skipped_count: usize,
    /// 被跳过的节点及原因
    pub skipped: Vec<SkippedNode>,
    /// Sheet/Bitable 等特殊资源的成功导出结果
    pub exports: Vec<SpecialExport>,
    /// 特殊资源导出失败记录（与“不支持而跳过”分开）
    pub export_failures: Vec<SpecialExportFailure>,
    /// 是否由用户取消；为 true 时结果可能是部分结果
    pub cancelled: bool,
    /// 实际已处理的项目数
    pub completed_count: usize,
    /// 统一的项目结果，调用方无需分别拼接文档、表格、失败和跳过列表
    pub items: Vec<ExportItemResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportItemResult {
    pub title: String,
    pub node_token: Option<String>,
    pub obj_type: WikiNodeType,
    pub status: ExportItemStatus,
    pub paths: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportItemStatus {
    Success,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialExport {
    pub title: String,
    pub node_token: String,
    pub obj_type: WikiNodeType,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialExportFailure {
    pub title: String,
    pub node_token: String,
    pub obj_type: WikiNodeType,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedNode {
    pub title: String,
    pub node_token: String,
    pub obj_type: WikiNodeType,
    pub reason: String,
}

/// 文档提取失败记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocFailure {
    /// 文档标题
    pub title: String,
    /// 节点 token
    pub node_token: String,
    /// 错误信息
    pub error: String,
}

// ============================================================================
// 批量提取进度
// ============================================================================

/// 批量提取进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    /// 任务 ID
    pub task_id: String,
    /// 总文档数
    pub total: usize,
    /// 已完成数
    pub done: usize,
    /// 当前正在提取的文档标题
    pub current_doc: Option<String>,
    /// 当前文档所在目录路径
    pub current_path: Option<String>,
    /// 成功数
    pub success_count: usize,
    /// 失败数
    pub failed_count: usize,
    /// 错误列表
    pub errors: Vec<String>,
    /// 任务当前状态
    pub status: TaskStatus,
    /// 当前业务阶段，供调用方展示更准确的反馈
    pub phase: TaskPhase,
    /// 当前项目类型（doc/sheet/bitable）
    pub current_item_type: Option<WikiNodeType>,
    /// ISO 8601 时间
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// 已运行秒数
    pub elapsed_seconds: u64,
    /// 根据当前平均速度估算的剩余秒数；样本不足时为空
    pub estimated_remaining_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiTaskResult {
    pub task_id: String,
    pub progress: Progress,
    pub result: Option<WikiExtractResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    Queued,
    CheckingOutput,
    ScanningWiki,
    ExportingDocument,
    ExportingSheet,
    ExportingBitable,
    /// 下载文件类附件（file 节点）
    ExportingFile,
    Finalizing,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Progress {
    pub fn new(task_id: String, total: usize) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            task_id,
            total,
            done: 0,
            current_doc: None,
            current_path: None,
            success_count: 0,
            failed_count: 0,
            errors: vec![],
            status: TaskStatus::Pending,
            phase: TaskPhase::Queued,
            current_item_type: None,
            created_at: now,
            started_at: None,
            finished_at: None,
            elapsed_seconds: 0,
            estimated_remaining_seconds: None,
        }
    }

    pub fn start_phase(&mut self, phase: TaskPhase) {
        if self.started_at.is_none() {
            self.started_at = Some(chrono::Utc::now().to_rfc3339());
        }
        self.status = TaskStatus::Running;
        self.phase = phase;
        self.refresh_timing();
    }

    pub fn refresh_timing(&mut self) {
        let Some(started) = self.started_at.as_deref() else {
            return;
        };
        let Ok(started) = chrono::DateTime::parse_from_rfc3339(started) else {
            return;
        };
        self.elapsed_seconds = (chrono::Utc::now() - started.with_timezone(&chrono::Utc))
            .num_seconds()
            .max(0) as u64;
        self.estimated_remaining_seconds = if self.done > 0 && self.total > self.done {
            Some(
                self.elapsed_seconds
                    .saturating_mul((self.total - self.done) as u64)
                    / self.done as u64,
            )
        } else {
            None
        };
    }

    pub fn finish(&mut self, status: TaskStatus) {
        self.refresh_timing();
        self.status = status;
        self.phase = TaskPhase::Finished;
        self.current_doc = None;
        self.current_path = None;
        self.current_item_type = None;
        self.finished_at = Some(chrono::Utc::now().to_rfc3339());
        self.estimated_remaining_seconds = Some(0);
    }
}

// ============================================================================
// 设置
// ============================================================================

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// 默认输出目录
    pub output_dir: String,
    /// 图片并发下载数量
    pub concurrency: usize,
    /// 是否下载图片
    pub download_images: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPreflight {
    pub path: String,
    pub writable: bool,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsStatus {
    pub settings: Settings,
    pub warning: Option<String>,
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.output_dir.trim().is_empty() {
            return Err("输出目录不能为空".to_string());
        }
        if !(1..=32).contains(&self.concurrency) {
            return Err("图片并发数必须在 1 到 32 之间".to_string());
        }
        let path = std::path::Path::new(&self.output_dir);
        if path.exists() && !path.is_dir() {
            return Err("输出路径必须是目录，不能是普通文件".to_string());
        }
        Ok(())
    }

    pub fn validate_writable(&self) -> Result<(), String> {
        self.validate()?;
        validate_output_directory_writable(std::path::Path::new(&self.output_dir))
    }
}

pub fn validate_output_directory_writable(path: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("无法创建输出目录: {e}"))?;
    let probe = tempfile::Builder::new()
        .prefix(".larkreader-write-test-")
        .tempfile_in(path)
        .map_err(|e| format!("输出目录不可写: {e}"))?;
    probe
        .close()
        .map_err(|e| format!("输出目录写入探测清理失败: {e}"))
}

impl Default for Settings {
    fn default() -> Self {
        let output_dir = dirs::document_dir()
            .or_else(dirs::home_dir)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(std::env::temp_dir)
            .join("LarkReader")
            .to_string_lossy()
            .to_string();

        Self {
            output_dir,
            concurrency: 5,
            download_images: true,
        }
    }
}

// ============================================================================
// 登录相关
// ============================================================================

/// 非阻塞登录返回的设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// 设备码（用于后续 --device-code 轮询）
    pub device_code: String,
    /// 用户需要在浏览器中打开的验证 URL
    pub verification_url: String,
}

/// 登录结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResult {
    /// 是否成功
    pub success: bool,
    /// 用户名
    pub user_name: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

/// 飞书应用创建向导实时状态（start_app_init / get_app_init_status）
///
/// `config init --new` 是阻塞式浏览器向导：命令在后台运行并输出验证 URL，
/// 前端轮询本状态，抓到 `url` 后自动打开浏览器让用户完成创建。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppInitStatus {
    /// 是否仍在运行中
    pub running: bool,
    /// 当前阶段文案（给用户看的简短中文）
    pub stage: String,
    /// 捕获到的浏览器创建向导 URL（出现后前端应自动打开）
    pub url: Option<String>,
    /// 最近一行过程信息（原始输出的裁剪/截断）
    pub message: Option<String>,
    /// 失败原因（running=false 且 error 有值时表示失败）
    pub error: Option<String>,
}

// ============================================================================
// 运行日志
// ============================================================================

/// 日志文件元信息（list_log_files 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileMeta {
    /// 文件名，如 app-2026-09-05.log
    pub name: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 最后修改时间（RFC 3339）
    pub modified_at: Option<String>,
}

/// 日志文件内容（read_log_file 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileContent {
    /// 文件名
    pub name: String,
    /// 文本内容
    pub content: String,
    /// 文件总大小（字节）
    pub size_bytes: u64,
    /// 内容过大时是否只返回了末尾部分
    pub truncated: bool,
}

// ============================================================================
// lark-cli 响应结构（内部使用）
// ============================================================================

/// lark-cli 命令返回的顶层 JSON 结构
#[derive(Debug, Clone, Deserialize)]
pub struct LarkResponse {
    /// 是否成功
    pub ok: bool,
    /// 错误信息（ok=false 时有值，可能是字符串或嵌套对象）
    pub error: Option<serde_json::Value>,
    /// 错误码
    pub code: Option<i32>,
    /// 数据负载
    pub data: Option<serde_json::Value>,
}

/// whoami 返回结构
#[derive(Debug, Clone, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default)]
    pub identity: Option<String>,
    #[serde(rename = "tokenStatus", default)]
    pub token_status: Option<String>,
    #[serde(default)]
    pub available: Option<bool>,
    #[serde(rename = "onBehalfOf", default)]
    pub on_behalf_of: Option<OnBehalfOf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OnBehalfOf {
    #[serde(rename = "userName", default)]
    pub user_name: Option<String>,
    #[serde(rename = "openId", default)]
    pub open_id: Option<String>,
}

/// config show 返回结构
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigResponse {
    #[serde(rename = "appId", default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub brand: Option<String>,
}

/// docs +fetch 返回的文档数据
#[derive(Debug, Clone, Deserialize)]
pub struct FetchDocData {
    pub document: FetchDocument,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchDocument {
    pub content: String,
}

/// docs +media-preview 返回的数据
#[derive(Debug, Clone, Deserialize)]
pub struct MediaPreviewData {
    #[serde(rename = "saved_path", default)]
    pub saved_path: Option<String>,
}

/// wiki +node-get 返回的节点信息
#[derive(Debug, Clone, Deserialize)]
pub struct NodeGetInfo {
    #[serde(rename = "space_id", default)]
    pub space_id: Option<String>,
    #[serde(rename = "node_token", default)]
    pub node_token: Option<String>,
    #[serde(rename = "obj_token", default)]
    pub obj_token: Option<String>,
    #[serde(rename = "obj_type", default)]
    pub obj_type: Option<String>,
    #[serde(rename = "has_child", default)]
    pub has_child: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
}

/// wiki +node-list 返回的节点列表项
#[derive(Debug, Clone, Deserialize)]
pub struct NodeListItem {
    #[serde(rename = "node_token", default)]
    pub node_token: Option<String>,
    #[serde(rename = "obj_token", default)]
    pub obj_token: Option<String>,
    #[serde(rename = "obj_type", default)]
    pub obj_type: Option<String>,
    #[serde(rename = "has_child", default)]
    pub has_child: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub position: Option<i64>,
}
