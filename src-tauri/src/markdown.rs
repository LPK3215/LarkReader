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

/// 生成文档的内嵌表格目录名
///
/// 格式：`{filename}_tables`，与 `_images` 同级、同样按文档隔离。
/// 例如：`01_第一章_tables`
pub fn tables_dir_name(filename: &str) -> String {
    let stem = filename.trim_end_matches(".md");
    format!("{}_tables", stem)
}

// ============================================================================
// 文档内嵌富对象（电子表格 / 多维表格 / 画板 / 原生表格）
// ============================================================================

/// 内嵌富对象类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichRefKind {
    /// 内嵌电子表格（sheet block）：数据在另一张表里，正文只留引用
    Sheet,
    /// 内嵌多维表格
    Bitable,
    /// 画板
    Whiteboard,
    /// 媒体标签形态的图片（`<image token="…"/>` / `<img src="…">`）
    ///
    /// Markdown 形态的图片由 `extract_images` 处理；这里管的是**标签形态**——
    /// 只认 Markdown 的话，这类图片会一个都下不下来。
    Image,
    /// 原生表格（DocxXML `<table>…</table>` 片段，未被转成 Markdown 表格）
    Table,
    /// 未识别的独占一行的标签：不是已知富对象，也不是常见 HTML 写法
    ///
    /// 兜底用：匹配不到不能等于"悄悄丢掉"，必须让用户看得见。
    Unknown,
}

/// 已知会被单独处理的块级标签
const KNOWN_BLOCK_TAGS: &[&str] = &[
    "sheet",
    "bitable",
    "base_refer",
    "whiteboard",
    "image",
    "img",
    "table",
];

/// 正文里合法出现的 HTML 标签：正常写法，不代表"有内容没被还原"
const HTML_TAGS: &[&str] = &[
    "a", "b", "blockquote", "br", "center", "code", "del", "details", "div", "em", "font", "h1",
    "h2", "h3", "h4", "h5", "h6", "hr", "i", "ins", "kbd", "li", "mark", "ol", "p", "pre", "s",
    "small", "span", "strong", "sub", "summary", "sup", "ul",
];

/// 文档正文中的一处内嵌富对象引用
#[derive(Debug, Clone)]
pub struct RichRef {
    pub kind: RichRefKind,
    /// 富对象 token（电子表格 / 多维表格 base / 画板 / 图片文件 token）
    pub token: Option<String>,
    /// 电子表格的子表 id（`sheet-id` 属性，可能缺失）
    pub sheet_id: Option<String>,
    /// 标签名（`sheet` / `table` / `image` / 未知标签…），用于提示与日志
    pub tag: String,
    /// 原文片段，用于替换
    pub raw: String,
    /// 在正文中的字节起始位置
    pub start: usize,
    /// 在正文中的字节结束位置（不含）
    pub end: usize,
}

