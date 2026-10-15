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

    // 先取正文：标题要按"知识库节点 → 正文 `<title>` → token"的顺序兜底，
    // `docx` 直链没有知识库节点，只能靠正文里那行真实标题。
    let content = lark::docs_fetch(&wiki_url)?;
    let title = resolve_document_title(&wiki_url, &content);

    // 提取图片引用
    let images = markdown::extract_images(&content);

    Ok(PreviewResult {
        title,
        content_markdown: content.clone(),
        images,
        char_count: content.chars().count(),
    })
}

/// 解析文档标题：知识库节点标题 → 正文首行 `<title>` → URL token 兜底
///
/// `extract_title_from_url` 只能给出"token 前 20 个字符"，对 docx 直链来说
/// 文件名就成了看不出内容的一串乱码，所以正文里的真实标题必须顶上。
fn resolve_document_title(wiki_url: &str, content: &str) -> String {
    let node_token = parse_node_token(wiki_url);
    if let Ok(info) = lark::wiki_node_get(&node_token) {
        if let Some(title) = info.title {
            if !title.trim().is_empty() {
                return title;
            }
        }
    }
    if let Some(title) = markdown::extract_document_title(content) {
        return title;
    }
    extract_title_from_url(wiki_url)
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

    // 1. 获取文档正文（顺带取回 `reference_map`：内嵌 HTML 内容块的正文只挂在那里）
    let mut docs = lark::docs_fetch_with_refs_controlled(&wiki_url, cancelled.as_deref())?;

    // 标题优先级：调用方指定 → 知识库节点标题 → 正文首行 `<title>` → token 兜底
    //
    // 为什么加 `<title>` 这一档：docx 直链（`/docx/xxx`）不是知识库节点，
    // `wiki_node_get` 拿不到标题，之前就退化成"token 前 20 个字符"这种看不出
    // 是什么的目录名；正文首行其实带着真实标题。
    let doc_title = match title {
        Some(t) => t.to_string(),
        None => resolve_document_title(&wiki_url, &docs.content),
    };

    // 正文取出来后就不再需要 `docs.content`，挪走 ownership，
    // 同时保留 `docs` 供内嵌 HTML 内容块按 `data-ref` 查询。
    let mut content = std::mem::take(&mut docs.content);

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
                            crate::logger::warn(format!(
                                "图片重命名失败 {} -> {}: {}",
                                saved.display(),
                                final_path.display(),
                                e
                            ));
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
                    crate::logger::warn(format!(
                        "图片 {}/{} 下载失败 token={}: {}",
                        i + 1,
                        image_count,
                        token,
                        e
                    ));
                }
            }
        }
        content = markdown::replace_image_urls(&content, &url_map);
    }

    // 4.5 全文检索内嵌富对象（表格 / 多维表格 / 画板 / 标签形态的图片）
    //
    // 为什么必须单独做：`docs +fetch --doc-format markdown` 对内嵌电子表格这类
    // 富对象不做内容转换，正文里只留一个引用片段，不处理的话导出结果里那张表
    // 就是空的（2026-10 用户反馈）。
    //
    // 原则是"先全文扫完再分派"：不按文档类型提前决定处理什么，而是把正文里所有
    // 能识别的东西一次扫出来，匹配到就处理、匹配不到就只留可见提示，绝不出现
    // "这篇文档有表格所以走了表格分支、结果图片没下"这种事。
    let tables_dir_name = markdown::tables_dir_name(&filename);
    let tables_dir = temp_dir.path().join(&tables_dir_name);
    let embedded_ctx = EmbeddedContext {
        wiki_url: &wiki_url,
        tables_dir: &tables_dir,
        tables_dir_name: &tables_dir_name,
        img_dir: &img_dir,
        img_dir_name: &img_dir_name,
        download_images: settings.download_images,
        mode: if settings.export_pure_data {
            lark::SheetExportMode::PureData
        } else {
            lark::SheetExportMode::Official
        },
        cancelled: cancelled.as_deref(),
        fetch: Some(&docs),
    };
    // 先把"数据全在属性里、无需联网"的标签（bookmark / button / cite）落成 Markdown，
    // 免得导出的正文里留一串 XML；还原不了的（callout / grid / source / …）仍保持
    // 原文，由 `extract_embedded_objects` 末尾的统一提示兜底。
    content = markdown::restore_inline_server_tags(&content);

    let embedded = extract_embedded_objects(&content, &embedded_ctx);
    let embedded_failed = embedded.failed;
    content = embedded.content;
    errors.extend(embedded.errors);

    // 4.6 深层标题（h7~h9）层级修正
    //
    // 服务端把 `<h7>`/`<h8>`/`<h9>` 一律渲染成 `######`，三个不同层级塌成一级。
    // 但这属于"少数文档才有"的情况，只有真的出现 6 级标题时才值得多花一次
    // `docs +fetch --doc-format xml`；绝大多数文档不会触发。
    if markdown::has_six_level_heading(&content) {
        match lark::docs_fetch_xml_controlled(&wiki_url, cancelled.as_deref()) {
            Ok(xml) => {
                let fixed = markdown::fix_deep_headings(&content, &xml);
                if fixed != content {
                    crate::logger::info("已按 XML 层级修正深层标题（h7~h9）".to_string());
                    content = fixed;
                }
            }
            Err(error) => {
                crate::logger::warn(format!("深层标题层级修正失败（保持 6 级标题）：{error}"));
                errors.push(format!(
                    "深层标题层级修正失败，h7~h9 仍按 6 级标题保留：{error}"
                ));
            }
        }
    }

    // 5. 保存 .md 文件
    fs::write(&temp_filepath, content.as_bytes())?;
    let final_img_dir = Path::new(output_dir).join(&img_dir_name);
    if img_dir.exists() {
        fs::rename(&img_dir, &final_img_dir)?;
    }
    let final_tables_dir = Path::new(output_dir).join(&tables_dir_name);
    if tables_dir.exists() {
        fs::rename(&tables_dir, &final_tables_dir)?;
    }
    if let Err(error) = fs::rename(&temp_filepath, &filepath) {
        if final_img_dir.exists() {
            let _ = fs::remove_dir_all(&final_img_dir);
        }
        if final_tables_dir.exists() {
            let _ = fs::remove_dir_all(&final_tables_dir);
        }
        return Err(error.into());
    }

    // 判断提取状态：内嵌表格没导出来，正文就是缺内容的，同样算"部分成功"
    let status = if images_failed == 0 && embedded_failed == 0 {
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
    // 与文档同名的 `_images` / `_tables` 资源目录同样算"已占用"：否则同名文档
    // 第二次导出时会把这些资源目录当成自己新建的，导致图片/内嵌表格互相覆盖。
    let taken = |filename: &str| {
        output_dir.join(filename).exists()
            || output_dir.join(markdown::images_dir_name(filename)).exists()
            || output_dir.join(markdown::tables_dir_name(filename)).exists()
    };
    let filename = format!("{}.md", title);
    if !taken(&filename) {
        return filename;
    }
    for suffix in 2..=10_000 {
        let filename = format!("{} ({}).md", title, suffix);
        if !taken(&filename) {
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

// ============================================================================
// 文档内嵌表格（电子表格 / 多维表格 / 画板）
// ============================================================================
//
// 背景（2026-10 用户反馈）：在**文档里**插入的表格，导出后是空的。
//
// 根因是文档与独立表格走的是两条完全不同的路径：
// - 独立表格页面是 wiki 节点（`obj_type = sheet`），走 `sheets +workbook-export`
//   / 读单元格降级，数据完整；
// - 文档正文由 `docs +fetch --doc-format markdown` 一次性转成 Markdown，其中
//   **内嵌电子表格这类富对象不做内容转换**，正文里只剩一个引用片段。文档内部的
//   块不是 wiki 节点，所以永远不会被上面那套表格导出逻辑碰到。
//
// 这里补的就是这一环：把正文里的引用片段认出来，用引用里的 token 把表读回来，
// 既内联进正文（解决"看起来是空的"），也整份导出成 xlsx（保证数据完整）。

/// 正文里最多内联多少行表格数据（含表头）
///
/// 内嵌表格动辄几千行，整张塞进 Markdown 既没人会读、也会把 `.md` 撑成几十兆。
/// 完整数据始终在旁边的 xlsx 里，正文只做可读预览。
const MAX_INLINE_TABLE_ROWS: usize = 200;

/// 内嵌表格处理结果
struct EmbeddedTables {
    /// 替换过引用片段之后的正文
    content: String,
    /// 发现的富对象引用数
    found: usize,
    /// 成功产出文件（xlsx）的个数
    exported: usize,
    /// 失败个数（会让整篇文档变成"部分成功"）
    failed: usize,
    /// 面向用户的错误说明
    errors: Vec<String>,
}

/// 单个引用片段的处理结果
struct RefOutcome {
    /// 用于替换原文的文本；`None` 表示保持原文不动
    replacement: Option<String>,
    /// 是否产出了文件
    exported: bool,
    /// 是否算失败
    failed: bool,
}

/// 内嵌对象抽取所需的上下文（避免一长串参数）
struct EmbeddedContext<'a> {
    wiki_url: &'a str,
    /// 内嵌表格产物目录（`{文档名}_tables`，落在临时目录里，最后统一提交）
    tables_dir: &'a Path,
    tables_dir_name: &'a str,
    /// 图片目录（与 `_images` 同一个，标签形态的图片也落到这里）
    img_dir: &'a Path,
    img_dir_name: &'a str,
    download_images: bool,
    mode: lark::SheetExportMode,
    cancelled: Option<&'a AtomicBool>,
    /// `docs +fetch` 的完整结果（正文 + `reference_map`）
    ///
    /// 内嵌 HTML 内容块的正文不在文档里，只能靠 `data-ref` 从这里取；
    /// 没有它这块内容就是整段丢失。
    fetch: Option<&'a lark::DocsFetch>,
}

/// 从文档链接取出 `{scheme}://{host}`
///
/// 内嵌电子表格在正文里只留 token，必须自己补出完整的表链接，才能让 lark-cli
/// 走与独立表格页面完全相同的 `--url` 参数。
fn feishu_origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

/// 同一 token 多次出现时用于去重的键（没有 token 的形态不参与去重）
///
/// 电子表格必须把 `sheet-id` 也算进键：同一工作簿的不同子表是两个**独立的块**，
/// 只用 `token` 去重会把第二个块误判成"同一个对象的重复引用"而整块跳过，
/// 结果就是正文里只剩第一块的表、两个块的内容还都指向第一块。
fn rich_ref_key(reference: &markdown::RichRef) -> Option<String> {
    let kind = match reference.kind {
        markdown::RichRefKind::Table | markdown::RichRefKind::Unknown => return None,
        markdown::RichRefKind::Sheet => "sheet",
        markdown::RichRefKind::Bitable => "bitable",
        markdown::RichRefKind::Whiteboard => "whiteboard",
        markdown::RichRefKind::Image => "image",
        // 内嵌 HTML 块没有 token，去重键用 `data-ref`
        markdown::RichRefKind::Html5 => {
            return reference
                .data_ref
                .as_ref()
                .map(|data_ref| format!("html5:{data_ref}"));
        }
    };
    let token = reference.token.as_ref()?;
    match reference.sheet_id.as_deref() {
        Some(sheet_id) if !sheet_id.is_empty() => Some(format!("{kind}:{token}:{sheet_id}")),
        _ => Some(format!("{kind}:{token}")),
    }
}

/// 全文检出正文里的内嵌对象并逐个处理
///
/// 匹配到就处理（电子表格 / 多维表格 / 图片标签形态），匹配不到的块级标签留在
/// 原文里、在文末给一段可见提示并写日志——**不静默丢弃、也不假装成功**。
fn extract_embedded_objects(content: &str, ctx: &EmbeddedContext<'_>) -> EmbeddedTables {
    let references = markdown::find_rich_refs(content);
    let mut result = EmbeddedTables {
        content: content.to_string(),
        found: references.len(),
        exported: 0,
        failed: 0,
        errors: Vec::new(),
    };
    if references.is_empty() {
        report_leftover_tags(&mut result);
        return result;
    }

    let origin = feishu_origin(ctx.wiki_url);
    let mut seen: HashSet<String> = HashSet::new();
    let mut rendered: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    // 工作簿 token → 已落盘的 xlsx 序号：同一工作簿被多个块引用时只导出一份文件
    let mut sheet_file_seq: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut plan: Vec<(usize, usize, String)> = Vec::new();
    let mut table_seq = 0usize;
    let mut image_seq = 0usize;
    let mut failed = 0usize;

    for reference in &references {
        // 未识别的块级标签：保持原文（可能还是有效内容），不做替换；
        // 它会被文末的残留标签扫描统一登记并提示。
        if reference.kind == markdown::RichRefKind::Unknown {
            continue;
        }

        let key = rich_ref_key(reference);

        // 同一个 token 出现多次（引用片段 + IM 链接形态）只处理一次，复用替换文本
        if let Some(key) = &key {
            if seen.contains(key) {
                if let Some(Some(text)) = rendered.get(key) {
                    plan.push((reference.start, reference.end, text.clone()));
                }
                continue;
            }
            seen.insert(key.clone());
        }

        // 产物编号 + "是否新落一份文件"
        //
        // 电子表格按**工作簿 token** 计一份 xlsx：同一工作簿的两个子表被分别嵌入时，
        // 两个块各自渲染自己引用的子表，但"完整数据"是同一个工作簿，所以只落一份文件、
        // 两个块指向同一份，避免产出两份内容完全相同的 xlsx。
        let mut index = 0usize;
        let mut produce_file = true;
        match reference.kind {
            markdown::RichRefKind::Sheet => match reference.token.as_deref() {
                Some(token) => {
                    let token_key = format!("sheet:{token}");
                    match sheet_file_seq.get(&token_key) {
                        Some(existing) => {
                            index = *existing;
                            produce_file = false;
                        }
                        None => {
                            table_seq += 1;
                            index = table_seq;
                            sheet_file_seq.insert(token_key, index);
                        }
                    }
                }
                None => {
                    table_seq += 1;
                    index = table_seq;
                }
            },
            markdown::RichRefKind::Bitable => {
                table_seq += 1;
                index = table_seq;
            }
            markdown::RichRefKind::Image => {
                image_seq += 1;
                index = image_seq;
            }
            _ => {}
        }

        let mut errors: Vec<String> = Vec::new();
        let outcome = render_embedded_ref(
            reference,
            ctx,
            origin.as_deref(),
            index,
            produce_file,
            &mut errors,
        );
        result.errors.extend(errors);
        if outcome.failed {
            failed += 1;
        }
        if outcome.exported {
            result.exported += 1;
        }
        if let Some(key) = &key {
            rendered.insert(key.clone(), outcome.replacement.clone());
        }
        if let Some(text) = outcome.replacement {
            plan.push((reference.start, reference.end, text));
        }
    }
    result.failed = failed;

    // 从后往前替换：前面的片段偏移不会被已经改过的长度影响
    for (start, end, text) in plan.into_iter().rev() {
        result.content.replace_range(start..end, &text);
    }

    // 文末提示：正文里还留着"按原文保留、没被还原"的标签就明确告知
    //
    // 为什么放在替换之后再统一扫一遍：这些标签既有"整行只有一个"的
    // （`<bookmark …></bookmark>`），也有与文字同行的（`<button …>文字</button>`），
    // 只靠 `find_rich_refs` 里的整行匹配会漏（整行匹配要求整行就是**一个**标签，
    // 而 `<bookmark …></bookmark>` 一行里其实有两个）。扫最终正文能把两类一起兜住，
    // 也顺带覆盖"表格结构解析失败、原样保留"的情况。围栏代码块里的示例文本会被跳过。
    report_leftover_tags(&mut result);
    result
}

/// 扫正文里的残留标签：写日志 + 在文末追加一段可见提示
///
/// 单独成函数是因为"没有任何富对象引用"的文档也要做这件事——那类文档（例如
/// 只有书签/按钮的链接型文档）会提前返回，之前就漏掉了提示。
fn report_leftover_tags(result: &mut EmbeddedTables) {
    let leftovers = markdown::unresolved_tags(&result.content);
    if !leftovers.is_empty() {
        crate::logger::warn(format!(
            "文档里有 {} 种标签未能离线还原：{}（已保持原文，未做转换）",
            leftovers.len(),
            leftovers.join(", ")
        ));
        result.content.push_str(&format!(
            "\n\n---\n\n> 提示：本文档里还有 {} 种内容标记没能离线还原（标签：{}），\
             已按原样保留在正文中，显示效果可能与飞书里不同；需要完整效果请在飞书里查看。\n",
            leftovers.len(),
            leftovers.join("、")
        ));
    }
    crate::logger::info(format!(
        "文档内嵌对象：发现 {} 处，导出 {} 个，失败 {} 个，残留标签 {} 种",
        result.found,
        result.exported,
        result.failed,
        leftovers.len()
    ));
}

/// 处理单个内嵌引用
///
/// `index`：该引用对应的产物序号（电子表格 / 多维表格 / 图片各自成序）。
/// `produce_file`：是否还要落一份新文件；同一工作簿的第二个引用会传 `false`
/// （文件已由第一个引用产出，复用同一份，避免重复落盘）。
fn render_embedded_ref(
    reference: &markdown::RichRef,
    ctx: &EmbeddedContext<'_>,
    origin: Option<&str>,
    index: usize,
    produce_file: bool,
    errors: &mut Vec<String>,
) -> RefOutcome {
    match reference.kind {
        markdown::RichRefKind::Sheet => {
            let label = match reference.sheet_id.as_deref() {
                Some(sheet_id) => format!("内嵌电子表格（子表 {sheet_id}）"),
                None => "内嵌电子表格".to_string(),
            };
            let Some(token) = reference.token.as_deref() else {
                let message = format!("{label}缺少 token，无法导出");
                errors.push(message.clone());
                crate::logger::warn(format!("文档内嵌表格：{message}"));
                return RefOutcome {
                    replacement: Some(format!("> {message}")),
                    exported: false,
                    failed: true,
                };
            };
            render_embedded_sheet(
                &label,
                token,
                reference.sheet_id.as_deref(),
                ctx,
                origin,
                index,
                produce_file,
                errors,
            )
        }
        markdown::RichRefKind::Bitable => {
            let Some(token) = reference.token.as_deref() else {
                let message = "内嵌多维表格缺少 token，无法导出".to_string();
                errors.push(message.clone());
                crate::logger::warn(format!("文档内嵌多维表格：{message}"));
                return RefOutcome {
                    replacement: Some(format!("> {message}")),
                    exported: false,
                    failed: true,
                };
            };
            render_embedded_bitable("内嵌多维表格", token, ctx, index, errors)
        }
        markdown::RichRefKind::Image => {
            let Some(token) = reference.token.as_deref() else {
                let note = "> 内嵌图片缺少文件 token，无法下载（已保持原文）".to_string();
                crate::logger::warn("文档内嵌图片：标签里没有可用 token，已保持原文");
                return RefOutcome {
                    replacement: Some(note),
                    exported: false,
                    failed: false,
                };
            };
            render_embedded_image(token, ctx, index, errors)
        }
        markdown::RichRefKind::Whiteboard => {
            let note = format!(
                "> 内嵌画板（token：{}）无法离线还原内容，请在飞书里查看",
                reference.token.as_deref().unwrap_or("未知")
            );
            crate::logger::warn("文档内嵌画板：正文里只留引用片段，已改写为提示");
            RefOutcome {
                replacement: Some(note),
                exported: false,
                failed: false,
            }
        }
        // 原生表格片段：整块转成 Markdown 管道表。
        //
        // 之前这里是"保持原文"，结果 `.md` 里留着一整段 HTML：列宽、底色丢了，
        // 单元格里的公式还退回 `<latex>` 字面标签，读者以为没转成功。现在按行/列
        // 还原成管道表，单元格里的行内标签也转成 Markdown；表里的图片顺便下载落盘
        // （这类图片藏在表格 HTML 里，`extract_images` 只认 `![]()`，扫不到它）。
        markdown::RichRefKind::Table => {
            let mut cell_errors: Vec<String> = Vec::new();
            let mut cell_failed = false;
            let converted = markdown::html_table_to_markdown(&reference.raw, |src, caption| {
                match table_cell_image_markdown(src, caption, ctx) {
                    Ok(text) => text,
                    Err(message) => {
                        cell_failed = true;
                        cell_errors.push(message);
                        "[图片下载失败]".to_string()
                    }
                }
            });
            errors.extend(cell_errors);
            match converted {
                Some(table) => RefOutcome {
                    replacement: Some(table),
                    exported: false,
                    failed: cell_failed,
                },
                None => {
                    crate::logger::warn(format!(
                        "文档里有解析不出结构的 <table> 片段（{} 字节），已保持原文",
                        reference.raw.len()
                    ));
                    RefOutcome {
                        replacement: None,
                        exported: false,
                        failed: false,
                    }
                }
            }
        }
        // 未识别的块级标签在调用处就已经分流（保持原文 + 文末提示），不会走到这里
        markdown::RichRefKind::Unknown => RefOutcome {
            replacement: None,
            exported: false,
            failed: false,
        },
        // 内嵌 HTML 内容块：正文不在文档里，必须靠 `data-ref` 去
        // `docs +fetch` 的 `reference_map["html5-block"]` 取回。
        //
        // 之前这里整个块的内容是丢的（正文里只有一个 `<html5-block …/>` 空标签），
        // 因为它长得像富对象引用、但其实任何 token 都没有。现在取回后原样包进
        // 围栏代码块写下：内容是任意 HTML，直接摊在 Markdown 里会污染版式。
        markdown::RichRefKind::Html5 => {
            let Some(data_ref) = reference.data_ref.as_deref() else {
                let message = "内嵌 HTML 内容块缺少 data-ref，内容无法取出".to_string();
                errors.push(message.clone());
                crate::logger::warn(format!("文档内嵌 HTML 块：{message}"));
                return RefOutcome {
                    replacement: Some(format!("> {message}")),
                    exported: false,
                    failed: true,
                };
            };
            match ctx.fetch.and_then(|fetch| fetch.html5_block(data_ref)) {
                Some(html) => {
                    crate::logger::info(format!(
                        "文档内嵌 HTML 块：已还原 {} 字节（data-ref={data_ref}）",
                        html.len()
                    ));
                    RefOutcome {
                        replacement: Some(markdown::wrap_fence("html", html)),
                        exported: false,
                        failed: false,
                    }
                }
                None => {
                    let message =
                        format!("内嵌 HTML 内容块在引用表里取不到（data-ref={data_ref}）");
                    errors.push(message.clone());
                    crate::logger::warn(format!("文档内嵌 HTML 块：{message}"));
                    RefOutcome {
                        replacement: Some(format!("> {message}，内容未能还原")),
                        exported: false,
                        failed: true,
                    }
                }
            }
        }
    }
}

/// 表格单元格里的 `<img src="token"/>`：下载到 `_images`，改写成 Markdown 图片
///
/// 这类图片藏在表格 HTML 里，`extract_images`（只认 `![]()` 形态）扫不到它：
/// 不处理就是"图还在表里，但没落盘、正文只剩一个裸 token"。
///
/// 文件名直接用文件 token 命名：同一张图被多处引用时天然复用一个文件，
/// 不需要额外维护计数器。
fn table_cell_image_markdown(
    src: &str,
    caption: &str,
    ctx: &EmbeddedContext<'_>,
) -> Result<String, String> {
    let token = markdown::extract_file_token(src);
    let alt = table_image_alt(caption);
    if !ctx.download_images || token.is_empty() {
        // 不下载时保留原始引用：源码里仍看得出这是哪张图
        return Ok(format!("![{alt}]({src})"));
    }

    fs::create_dir_all(ctx.img_dir).map_err(|error| format!("创建图片目录失败：{error}"))?;

    let stem = format!("img_cell_{}", markdown::safe_filename(&token));
    let output_base = ctx.img_dir.join(&stem);
    let saved = lark::docs_media_preview_controlled(
        &token,
        &output_base.to_string_lossy(),
        ctx.cancelled,
    )
    .map_err(|error| format!("表格内图片 {token} 下载失败：{error}"))?;

    let saved = PathBuf::from(&saved);
    if !saved.is_file() {
        return Err(format!("表格内图片 {token} 下载结果不存在"));
    }
    let ext = saved
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_else(|| ".png".to_string());
    let file_name = format!("{stem}{ext}");
    let final_path = ctx.img_dir.join(&file_name);
    if saved != final_path {
        if final_path.exists() {
            let _ = fs::remove_file(&final_path);
        }
        fs::rename(&saved, &final_path)
            .map_err(|error| format!("表格内图片 {token} 重命名失败：{error}"))?;
    }

    Ok(format!("![{alt}]({}/{})", ctx.img_dir_name, file_name))
}

/// 图片 alt 文案：折叠空白、转义会破坏 `![…]` 结构的方括号
fn table_image_alt(caption: &str) -> String {
    let cleaned: String = caption.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return "图片".to_string();
    }
    cleaned.replace(']', "\\]")
}

/// 处理一个标签形态的内嵌图片：下载到 `_images` 并改写为 Markdown 图片
///
/// 为什么要有它：`extract_images` 只认 Markdown 的 `![]()` 形态。如果正文里那
/// 张图是 `<image token="…"/>` 这种标签形态，它一个都不会被下载——图片"没匹配
/// 到就下不下来"正是这个原因。
fn render_embedded_image(
    token: &str,
    ctx: &EmbeddedContext<'_>,
    index: usize,
    errors: &mut Vec<String>,
) -> RefOutcome {
    if !ctx.download_images {
        return RefOutcome {
            replacement: Some("> 内嵌图片（设置里已关闭图片下载）".to_string()),
            exported: false,
            failed: false,
        };
    }

    if let Err(error) = fs::create_dir_all(ctx.img_dir) {
        let message = format!("创建图片目录失败：{error}");
        errors.push(message.clone());
        return RefOutcome {
            replacement: Some(format!("> {message}")),
            exported: false,
            failed: true,
        };
    }

    let stem = format!("img_tag_{:02}", index.max(1));
    let output_base = ctx.img_dir.join(&stem);
    let output_base_str = output_base.to_string_lossy().to_string();

    let fail = |message: String, errors: &mut Vec<String>| {
        crate::logger::warn(format!("文档内嵌图片：{message}"));
        errors.push(message.clone());
        RefOutcome {
            replacement: Some(format!("> {message}")),
            exported: false,
            failed: true,
        }
    };

    let saved = match lark::docs_media_preview_controlled(token, &output_base_str, ctx.cancelled) {
        Ok(saved) => saved,
        Err(error) => return fail(format!("内嵌图片 {token} 下载失败：{error}"), errors),
    };
    let saved = PathBuf::from(&saved);
    if !saved.is_file() {
        return fail(format!("内嵌图片 {token} 下载结果不存在"), errors);
    }

    let ext = saved
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_else(|| ".png".to_string());
    let file_name = format!("{stem}{ext}");
    let final_path = ctx.img_dir.join(&file_name);
    if saved != final_path {
        if final_path.exists() {
            if let Err(error) = fs::remove_file(&final_path) {
                return fail(
                    format!("内嵌图片 {token} 无法替换旧文件：{error}"),
                    errors,
                );
            }
        }
        if let Err(error) = fs::rename(&saved, &final_path) {
            return fail(format!("内嵌图片 {token} 重命名失败：{error}"), errors);
        }
    }

    RefOutcome {
        replacement: Some(format!("![内嵌图片]({}/{})", ctx.img_dir_name, file_name)),
        exported: true,
        failed: false,
    }
}

/// 拼装替换文本：首行标明来源与完整性，随后是内容/链接/失败原因
fn compose_embedded_note(
    label: &str,
    token: &str,
    failed: bool,
    exported: bool,
    parts: &[String],
) -> RefOutcome {
    let mut out = if failed {
        format!("> {label}（token：{token}）导出不完整\n")
    } else {
        format!("> {label}（token：{token}）\n")
    };
    for part in parts {
        out.push('\n');
        out.push_str(part);
        out.push('\n');
    }
    RefOutcome {
        replacement: Some(out.trim_end().to_string()),
        exported,
        failed,
    }
}

/// 处理一个内嵌电子表格：内联 Markdown 表格 + 整份导出 xlsx
///
/// `sheet_id`：该块真正引用的子表（引用里带的 `sheet-id`）。只渲染这一张子表，
/// 避免"文档里嵌了两个子表，结果正文里每块都把整份工作簿渲染一遍"这种错位。
/// 引用没带 `sheet-id`、或带的 id 在表里找不到时，退回渲染全部子表——宁可多给，
/// 也不能少给。
fn render_embedded_sheet(
    label: &str,
    token: &str,
    sheet_id: Option<&str>,
    ctx: &EmbeddedContext<'_>,
    origin: Option<&str>,
    index: usize,
    produce_file: bool,
    errors: &mut Vec<String>,
) -> RefOutcome {
    let Some(origin) = origin else {
        let message = format!("{label}（{token}）未能导出：无法从文档链接推断飞书域名");
        errors.push(message.clone());
        return compose_embedded_note(label, token, true, false, &[message]);
    };
    let url = format!("{origin}/sheets/{token}");

    let mut parts: Vec<String> = Vec::new();
    let mut failed = false;
    let mut exported = false;

    // ① 把数据读回来内联进正文 —— 这一步才真正解决"表格是空的"
    match lark::sheets_read_all_csv(&url) {
        Ok((_title, sheets)) => {
            let selected: Vec<&lark::SheetCsv> = match sheet_id.filter(|id| !id.is_empty()) {
                Some(id) => {
                    let matched: Vec<&lark::SheetCsv> =
                        sheets.iter().filter(|sheet| sheet.sheet_id == id).collect();
                    if matched.is_empty() {
                        crate::logger::warn(format!(
                            "{label}（{token}）里找不到 sheet-id={id} 的子表，改为渲染全部子表"
                        ));
                        sheets.iter().collect()
                    } else {
                        matched
                    }
                }
                None => sheets.iter().collect(),
            };
            // 指定了具体子表、或确实是多子表工作簿时，把子表名字打出来，
            // 让"这一块对应的到底哪张表"一目了然。
            let show_name = selected.len() > 1 || sheet_id.is_some();
            let mut inline = String::new();
            for sheet in &selected {
                let table = markdown::csv_to_markdown_table_marked(
                    &sheet.csv,
                    MAX_INLINE_TABLE_ROWS,
                    &sheet.image_cells,
                );
                if table.rows_used == 0 {
                    continue;
                }
                if show_name {
                    inline.push_str(&format!(
                        "**{}**（{} 行 × {} 列）\n\n",
                        sheet.name, table.rows_total, table.columns
                    ));
                }
                inline.push_str(&table.markdown);
                if table.rows_used < table.rows_total {
                    inline.push_str(&format!(
                        "（该子表共 {} 行，此处只内联前 {} 行，完整数据见下方 xlsx）",
                        table.rows_total, table.rows_used
                    ));
                }
                // 图片格在 CSV 里就是空串：不说明的话，正文那格看着像"原表本来就是空的"
                if !sheet.image_cells.is_empty() {
                    inline.push_str(&format!(
                        "（该子表含 {} 处图片，Markdown 不能内联图片，已在对应格子标出 [图片]{}；\
                         完整内容见下方 xlsx）",
                        sheet.image_cells.len(),
                        if sheet.image_cells_complete {
                            ""
                        } else {
                            "，位置未读全"
                        }
                    ));
                }
                inline.push('\n');
            }
            if inline.trim().is_empty() {
                failed = true;
                let message = format!("{label}（{token}）读回来是空表，可能原表本身没有数据");
                crate::logger::warn(format!("文档内嵌表格：{message}"));
                errors.push(message.clone());
                parts.push(message);
            } else {
                parts.push(inline.trim_end().to_string());
            }
        }
        Err(error) => {
            failed = true;
            let message = format!("{label}（{token}）读取失败：{error}");
            crate::logger::warn(format!("文档内嵌表格：{message}"));
            errors.push(message.clone());
            parts.push(message);
        }
    }

    // ② 整份导出 xlsx（完整数据，与内联互不依赖：一边成功一边失败也照常报告）
    //
    // 同一个工作簿的第二个引用不再重复落盘，直接指向第一份；只有当第一份没落成
    // （导出失败）时才补导，避免出现"链接指向一个不存在的文件"。
    let file_name = format!("内嵌表格_{:02}.xlsx", index.max(1));
    let reused = ctx.tables_dir.join(&file_name);
    if !produce_file && reused.is_file() {
        parts.push(format!(
            "完整数据：[{file_name}]({}/{file_name})",
            ctx.tables_dir_name
        ));
    } else {
        match fs::create_dir_all(ctx.tables_dir) {
            Ok(()) => {
                let target = ctx.tables_dir.join(&file_name);
                match lark::sheets_export_controlled(
                    &url,
                    &target.to_string_lossy(),
                    ctx.mode,
                    ctx.cancelled,
                ) {
                    Ok(_) => {
                        parts.push(format!(
                            "完整数据：[{file_name}]({}/{file_name})",
                            ctx.tables_dir_name
                        ));
                        exported = true;
                    }
                    Err(error) => {
                        failed = true;
                        let message = format!("{label}（{token}）导出 xlsx 失败：{error}");
                        crate::logger::warn(format!("文档内嵌表格：{message}"));
                        errors.push(message.clone());
                        parts.push(message);
                    }
                }
            }
            Err(error) => {
                failed = true;
                let message = format!("创建内嵌表格目录失败：{error}");
                errors.push(message.clone());
                parts.push(message);
            }
        }
    }

    compose_embedded_note(label, token, failed, exported, &parts)
}

/// 处理一个内嵌多维表格：每张数据表导出为 NDJSON
///
/// 与知识库里独立的多维表格节点走的是同一对命令（`base +table-list` /
/// `base +record-list`），引用片段里的 token 就是 base token，所以直接复用。
fn render_embedded_bitable(
    label: &str,
    token: &str,
    ctx: &EmbeddedContext<'_>,
    index: usize,
    errors: &mut Vec<String>,
) -> RefOutcome {
    let tables: AppResult<Vec<serde_json::Value>> =
        lark::base_table_list_controlled(token, ctx.cancelled).and_then(|data| {
            data.get("items")
                .or_else(|| data.get("tables"))
                .and_then(|value| value.as_array())
                .cloned()
                .ok_or_else(|| {
                    AppError::LarkCliResponse("无法解析多维表格的数据表列表".to_string())
                })
        });

    let tables = match tables {
        Ok(tables) => tables,
        Err(error) => {
            let message = format!("{label}（{token}）读取失败：{error}");
            crate::logger::warn(format!("文档内嵌多维表格：{message}"));
            errors.push(message.clone());
            return compose_embedded_note(label, token, true, false, &[message]);
        }
    };

    if tables.is_empty() {
        let message = format!("{label}（{token}）没有可导出的数据表");
        crate::logger::warn(format!("文档内嵌多维表格：{message}"));
        errors.push(message.clone());
        return compose_embedded_note(label, token, true, false, &[message]);
    }

    if let Err(error) = fs::create_dir_all(ctx.tables_dir) {
        let message = format!("创建内嵌表格目录失败：{error}");
        errors.push(message.clone());
        return compose_embedded_note(label, token, true, false, &[message]);
    }

    let mut parts: Vec<String> = Vec::new();
    let mut exported = false;
    let mut failed = false;

    for (seq, table) in tables.iter().enumerate() {
        let Some(table_id) = table
            .get("table_id")
            .or_else(|| table.get("id"))
            .and_then(|value| value.as_str())
        else {
            failed = true;
            let message = format!("{label}（{token}）第 {} 张数据表缺少 table_id", seq + 1);
            crate::logger::warn(format!("文档内嵌多维表格：{message}"));
            errors.push(message.clone());
            parts.push(message);
            continue;
        };
        let name = table
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or(table_id);
        let file_name = format!(
            "内嵌多维表格_{:02}_{:02}_{}.ndjson",
            index.max(1),
            seq + 1,
            markdown::safe_filename(name)
        );
        let target = ctx.tables_dir.join(&file_name);
        match lark::base_records_export_controlled(
            token,
            table_id,
            &target.to_string_lossy(),
            ctx.cancelled,
        ) {
            Ok(_) => {
                parts.push(format!(
                    "- {name}：[{file_name}]({}/{file_name})",
                    ctx.tables_dir_name
                ));
                exported = true;
            }
            Err(error) => {
                failed = true;
                let message = format!("{label}（{token}）数据表「{name}」导出失败：{error}");
                crate::logger::warn(format!("文档内嵌多维表格：{message}"));
                errors.push(message.clone());
                parts.push(message);
            }
        }
    }

    compose_embedded_note(label, token, failed, exported, &parts)
}
