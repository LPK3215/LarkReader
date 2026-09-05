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

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::lark;
use crate::markdown;
use crate::models::{ExtractResult, ExtractStatus, PreviewResult, Settings};
use rayon::prelude::*;

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

/// 词法上展开路径中的 `.` / `..` 段（不做磁盘 I/O，不解析符号链接）。
///
/// 背景：Windows 上 `fs::rename` 的目标若含未展开的 `..` 段（例如
/// `D:\a\..\out\file`）会直接报 `os error 2`，而该路径经其它工具
/// （如 lark-cli）规范化后返回的绝对路径与源路径字符串不一致，导致
/// 本应跳过 rename 的场景也走进 rename 分支而失败。此函数把输出路径
/// 统一规范化，避免两套表示并存。
fn lexically_clean_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component.as_os_str());
                }
            }
            other => out.push(other),
        }
    }
    out
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
    extract_doc_with_title_controlled(url, title, output_dir, settings, None)
}

fn extract_doc_with_title_controlled(
    url: &str,
    title: Option<&str>,
    output_dir: &str,
    settings: &Settings,
    cancelled: Option<Arc<AtomicBool>>,
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
    let content = lark::docs_fetch_controlled(&wiki_url, cancelled.as_deref())?;

    // 2. 提取图片引用
    let images = markdown::extract_images(&content);
    let image_count = images.len();

    // 3. 准备输出路径
    // 先规范化输出目录（展开 `..` 段），确保所有 rename / 子进程写入基于同一
    // 套路径表示，避免 Windows 下 rename 目标含 `..` 时失败。
    fs::create_dir_all(output_dir)?;
    let cleaned_output_dir = lexically_clean_path(Path::new(output_dir))
        .to_string_lossy()
        .into_owned();
    let output_dir = cleaned_output_dir.as_str();
    let safe_title = markdown::safe_filename(&doc_title);
    let filename = unique_markdown_filename(Path::new(output_dir), &safe_title);
    let filepath = Path::new(output_dir).join(&filename);
    let img_dir_name = markdown::images_dir_name(&filename);

    let temp_dir = tempfile::Builder::new()
        .prefix(".larkreader-")
        .tempdir_in(output_dir)?;
    let temp_filepath = temp_dir.path().join(&filename);
    let img_dir = temp_dir.path().join(&img_dir_name);

    let mut errors = Vec::new();
    let mut images_downloaded = 0usize;
    let mut images_failed = 0usize;
    let mut content = content;
    let mut url_map = Vec::new();

    // 4. 下载图片（按 Settings.concurrency 并发下载，失败计数）
    if settings.download_images && image_count > 0 {
        fs::create_dir_all(&img_dir)?;
        let mut processed_urls = HashSet::new();
        let unique_images: Vec<_> = images
            .iter()
            .enumerate()
            .filter(|(_, image)| processed_urls.insert(image.url.clone()))
            .map(|(index, image)| (index, image.clone()))
            .collect();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(settings.concurrency.min(unique_images.len()).max(1))
            .build()
            .map_err(|error| AppError::Other(format!("创建图片下载线程池失败: {}", error)))?;
        let download_results: Vec<_> = pool.install(|| {
            unique_images
                .par_iter()
                .map(|(i, img)| {
                    if cancelled
                        .as_ref()
                        .is_some_and(|flag| flag.load(Ordering::Relaxed))
                    {
                        return (
                            *i,
                            img.clone(),
                            Err(AppError::Extract("任务已取消".to_string())),
                        );
                    }
                    let output_base = img_dir.join(format!("img_{:02}", i + 1));
                    let output_base_str = output_base.to_string_lossy().to_string();
                    let first = lark::docs_media_preview_controlled(
                        &img.file_token,
                        &output_base_str,
                        cancelled.as_deref(),
                    );
                    let result = if first.is_err() {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        lark::docs_media_preview_controlled(
                            &img.file_token,
                            &output_base_str,
                            cancelled.as_deref(),
                        )
                    } else {
                        first
                    };
                    (*i, img.clone(), result)
                })
                .collect()
        });

        for (i, img, media_result) in download_results {
            let token = &img.file_token;
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
                            if let Err(e) = fs::remove_file(&final_path) {
                                images_failed += 1;
                                errors.push(format!(
                                    "图片 {}/{} 无法替换旧文件: {}",
                                    i + 1,
                                    image_count,
                                    e
                                ));
                                continue;
                            }
                        }
                        if let Err(e) = fs::rename(&saved, &final_path) {
                            images_failed += 1;
                            errors.push(format!(
                                "图片 {}/{} 重命名失败 [{} -> {}]: {}",
                                i + 1,
                                image_count,
                                saved.display(),
                                final_path.display(),
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
                    url_map.push((img.url.clone(), local_ref));
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
        content = markdown::replace_image_urls(&content, &url_map);
    }

    // 5. 保存 .md 文件
    fs::write(&temp_filepath, content.as_bytes())?;
    let final_img_dir = Path::new(output_dir).join(&img_dir_name);
    if img_dir.exists() {
        fs::rename(&img_dir, &final_img_dir)?;
    }
    if let Err(error) = fs::rename(&temp_filepath, &filepath) {
        if final_img_dir.exists() {
            let _ = fs::remove_dir_all(&final_img_dir);
        }
        return Err(error.into());
    }

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

fn unique_markdown_filename(output_dir: &Path, title: &str) -> String {
    let filename = format!("{}.md", title);
    if !output_dir.join(&filename).exists()
        && !output_dir
            .join(markdown::images_dir_name(&filename))
            .exists()
    {
        return filename;
    }
    for suffix in 2..=10_000 {
        let filename = format!("{} ({}).md", title, suffix);
        if !output_dir.join(&filename).exists()
            && !output_dir
                .join(markdown::images_dir_name(&filename))
                .exists()
        {
            return filename;
        }
    }
    format!("{}_{}.md", title, std::process::id())
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
    extract_doc_with_title_async_controlled(url, title, output_dir, settings, None).await
}

pub async fn extract_doc_with_title_async_controlled(
    url: &str,
    title: Option<&str>,
    output_dir: &str,
    settings: &Settings,
    cancelled: Option<Arc<AtomicBool>>,
) -> AppResult<ExtractResult> {
    let url = url.to_string();
    let title = title.map(|s| s.to_string());
    let output_dir = output_dir.to_string();
    let settings = settings.clone();

    tokio::task::spawn_blocking(move || {
        extract_doc_with_title_controlled(&url, title.as_deref(), &output_dir, &settings, cancelled)
    })
    .await
    .map_err(|e| AppError::Other(format!("任务执行失败: {}", e)))?
}