/// 找出正文里所有内嵌富对象引用
///
/// 为什么需要它：`docs +fetch --doc-format markdown` 对"文档里嵌的电子表格"这类
/// 富对象**不做内容转换**，正文里只留下一个引用片段（DocxXML 标签，或 IM 形态的
/// `[sheet xxx](url)` 链接）。不做处理的话，导出的 `.md` 里那张表就是空的或一串
/// 裸标签——这是 2026-10 用户反馈"文档中嵌入的表格导出后是空的"的现象。
///
/// 设计原则是**全文检索、按形态分派**，而不是只认某几种写法：
/// - 已知富对象标签：**任意位置**都认（宽容，宁可多认也别漏）；
/// - 图片的标签形态（`<image>` / `<img>`）也归这里，否则这类图片一个都下不下来；
/// - 其余标签：只有**整行就是一个标签**时才当作"未识别的块"（收敛，避免把行内
///   HTML 标签、`<https://…>` 自动链接误判成没还原的内容）。
///
/// 返回的片段**互不重叠**且按出现顺序排列，可直接从后往前做字节替换。
pub fn find_rich_refs(content: &str) -> Vec<RichRef> {
    let mut refs: Vec<RichRef> = Vec::new();

    // ① 原生表格整块：`<table>…</table>`（先整块取，避免与下面的标签扫描重叠）
    let table_re = Regex::new(r"(?is)<table\b[^>]*>.*?</table>").unwrap();
    for m in table_re.find_iter(content) {
        refs.push(RichRef {
            kind: RichRefKind::Table,
            token: None,
            sheet_id: None,
            tag: "table".to_string(),
            raw: m.as_str().to_string(),
            start: m.start(),
            end: m.end(),
        });
    }

    // ② 已知富对象标签：`<sheet token="…" sheet-id="…"/>`、`<bitable …>`、`<image …>`
    //
    // 刻意不用"取到闭合标签"的写法：这类块多数是自闭合的，且服务端输出的属性顺序
    // 不保证（`token` 可能在 `sheet-id` 之后），只按属性名取值，不依赖顺序。
    let tag_re =
        Regex::new(r#"(?is)<\s*(sheet|bitable|base_refer|whiteboard|image|img)\b[^<>]*?/?>"#)
            .unwrap();
    let token_re = Regex::new(r#"(?i)\btoken\s*=\s*["']([^"']*)["']"#).unwrap();
    let src_re = Regex::new(r#"(?i)\bsrc\s*=\s*["']([^"']*)["']"#).unwrap();
    let sheet_id_re = Regex::new(r#"(?i)\bsheet-id\s*=\s*["']([^"']*)["']"#).unwrap();
    for caps in tag_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        let name = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        let kind = match name.as_str() {
            "sheet" => RichRefKind::Sheet,
            "whiteboard" => RichRefKind::Whiteboard,
            "image" | "img" => RichRefKind::Image,
            _ => RichRefKind::Bitable,
        };
        let raw = whole.as_str();
        let attr = |re: &Regex| -> Option<String> {
            re.captures(raw)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .filter(|value| !value.is_empty())
        };
        // 图片没有 `token` 属性时退到 `src`，并从 URL 里取文件 token
        let token = match kind {
            RichRefKind::Image => attr(&token_re).or_else(|| {
                attr(&src_re)
                    .map(|src| extract_file_token(&src))
                    .filter(|token| !token.is_empty())
            }),
            _ => attr(&token_re),
        };
        refs.push(RichRef {
            kind,
            token,
            sheet_id: attr(&sheet_id_re),
            tag: name,
            raw: raw.to_string(),
            start: whole.start(),
            end: whole.end(),
        });
    }

    // ②-b 成对标签：服务端把内嵌表格写成 `<sheet …></sheet>` 这种**成对**形态，
    // 而不是自闭合。只吃开标签的话，渲染结果后面会拖一个裸 `</sheet>` 留在正文里。
    // Rust 的 regex 不支持反向引用，没法在一条正则里配对，所以这里按"紧随其后的
    // 同名闭合标签"补齐区间：命中才扩展，`<image …/>` 这类自闭合写法不受影响。
    for reference in refs.iter_mut() {
        if !KNOWN_BLOCK_TAGS.contains(&reference.tag.as_str()) {
            continue;
        }
        let rest = &content[reference.end..];
        let trimmed = rest.trim_start();
        let close = format!("</{}>", reference.tag);
        let Some(head) = trimmed.get(..close.len()) else {
            continue;
        };
        if head.eq_ignore_ascii_case(&close) {
            reference.end += (rest.len() - trimmed.len()) + close.len();
            reference.raw = content[reference.start..reference.end].to_string();
        }
    }

    // ③ IM 形态：`[sheet s1](https://xxx.feishu.cn/sheets/<token>)`
    let link_re = Regex::new(r"\[sheet\s+([^\]]*)\]\((https?://[^)\s]+)\)").unwrap();
    for caps in link_re.captures_iter(content) {
        let (Some(whole), Some(url)) = (caps.get(0), caps.get(2)) else {
            continue;
        };
        let token = url
            .as_str()
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();
        if token.is_empty() {
            continue;
        }
        let sheet_id = caps
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .filter(|value| !value.is_empty());
        refs.push(RichRef {
            kind: RichRefKind::Sheet,
            token: Some(token),
            sheet_id,
            tag: "sheet".to_string(),
            raw: whole.as_str().to_string(),
            start: whole.start(),
            end: whole.end(),
        });
    }

    // ④ 兜底：整行只有一个标签，且既不是已知富对象、也不是常见 HTML 写法
    //
    // 目的只有一个：**匹配不到不等于可以悄悄丢**。这类片段会在正文里保留原文，
    // 由调用方在文末给一段可见提示并写日志，用户不会以为"导出成功了但内容没了"。
    // 注意：Rust 的 regex 不支持前瞻（`(?=…)`），所以用"属性段可选"的写法来表达
    // "标签名后面必须是空白、`/` 或 `>`"，同时挡住 `<https://…>` 这类自动链接。
    let line_tag_re =
        Regex::new(r"(?m)^[ \t]*<\s*([a-zA-Z][a-zA-Z0-9_-]*)(?:\s[^<>]*?)?/?>[ \t]*$").unwrap();
    for caps in line_tag_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        let name = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        if name.is_empty()
            || KNOWN_BLOCK_TAGS.contains(&name.as_str())
            || HTML_TAGS.contains(&name.as_str())
        {
            continue;
        }
        refs.push(RichRef {
            kind: RichRefKind::Unknown,
            token: None,
            sheet_id: None,
            tag: name,
            raw: whole.as_str().to_string(),
            start: whole.start(),
            end: whole.end(),
        });
    }

    // 围栏代码块里的 `<sheet …/>` 是**示例文本**，不是真块：照着处理会把示例
    // 换成提示，等于把正文改坏。这块整体跳过。
    let fenced = fenced_code_ranges(content);
    refs.retain(|reference| {
        !fenced
            .iter()
            .any(|(start, end)| reference.start >= *start && reference.start < *end)
    });

    // 按位置排序并丢掉被前一个片段覆盖的引用，保证互不重叠
    refs.sort_by_key(|reference| reference.start);
    let mut cleaned: Vec<RichRef> = Vec::with_capacity(refs.len());
    let mut cursor = 0usize;
    for reference in refs {
        if reference.start < cursor {
            continue;
        }
        cursor = reference.end;
        cleaned.push(reference);
    }
    cleaned
}

/// 围栏代码块（``` / ~~~）覆盖的字节区间
///
/// 用于把"正文示例"和"真实内嵌块"区分开：代码块里写 `<sheet token="…"/>` 是在
/// 讲解格式，不是文档真的嵌了一张表。
fn fenced_code_ranges(content: &str) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut open: Option<usize> = None;
    let mut fence = String::new();
    let mut offset = 0usize;

    for line in content.split_inclusive('\n') {
        let marker: String = line
            .trim_start()
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        if marker.len() >= 3 {
            match open {
                None => {
                    open = Some(offset);
                    fence = marker;
                }
                Some(start) => {
                    // 闭合围栏：同一种字符、长度不短于开启的那个
                    if fence.chars().next() == marker.chars().next() && marker.len() >= fence.len()
                    {
                        ranges.push((start, offset + line.len()));
                        open = None;
                        fence.clear();
                    }
                }
            }
        }
        offset += line.len();
    }
    if let Some(start) = open {
        ranges.push((start, content.len()));
    }
    ranges
}

/// CSV → Markdown 表格的转换结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvTable {
    /// Markdown 表格文本；没有任何数据行时为空串
    pub markdown: String,
    /// 实际写进正文的数据行数（不含表头与分隔行）
    pub rows_used: usize,
    /// 表格里真实存在的数据行数（已去掉全空行）
    pub rows_total: usize,
    /// 列数
    pub columns: usize,
}

