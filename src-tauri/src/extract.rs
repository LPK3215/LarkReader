//! 文档提取主流程
//!
//! 直接参考 Python MVP (extract_generic.py) 的实现方式：
//! 1. fetch_doc → 获取 Markdown 正文
//! 2. extract_images → 正则提取图片引用
//! 3. for 循环逐张下载图片（串行，不用并发）
//! 4. 替换 Markdown 中的远程 URL 为本地路径
//! 5. 保存 .md 文件
//!
//! 不用 tokio，不用 spawn_blocking，简单直接。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::lark;
use crate::markdown;
use crate::models::{ExtractResult, ExtractStatus, PreviewResult, Settings};

/// 从飞书 URL 中提取 node_token
///
/// 支持格式：
/// - `https://xxx.feishu.cn/wiki/<node_token>`
/// - 纯 node_token 字符串
pub fn parse_node_token(url: &str) -> String {
    let url = url.trim();
    // 如果是纯 token（不含 /），直接返回（去掉可能的 query string）
    if !url.contains('/') {
        return url.split(['?', '#']).next().unwrap_or(url).to_string();
    }

    // 从 URL 中提取最后一段作为 node_token
    let trimmed = url.trim_end_matches('/');
    let last_segment = trimmed.rsplit('/').next().unwrap_or("");
    // 去掉 query string
    last_segment
        .split(['?', '#'])
        .next()
        .unwrap_or(last_segment)
        .to_string()
}

fn normalize_wiki_url(input: &str) -> AppResult<String> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::InvalidInput(
            "飞书链接或 token 不能为空".to_string(),
        ));
    }
    let token = parse_node_token(input);
    if token.is_empty()
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::InvalidInput(
            "无法识别有效的飞书节点 token".to_string(),
        ));
    }
    if input.contains("://") {
        let lower = input.to_ascii_lowercase();
        if !(lower.starts_with("https://")
            && (lower.contains(".feishu.cn/") || lower.contains("//feishu.cn/")))
        {
            return Err(AppError::InvalidInput("仅支持 HTTPS 飞书链接".to_string()));
        }
    }
    Ok(build_wiki_url(input))
}

/// 构造飞书 Wiki URL
///
/// 将 node_token 拼接为完整的 Wiki URL 格式
pub fn build_wiki_url(node_token: &str) -> String {
    if node_token.starts_with("http") {
        return node_token.to_string();
    }
    format!("https://feishu.cn/wiki/{}", node_token)
}

/// 从 URL 中提取文档标题（用于默认文件名）
pub fn extract_title_from_url(url: &str) -> String {
    let token = parse_node_token(url);
    // 截取前20个字符（安全截断，不切断中文字符）
    token.chars().take(20).collect()
}

/// 预览文档：获取 Markdown 正文（不下载图片）
///
/// 对应 Python: fetch_doc(node_token)
/// 先尝试获取文档真实标题，获取失败则用 URL token
pub fn preview_doc(url: &str) -> AppResult<PreviewResult> {
    let wiki_url = normalize_wiki_url(url)?;
    let node_token = parse_node_token(&wiki_url);

    // 尝试获取真实标题
    let title = match lark::wiki_node_get(&node_token) {
        Ok(info) => info
            .title
            .unwrap_or_else(|| extract_title_from_url(&wiki_url)),
        Err(_) => extract_title_from_url(&wiki_url),
    };

    // 获取文档正文
    let content = lark::docs_fetch(&wiki_url)?;

    // 提取图片引用
    let images = markdown::extract_images(&content);

    Ok(PreviewResult {
        title,
        content_markdown: content.clone(),
        images,
        char_count: content.chars().count(),
    })
}

/// 提取单篇文档：正文 + 图片下载 + 保存 .md
///
/// 对应 Python: main() 中的单文档处理逻辑
/// 串行下载，不用并发，简单可靠
pub fn extract_doc(url: &str, output_dir: &str, settings: &Settings) -> AppResult<ExtractResult> {
    extract_doc_with_title(url, None, output_dir, settings)
}

