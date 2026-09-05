//! Markdown 解析与图片引用处理
//!
//! 职责：
//! 1. 从 Markdown 文本中提取所有图片引用
//! 2. 提取图片 URL 中的 file_token
//! 3. 将远程 URL 替换为本地相对路径
//! 4. 文件名安全化处理

use pulldown_cmark::{Event, Parser, Tag};
use regex::Regex;

use crate::models::ImageRef;

/// 从 Markdown 内容中提取所有图片引用
///
/// 匹配模式：`![描述](url)`
/// 返回 ImageRef 列表，包含 alt、url、file_token
pub fn extract_images(content: &str) -> Vec<ImageRef> {
    let mut images = Vec::new();
    let mut current: Option<(String, String)> = None;
    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                current = Some((dest_url.into_string(), String::new()))
            }
            Event::Text(text) | Event::Code(text) if current.is_some() => {
                if let Some((_, alt)) = &mut current {
                    alt.push_str(&text);
                }
            }
            Event::End(pulldown_cmark::TagEnd::Image) => {
                if let Some((url, alt)) = current.take() {
                    if !url.is_empty() {
                        images.push(ImageRef {
                            file_token: extract_file_token(&url),
                            alt,
                            url,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    images
}

/// 从 URL 中提取 file_token
///
/// 支持的格式：
/// - `https://xxx.feishu.cn/file/<token>`
/// - `https://internal-api-drive-stream.feishu.cn/suite/api/v1/file/<token>`
/// - 其他以 `/` 分隔的 URL，取最后一段作为 token
fn extract_file_token(url: &str) -> String {
    // 去掉 query string
    let base_url = url.split('?').next().unwrap_or(url);
    // 去掉末尾的 /
    let trimmed = base_url.trim_end_matches('/');
    // 取最后一段
    let token = trimmed.rsplit('/').next().unwrap_or("");
    token.to_string()
}

/// 将 Markdown 中的远程图片 URL 替换为本地相对路径
///
/// - `url_map`: 远程 URL → 本地相对路径 的映射
///
/// 返回替换后的 Markdown 内容
pub fn replace_image_urls(content: &str, url_map: &[(String, String)]) -> String {
    let mappings: std::collections::HashMap<&str, &str> = url_map
        .iter()
        .map(|(remote, local)| (remote.as_str(), local.as_str()))
        .collect();
    let mut replacements = Vec::new();
    for (event, span) in Parser::new(content).into_offset_iter() {
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            let source = &content[span.clone()];
            let destination = dest_url.as_ref();
            if let Some(local) = mappings.get(destination) {
                if let Some(relative) = source.find(destination) {
                    let start = span.start + relative;
                    replacements.push((start..start + destination.len(), (*local).to_string()));
                }
            }
        }
    }
    let mut result = content.to_string();
    for (span, local) in replacements.into_iter().rev() {
        result.replace_range(span, &local);
    }
    result
}

/// 将文件名安全化
///
/// - 替换 Windows / macOS 不允许的字符为 `_`
/// - 截断到 100 个字符
/// - 去掉首尾空格和点号
pub fn safe_filename(name: &str) -> String {
    let re = Regex::new(r#"[\\/:*?"<>|]"#).unwrap();
    let cleaned = re.replace_all(name, "_").to_string();
    let trimmed = cleaned.trim().trim_end_matches('.').to_string();
    // 安全截断到 100 个字符（不切断多字节字符）
    let trimmed = if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    };
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = trimmed.split('.').next().unwrap_or(&trimmed);
    let trimmed = if cfg!(windows) && reserved.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        format!("_{}", trimmed)
    } else {
        trimmed
    };
    if trimmed.chars().count() > 100 {
        trimmed.chars().take(100).collect()
    } else {
        trimmed
    }
}

/// 生成带位置前缀的文件/目录名
///
/// 格式：`{position:02d}_{title}`
/// 例如：`01_第一章`、`02_技术文档`
pub fn prefixed_filename(position: usize, title: &str) -> String {
    let safe = safe_filename(title);
    format!("{:02}_{}", position, safe)
}

/// 生成文档的图片目录名
///
/// 格式：`{filename}_images`
/// 例如：`01_第一章_images`
pub fn images_dir_name(filename: &str) -> String {
    // 去掉 .md 后缀
    let stem = filename.trim_end_matches(".md");
    format!("{}_images", stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_images() {
        let content = r#"# 标题

![图片1](https://feishu.cn/file/boxcn1abc123)

一些文字

![图片2描述](https://internal-api-drive-stream.feishu.cn/suite/api/v1/file/boxcn2def456?param=1)
"#;
        let images = extract_images(content);
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].alt, "图片1");
        assert_eq!(images[0].file_token, "boxcn1abc123");
        assert_eq!(images[1].alt, "图片2描述");
        assert_eq!(images[1].file_token, "boxcn2def456");
    }

    #[test]
    fn test_replace_image_urls() {
        let content = "![img](https://feishu.cn/file/token123)";
        let replaced = replace_image_urls(
            content,
            &[(
                "https://feishu.cn/file/token123".to_string(),
                "images/img_01.png".to_string(),
            )],
        );
        assert_eq!(replaced, "![img](images/img_01.png)");
    }

    #[test]
    fn test_replace_encoded_image_url_by_source_span() {
        let content = "![encoded](https://feishu.cn/file/token%20name)";
        let replaced = replace_image_urls(
            content,
            &[(
                "https://feishu.cn/file/token%20name".to_string(),
                "images/img_01.png".to_string(),
            )],
        );
        assert_eq!(replaced, "![encoded](images/img_01.png)");
    }

    #[test]
    fn test_safe_filename() {
        assert_eq!(safe_filename("hello/world"), "hello_world");
        assert_eq!(safe_filename("test:*?file"), "test___file");
        assert_eq!(safe_filename("正常标题"), "正常标题");
        assert_eq!(safe_filename("  trim  "), "trim");
    }

    #[test]
    fn test_prefixed_filename() {
        assert_eq!(prefixed_filename(0, "第一章"), "00_第一章");
        assert_eq!(prefixed_filename(3, "技术"), "03_技术");
        assert_eq!(prefixed_filename(12, "test"), "12_test");
    }

    #[test]
    fn test_images_dir_name() {
        assert_eq!(images_dir_name("01_第一章.md"), "01_第一章_images");
        assert_eq!(images_dir_name("test"), "test_images");
    }

    #[test]
    fn test_extract_file_token_with_query() {
        let token = extract_file_token("https://feishu.cn/file/abc123?param=1&x=2");
        assert_eq!(token, "abc123");
    }
}