/// 把 CSV 文本转成 Markdown 管道表（用于把内嵌表格直接写进正文）
///
/// - 全空行直接跳过：`csv-get` 读整表时会带回大量尾部幽灵空行，写进正文就是噪音
/// - 单元格内的换行/制表符折叠成空格，`|` 转义成 `\|`，避免撑破表格结构
/// - 超过 `max_rows` 行时只取前 `max_rows` 行（含表头），由调用方提示"已截断"
pub fn csv_to_markdown_table(csv_text: &str, max_rows: usize) -> CsvTable {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_text.as_bytes());
    for record in reader.records().flatten() {
        let row: Vec<String> = record.iter().map(normalize_table_cell).collect();
        if row.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        rows.push(row);
    }

    let rows_total = rows.len();
    if rows_total == 0 {
        return CsvTable {
            markdown: String::new(),
            rows_used: 0,
            rows_total: 0,
            columns: 0,
        };
    }

    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let rows_used = rows_total.min(max_rows.max(1));

    let mut out = String::new();
    out.push('|');
    for index in 0..columns {
        out.push(' ');
        out.push_str(table_cell(&rows[0], index));
        out.push_str(" |");
    }
    out.push('\n');
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows.iter().take(rows_used).skip(1) {
        out.push('|');
        for index in 0..columns {
            out.push(' ');
            out.push_str(table_cell(row, index));
            out.push_str(" |");
        }
        out.push('\n');
    }

    CsvTable {
        markdown: out,
        rows_used,
        rows_total,
        columns,
    }
}