/// 提取单篇文档（可指定标题）
pub fn extract_doc_with_title(
    url: &str,
    title: Option<&str>,
    output_dir: &str,
    settings: &Settings,
) -> AppResult<ExtractResult> {
    let wiki_url = normalize_wiki_url(url)?;
    let doc_title = match title {
        Some(t) => t.to_string(),
        None => {
            // 尝试从 wiki_node_get 获取真实标题
            let node_token = parse_node_token(&wiki_url);
            match lark::wiki_node_get(&node_token) {
                Ok(info) => info
                    .title
                    .unwrap_or_else(|| extract_title_from_url(&wiki_url)),
                Err(_) => extract_title_from_url(&wiki_url),
            }
        }
    };

    // 1. 获取文档正文
    let content = lark::docs_fetch(&wiki_url)?;

    // 2. 提取图片引用
    let images = markdown::extract_images(&content);
    let image_count = images.len();

    // 3. 准备输出路径
    let safe_title = markdown::safe_filename(&doc_title);
    let filename = format!("{}.md", safe_title);
    let filepath = Path::new(output_dir).join(&filename);
    let img_dir_name = markdown::images_dir_name(&filename);
    let img_dir = Path::new(output_dir).join(&img_dir_name);

    // 确保输出目录存在
    fs::create_dir_all(output_dir)?;

    let mut errors = Vec::new();
    let mut images_downloaded = 0usize;
    let mut images_failed = 0usize;
    let mut content = content;

    // 4. 下载图片（串行，和 Python 参考代码一样）
    if settings.download_images && image_count > 0 {
        fs::create_dir_all(&img_dir)?;

        for (i, img) in images.iter().enumerate() {
            let token = &img.file_token;
            let output_base = img_dir.join(format!("img_{:02}", i + 1));
            let output_base_str = output_base.to_string_lossy().to_string();

            // 清理可能存在的旧文件（lark-cli 不支持 --overwrite，遇到已存在文件会报错）
            // 删除所有 img_XX.* 格式的文件
            if let Ok(entries) = fs::read_dir(&img_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(&format!("img_{:02}", i + 1)) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }

            // 调用 lark-cli docs +media-preview（失败时重试一次）
            let media_result = lark::docs_media_preview(token, &output_base_str);
            let media_result = if media_result.is_err() {
                // 等待 1 秒后重试
                std::thread::sleep(std::time::Duration::from_secs(1));
                // 重试前清理旧文件
                if let Ok(entries) = fs::read_dir(&img_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with(&format!("img_{:02}", i + 1)) {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
                lark::docs_media_preview(token, &output_base_str)
            } else {
                media_result
            };

            match media_result {
                Ok(saved_path) => {
                    let saved = PathBuf::from(&saved_path);
                    // 获取扩展名
                    let ext = saved
                        .extension()
                        .map(|e| format!(".{}", e.to_string_lossy()))
                        .unwrap_or_else(|| ".png".to_string());

                    // 重命名为统一的 img_XX.ext 格式
                    let final_path = img_dir.join(format!("img_{:02}{}", i + 1, ext));
                    if !saved.is_file() {
                        images_failed += 1;
                        errors.push(format!("图片 {}/{} 下载结果不存在", i + 1, image_count));
                        continue;
                    }
                    if saved != final_path {
                        if final_path.exists() {
                            fs::remove_file(&final_path)?;
                        }
                        if let Err(e) = fs::rename(&saved, &final_path) {
                            images_failed += 1;
                            errors.push(format!(
                                "图片 {}/{} 重命名失败: {}",
                                i + 1,
                                image_count,
                                e
                            ));
                            tracing::warn!(
                                "图片重命名失败 {} -> {}: {}",
                                saved.display(),
                                final_path.display(),
                                e
                            );
                            continue;
                        }
                    }

                    if !final_path.is_file() {
                        images_failed += 1;
                        errors.push(format!("图片 {}/{} 未保存到预期位置", i + 1, image_count));
                        continue;
                    }

                    // 替换 Markdown 中的远程 URL → 本地相对路径
                    let local_ref = format!("{}/img_{:02}{}", img_dir_name, i + 1, ext);
                    content = content.replace(&img.url, &local_ref);
                    images_downloaded += 1;
                }
                Err(e) => {
                    images_failed += 1;
                    errors.push(format!(
                        "图片 {}/{} 下载失败: {}",
                        i + 1,
                        image_count,
                        token
                    ));
                    tracing::warn!(
                        "图片 {}/{} 下载失败 token={}: {}",
                        i + 1,
                        image_count,
                        token,
                        e
                    );
                }
            }
        }
    }

    // 5. 保存 .md 文件
    fs::write(&filepath, content.as_bytes())?;

    // 判断提取状态
    let status = if images_failed == 0 {
        ExtractStatus::Success
    } else {
        ExtractStatus::Partial
    };

    Ok(ExtractResult {
        title: doc_title,
        filename,
        char_count: content.chars().count(),
        image_count,
        images_downloaded,
        images_failed,
        filepath: filepath.to_string_lossy().to_string(),
        status,
        errors,
    })
}

/// 兼容异步调用方：用 tokio 的 spawn_blocking 包装同步函数
///
/// 内部仍然是同步逻辑，只是包了一层让 async 代码能调用
pub async fn extract_doc_async(
    url: &str,
    output_dir: &str,
    settings: &Settings,
) -> AppResult<ExtractResult> {
    let url = url.to_string();
    let output_dir = output_dir.to_string();
    let settings = settings.clone();

    tokio::task::spawn_blocking(move || extract_doc(&url, &output_dir, &settings))
        .await
        .map_err(|e| AppError::Other(format!("任务执行失败: {}", e)))?
}

/// 兼容异步调用方：带标题版本
pub async fn extract_doc_with_title_async(
    url: &str,
    title: Option<&str>,
    output_dir: &str,
    settings: &Settings,
) -> AppResult<ExtractResult> {
    let url = url.to_string();
    let title = title.map(|s| s.to_string());
    let output_dir = output_dir.to_string();
    let settings = settings.clone();

    tokio::task::spawn_blocking(move || {
        extract_doc_with_title(&url, title.as_deref(), &output_dir, &settings)
    })
    .await
    .map_err(|e| AppError::Other(format!("任务执行失败: {}", e)))?
}
