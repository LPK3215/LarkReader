//! 统一错误类型定义
//!
//! 所有后端模块的错误都统一为 `AppError`，
//! 通过 `thiserror` 自动实现 Display 和 From trait。

use thiserror::Error;

/// 应用级统一错误类型
#[derive(Debug, Error)]
pub enum AppError {
    /// lark-cli 未安装或找不到可执行文件
    #[error("lark-cli 未找到: {0}")]
    LarkCliNotFound(String),

    /// lark-cli 命令执行失败（非零退出码）
    #[error("lark-cli 执行失败: {0}")]
    LarkCliError(String),

    /// lark-cli 返回的 JSON 解析失败
    #[error("JSON 解析失败: {0}")]
    JsonParse(String),

    /// lark-cli 返回的响应中 ok=false 或缺少预期字段
    #[error("lark-cli 返回异常: {0}")]
    LarkCliResponse(String),

    /// Node.js 未安装
    #[error("Node.js 未安装，请先安装 Node.js 18+")]
    NodeNotFound,

    /// 飞书未登录或 token 已过期
    #[error("飞书未登录或 token 已过期，请重新登录")]
    NotLoggedIn,

    /// 飞书应用未配置
    #[error("飞书应用未配置，请先执行 lark-cli config init")]
    AppNotConfigured,

    /// 文件操作错误
    #[error("文件操作失败: {0}")]
    Io(#[from] std::io::Error),

    /// 网络/HTTP 请求错误
    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),

    /// 正则表达式错误
    #[error("正则错误: {0}")]
    Regex(#[from] regex::Error),

    /// 文档提取过程中的通用错误
    #[error("{0}")]
    Extract(String),

    /// 设置项无效
    #[error("设置无效: {0}")]
    InvalidSetting(String),

    #[error("输入无效: {0}")]
    InvalidInput(String),

    #[error("应用状态不可用: {0}")]
    StateUnavailable(String),

    #[error("外部命令执行超时（{0} 秒）")]
    CommandTimeout(u64),

    /// 其他未分类的错误
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("retryable", &self.retryable())?;
        state.end()
    }
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LarkCliNotFound(_) => "LARK_CLI_NOT_FOUND",
            Self::LarkCliError(_) | Self::LarkCliResponse(_) => "LARK_CLI_ERROR",
            Self::JsonParse(_) => "INVALID_CLI_RESPONSE",
            Self::NodeNotFound => "NODE_NOT_FOUND",
            Self::NotLoggedIn => "AUTH_REQUIRED",
            Self::AppNotConfigured => "APP_NOT_CONFIGURED",
            Self::Io(_) => "IO_ERROR",
            Self::Http(_) => "NETWORK_ERROR",
            Self::Regex(_) => "REGEX_ERROR",
            Self::Extract(_) => "EXTRACT_ERROR",
            Self::InvalidSetting(_) => "INVALID_SETTING",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::StateUnavailable(_) => "STATE_UNAVAILABLE",
            Self::CommandTimeout(_) => "COMMAND_TIMEOUT",
            Self::Other(_) => "INTERNAL_ERROR",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Http(_) | Self::CommandTimeout(_) | Self::StateUnavailable(_)
        )
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

/// 便捷 Result 类型别名
pub type AppResult<T> = Result<T, AppError>;