/// 取第 `index` 列；该行不足时给空串，保证每行列数一致
fn table_cell(row: &[String], index: usize) -> &str {
    row.get(index).map(String::as_str).unwrap_or("")
}

/// 单元格规范化：折叠所有空白（含换行）为单个空格，转义表格分隔符
fn normalize_table_cell(field: &str) -> String {
    field
        .replace('|', "\\|")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

    #[test]
    fn test_tables_dir_name() {
        assert_eq!(tables_dir_name("01_第一章.md"), "01_第一章_tables");
        assert_eq!(tables_dir_name("test"), "test_tables");
    }

    #[test]
    fn test_find_rich_refs_docxxml_tags() {
        let content = "正文\n\n<sheet token=\"ShtToken1\" sheet-id=\"abc\"/>\n\n尾部";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Sheet);
        assert_eq!(refs[0].token.as_deref(), Some("ShtToken1"));
        assert_eq!(refs[0].sheet_id.as_deref(), Some("abc"));
        assert_eq!(&content[refs[0].start..refs[0].end], refs[0].raw);
    }

    /// 成对标签形态：服务端实际输出的是 `<sheet …></sheet>`（不是自闭合）。
    /// 只吃开标签的话，渲染结果后面会拖一个裸 `</sheet>` 留在正文里。
    #[test]
    fn test_find_rich_refs_paired_closing_tag_is_consumed() {
        let content = "正文\n\n<sheet sheet-id=\"1FFRaa\" token=\"TOK\"></sheet>\n\n尾部";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Sheet);
        assert_eq!(refs[0].token.as_deref(), Some("TOK"));
        assert_eq!(
            &content[refs[0].start..refs[0].end],
            "<sheet sheet-id=\"1FFRaa\" token=\"TOK\"></sheet>"
        );
        assert_eq!(refs[0].raw, &content[refs[0].start..refs[0].end]);

        // 自闭合写法不受影响：区间仍只覆盖标签本身
        let self_closing = "<sheet token=\"TOK\"/>";
        let refs = find_rich_refs(self_closing);
        assert_eq!(refs[0].end, self_closing.len());
    }

    /// 属性顺序不固定：`sheet-id` 写在 `token` 前面也必须能取到 token。
    #[test]
    fn test_find_rich_refs_attribute_order_agnostic() {
        let refs = find_rich_refs("<sheet sheet-id='s1' token='TOK'/>");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].token.as_deref(), Some("TOK"));
        assert_eq!(refs[0].sheet_id.as_deref(), Some("s1"));
    }

    #[test]
    fn test_find_rich_refs_bitable_and_whiteboard() {
        let refs = find_rich_refs("<bitable token=\"BaseTok\"/>\n<whiteboard token=\"WbTok\"/>");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].kind, RichRefKind::Bitable);
        assert_eq!(refs[0].token.as_deref(), Some("BaseTok"));
        assert_eq!(refs[1].kind, RichRefKind::Whiteboard);
        assert_eq!(refs[1].token.as_deref(), Some("WbTok"));
    }

    #[test]
    fn test_find_rich_refs_im_markdown_link() {
        let content = "见 [sheet 7f1](https://x.feishu.cn/sheets/SHT2?from=doc) 结尾";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Sheet);
        assert_eq!(refs[0].token.as_deref(), Some("SHT2"));
        assert_eq!(refs[0].sheet_id.as_deref(), Some("7f1"));
    }

    /// 原生表格整块优先：`<table>` 内部的标签不能再单独被算成一个引用，
    /// 否则从后往前替换时字节偏移会互相踩踏。
    #[test]
    fn test_find_rich_refs_table_block_has_no_overlap() {
        let content = "<table><tr><td>a</td></tr></table>";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Table);
        assert_eq!(refs[0].start, 0);
        assert_eq!(refs[0].end, content.len());
    }

    #[test]
    fn test_find_rich_refs_plain_markdown_is_untouched() {
        let content = "# 标题\n\n| 姓名 | 部门 |\n| --- | --- |\n| 张三 | 技术部 |\n";
        assert!(find_rich_refs(content).is_empty());
    }

    /// 图片的标签形态：只认 Markdown 形态的话，这类图片会一个都下不下来。
    #[test]
    fn test_find_rich_refs_image_tags() {
        let refs = find_rich_refs("<image token=\"boxcn1\" width=\"100\"/>");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Image);
        assert_eq!(refs[0].token.as_deref(), Some("boxcn1"));
        assert_eq!(refs[0].tag, "image");

        // 没有 token 时退到 src，并从 URL 里取文件 token
        let refs = find_rich_refs("<img src=\"https://x.feishu.cn/file/boxcn2?y=1\" />");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Image);
        assert_eq!(refs[0].token.as_deref(), Some("boxcn2"));
    }

    /// 兜底：整行一个陌生标签 = 有可能是没被还原的块，必须让上层看得见。
    #[test]
    fn test_find_rich_refs_unknown_block_tag() {
        let refs = find_rich_refs("正文\n\n<synced_reference token=\"x\"/>\n\n尾部");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Unknown);
        assert_eq!(refs[0].tag, "synced_reference");
    }

    /// 收敛：行内 HTML、独立成行的 `<div>`、`<https://…>` 自动链接都不是"没还原的内容"。
    #[test]
    fn test_find_rich_refs_ignores_inline_html_and_autolinks() {
        let content = "行内 <span>文字</span> 与 <br>\n\n<div align=\"center\">居中</div>\n\n<https://example.com>\n\n<details>\n<summary>展开</summary>\n</details>\n";
        assert!(find_rich_refs(content).is_empty(), "{:?}", find_rich_refs(content));
    }

    /// 围栏代码块里的标签是示例文本，处理它会反过来把正文改坏。
    #[test]
    fn test_find_rich_refs_skips_fenced_code_blocks() {
        let content = "说明：\n\n```xml\n<sheet token=\"FAKE\"/>\n<synced_reference token=\"FAKE2\"/>\n```\n\n<sheet token=\"REAL\"/>\n";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1, "{:?}", refs);
        assert_eq!(refs[0].token.as_deref(), Some("REAL"));
        assert_eq!(&content[refs[0].start..refs[0].end], "<sheet token=\"REAL\"/>");
    }

    /// 未闭合的围栏也要算进保护区，不能因为少了收尾就漏出去。
    #[test]
    fn test_find_rich_refs_skips_unclosed_fence() {
        let content = "开头\n\n```\n<sheet token=\"FAKE\"/>\n";
        assert!(find_rich_refs(content).is_empty());
    }

    #[test]
    fn test_csv_to_markdown_table_basic() {
        let table = csv_to_markdown_table("姓名,部门\n张三,技术部\n李四,产品部\n", 200);
        assert_eq!(table.rows_total, 3);
        assert_eq!(table.rows_used, 3);
        assert_eq!(table.columns, 2);
        assert_eq!(
            table.markdown,
            "| 姓名 | 部门 |\n| --- | --- |\n| 张三 | 技术部 |\n| 李四 | 产品部 |\n"
        );
    }

    /// `csv-get` 读整表会带回大量尾部幽灵空行，不能写进正文。
    #[test]
    fn test_csv_to_markdown_table_drops_blank_rows() {
        let table = csv_to_markdown_table("a,b\n1,2\n,\n,\n\n", 200);
        assert_eq!(table.rows_total, 2);
        assert_eq!(table.markdown.matches('\n').count(), 3);
    }

    #[test]
    fn test_csv_to_markdown_table_escapes_and_truncates() {
        let table = csv_to_markdown_table("h\nA|B\nC\nD", 3);
        assert_eq!(table.rows_total, 4);
        assert_eq!(table.rows_used, 3, "含表头最多取 3 行");
        assert!(table.markdown.contains("A\\|B"), "竖线必须转义：{}", table.markdown);
        assert!(!table.markdown.contains("D"), "超出行数不应写入：{}", table.markdown);
    }

    #[test]
    fn test_csv_to_markdown_table_empty_input() {
        let table = csv_to_markdown_table("", 200);
        assert!(table.markdown.is_empty());
        assert_eq!(table.rows_total, 0);
        assert_eq!(table.columns, 0);
    }
}
