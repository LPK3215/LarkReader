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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialExport {
    pub title: String,
    pub node_token: String,
    pub obj_type: WikiNodeType,
    pub paths: Vec<String>,
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
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Progress {
    pub fn new(task_id: String, total: usize) -> Self {
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
        }
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
}

impl Default for Settings {
    fn default() -> Self {
        let output_dir = dirs::document_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
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
    #[serde(default)]
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
