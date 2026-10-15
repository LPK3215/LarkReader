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
/// - 直接就是 token 的裸串（表格 HTML 里的 `<img src="token"/>`）
pub fn extract_file_token(url: &str) -> String {
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
    // 控制字符（换行、制表、零宽等）一律换成下划线：正文里的 `<title>` 可能夹带
    // `&#xA;` 解码出的换行，原样落进文件名在 Windows 上是非法字符。
    let cleaned: String = cleaned
        .chars()
        .map(|character| if character.is_control() { '_' } else { character })
        .collect();
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
        // 截断点可能正好落在点号或空格上——原样留下会在 Windows 上生成非法文件名，
        // 所以截断后再收一次尾。
        trimmed
            .chars()
            .take(100)
            .collect::<String>()
            .trim_end_matches([' ', '.'])
            .to_string()
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
    /// 内嵌 HTML 内容块（`<html5-block data-ref="html5_1"/>`）
    ///
    /// 正文不在文档里：它挂在 `docs +fetch` 的 `reference_map["html5-block"]` 下，
    /// 必须靠 `data-ref` 去取，否则这块内容整段丢失。
    Html5,
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
    "html5-block",
];

/// 正文里合法出现的 HTML 标签：正常写法，不代表"有内容没被还原"
const HTML_TAGS: &[&str] = &[
    "a", "b", "blockquote", "br", "center", "code", "del", "details", "div", "em", "font", "h1",
    "h2", "h3", "h4", "h5", "h6", "hr", "i", "ins", "kbd", "li", "mark", "ol", "p", "pre", "s",
    "small", "span", "strong", "sub", "summary", "sup", "title", "ul",
];

/// 残留了也不算"内容没还原"的标签
///
/// - `sub-page` / `sub-page-list`：子页面正文会各自导出成独立文件，正文里那份只是索引
const BENIGN_TAGS: &[&str] = &["sub-page", "sub-page-list"];

/// 文档正文中的一处内嵌富对象引用
#[derive(Debug, Clone)]
pub struct RichRef {
    pub kind: RichRefKind,
    /// 富对象 token（电子表格 / 多维表格 base / 画板 / 图片文件 token）
    pub token: Option<String>,
    /// 电子表格的子表 id（`sheet-id` 属性，可能缺失）
    pub sheet_id: Option<String>,
    /// 内嵌 HTML 内容块的引用键（`data-ref` 属性）
    ///
    /// `<html5-block>` 正文不在文档里，要用这个键去
    /// `docs +fetch` 返回的 `reference_map["html5-block"]` 里取回。
    pub data_ref: Option<String>,
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
            data_ref: None,
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
    let tag_re = Regex::new(
        r#"(?is)<\s*(sheet|bitable|base_refer|whiteboard|image|img|html5-block)\b[^<>]*?/?>"#,
    )
    .unwrap();
    let token_re = Regex::new(r#"(?i)\btoken\s*=\s*["']([^"']*)["']"#).unwrap();
    let src_re = Regex::new(r#"(?i)\bsrc\s*=\s*["']([^"']*)["']"#).unwrap();
    let sheet_id_re = Regex::new(r#"(?i)\bsheet-id\s*=\s*["']([^"']*)["']"#).unwrap();
    let data_ref_re = Regex::new(r#"(?i)\bdata-ref\s*=\s*["']([^"']*)["']"#).unwrap();
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
            "html5-block" => RichRefKind::Html5,
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
            data_ref: attr(&data_ref_re),
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
            data_ref: None,
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
            data_ref: None,
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

/// 用围栏代码块包住任意文本，围栏长度按内容里最长的连续反引号自适应
///
/// 为什么不能固定用三个反引号：内嵌 HTML 里完全可能自带 ``` 片段，一旦与围栏
/// 等长，围栏会被内容提前"闭合"，后面所有正文都会被吞进代码块。这里取"内容里
/// 最长反引号串 + 1"（且不短于 3），保证围栏比内容里任何反引号串都长。
pub fn wrap_fence(lang: &str, content: &str) -> String {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in content.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    let fence = "`".repeat(longest.max(2) + 1);
    // 内容尾部多余的换行去掉，由这里的结构负责换行，保证闭合围栏独占一行
    let body = content.trim_end_matches(['\n', '\r']);
    format!("{}{}\n{}\n{}", fence, lang, body, fence)
}

/// 扫描正文里"按原文保留、未被还原"的残留标签名（去重，按出现顺序）
///
/// 用途：导出后给用户一句可见提示。为什么不复用 `find_rich_refs` 的 `Unknown`：
/// 那只认"整行只有一个标签"的写法，而 `<bookmark …>` / `<button …>` / `<cite …>`
/// 这些服务端标签常常与文字同行出现，只靠整行匹配会漏掉——漏掉就等于"悄悄丢了
/// 内容"，这正是本函数要堵的洞。
///
/// 排除项：
/// - 围栏代码块内的文本（那是示例，不是真标签）
/// - `HTML_TAGS`（正常 HTML 写法）与 `BENIGN_TAGS`（子页面索引，正文另存）
/// - `<https://…>` 这类自动链接（标签名后紧跟 `:`）
///
/// 还要"看起来真的是标签"才算：带属性（`<bookmark name="…">`）或成对出现
/// （`<grid>…</grid>`）。只有"光秃秃的单标签"一律不当真——那更可能是作者手打的
/// 字面文本（压测文档里就故意写了 `<tag>`），报成"内容没还原"是误报。
pub fn unresolved_tags(content: &str) -> Vec<String> {
    let tag_re = Regex::new(r"(?is)</?\s*([a-zA-Z][a-zA-Z0-9_-]*)((?:\s[^<>]*?)?)\s*/?>").unwrap();
    let close_re = Regex::new(r"(?is)</\s*([a-zA-Z][a-zA-Z0-9_-]*)\s*>").unwrap();
    let closings: Vec<String> = close_re
        .captures_iter(content)
        .filter_map(|caps| caps.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .collect();
    let fenced = fenced_code_ranges(content);
    let mut seen: Vec<String> = Vec::new();
    for caps in tag_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        if fenced
            .iter()
            .any(|(start, end)| whole.start() >= *start && whole.start() < *end)
        {
            continue;
        }
        let name = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        if name.is_empty()
            || HTML_TAGS.contains(&name.as_str())
            || BENIGN_TAGS.contains(&name.as_str())
        {
            continue;
        }
        // `<https://…>` 这类自动链接：标签名后面紧跟 `:`，不是 HTML 标签
        if content[whole.end()..].starts_with(':') {
            continue;
        }
        let has_attrs = caps
            .get(2)
            .map(|m| !m.as_str().trim().is_empty())
            .unwrap_or(false);
        if !has_attrs && !closings.contains(&name) {
            continue;
        }
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

/// 链接文字里的 `[` `]` 转义，否则会破坏 `[文字](url)` 的结构
fn escape_link_text(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

/// 把服务端"原样保留"、但**数据全在属性里、无需联网即可还原**的标签就地转成 Markdown。
///
/// 为什么只认这三个：`bookmark` / `button` / `cite` 的 `href` / `src` / `name` /
/// `user-name` / `title` 都写在属性或紧邻的正文里，不依赖任何请求；留着原文只会让用户
/// 对着一串 XML 发懵（"导出成功"但看着像坏文件）。其余标签要么要下载附件
/// （`source` / `figure`），要么是结构块（`callout` / `grid`），要么语义已丢
/// （`time`），一律保持原文，由文末的统一提示兜底，不在这里硬转。
///
/// - `<bookmark name="N" href="U"></bookmark>` → `[N](U)`
/// - `<button … src="U">文字</button>` → `[文字](U)`
/// - `<cite type="user" … user-name="张三">` → `@张三`
/// - `<cite type="doc" … title="某文档">` → `@某文档`
/// - `<cite type="citation"><a href="U">标题</a></cite>` → `[标题](U)`
///
/// 围栏代码块里的同名写法是**示例文本**，跳过不转。
pub fn restore_inline_server_tags(content: &str) -> String {
    let fenced = fenced_code_ranges(content);
    let in_fence =
        |start: usize| fenced.iter().any(|(open, close)| start >= *open && start < *close);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    let bookmark_re = Regex::new(r#"(?is)<bookmark\b([^>]*)>\s*</bookmark>"#).unwrap();
    let button_re = Regex::new(r#"(?is)<button\b([^>]*)>(.*?)</button>"#).unwrap();
    let cite_re = Regex::new(r#"(?is)<cite\b([^>]*)>(.*?)</cite>"#).unwrap();

    for caps in bookmark_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        if in_fence(whole.start()) {
            continue;
        }
        let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let (Some(name), Some(href)) = (attr_value(attrs, "name"), attr_value(attrs, "href"))
        else {
            continue;
        };
        let (name, href) = (name.trim(), href.trim());
        if name.is_empty() || href.is_empty() {
            continue;
        }
        edits.push((
            whole.start(),
            whole.end(),
            format!("[{}]({})", escape_link_text(name), href),
        ));
    }

    for caps in button_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        if in_fence(whole.start()) {
            continue;
        }
        let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let text = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        let Some(src) = attr_value(attrs, "src") else {
            continue;
        };
        let src = src.trim();
        if text.is_empty() || src.is_empty() {
            continue;
        }
        edits.push((
            whole.start(),
            whole.end(),
            format!("[{}]({})", escape_link_text(text), src),
        ));
    }

    for caps in cite_re.captures_iter(content) {
        let Some(whole) = caps.get(0) else { continue };
        if in_fence(whole.start()) {
            continue;
        }
        let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let inner = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
        let kind = attr_value(attrs, "type")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let replacement = match kind.as_str() {
            "user" => attr_value(attrs, "user-name")
                .filter(|name| !name.trim().is_empty())
                .map(|name| format!("@{}", name.trim())),
            "doc" => attr_value(attrs, "title")
                .filter(|title| !title.trim().is_empty())
                .map(|title| format!("@{}", title.trim())),
            "citation" => {
                let mut without_image = |_src: &str, _caption: &str| String::new();
                let converted = html_inline_to_markdown(inner, &mut without_image);
                let converted = converted.trim().to_string();
                if converted.is_empty() {
                    None
                } else {
                    Some(converted)
                }
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            edits.push((whole.start(), whole.end(), replacement));
        }
    }

    // 按位置倒序替换，避免前面的替换挪动后面的偏移
    edits.sort_by_key(|(start, _, _)| *start);
    let mut result = content.to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        result.replace_range(start..end, &replacement);
    }
    result
}

/// 从服务端 Markdown 正文里取出文档标题（首行的 `<title>…</title>`）
///
/// 用途：docx 直链（非知识库节点）拿不到 `wiki_node_get` 的标题，此前退化成
/// "token 前 20 个字符"；正文首行其实带着真实标题，用它兜底。
pub fn extract_document_title(content: &str) -> Option<String> {
    let title_re = Regex::new(r"(?is)<title>(.*?)</title>").unwrap();
    if let Some(raw) = title_re.captures(content).and_then(|caps| caps.get(1)) {
        let title = decode_entities(raw.as_str().trim());
        let title = title.trim();
        if !title.is_empty() {
            return Some(title.to_string());
        }
    }
    // 正文里一个标题块都没有时，服务端把**文档标题**渲染成首个 `# 一级标题`
    // （例如 `# 压测H1 · 非法字符/ …`），而不是 `<title>` 标签——那种情况下这一行
    // 就是文档标题，不认它文件名会退化成 20 位 token，用户根本认不出是哪篇。
    let first = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let heading = decode_entities(first.strip_prefix("# ")?.trim());
    let heading = heading.trim();
    if heading.is_empty() {
        None
    } else {
        Some(heading.to_string())
    }
}

/// 正文里是否存在 6 级标题（`######`，Markdown 能表达的最深一级）
pub fn has_six_level_heading(content: &str) -> bool {
    heading_lines(content)
        .iter()
        .any(|(_, _, level, _)| *level == 6)
}

/// 改写塌成 `######` 的深层标题（飞书的 h7/h8/h9）
///
/// 服务端把 `<h7>`/`<h8>`/`<h9>` 一律渲染成 `######`，7/8/9 级在 Markdown 里没有
/// 对应语法，结果三个不同层级变成同一级、无法区分。这里用二次
/// `docs +fetch --doc-format xml` 的层级信息还原：把 `######` 行改写成
/// `###### [h7] 文字`，保留标题结构的同时把真实层级写进正文。
///
/// 安全阀：只要"XML 标题序列"与"Markdown 标题序列"数量不等，或前 6 级的层级
/// 对不上，就**整体放弃**、原样返回——宁可少改，也不能把标题改错位。
pub fn fix_deep_headings(content: &str, xml: &str) -> String {
    let lines = heading_lines(content);
    if lines.is_empty() {
        return content.to_string();
    }
    let levels = xml_heading_levels(xml);
    if levels.len() != lines.len() {
        return content.to_string();
    }
    // 对齐校验：Markdown 里 1~5 级的地方，XML 必须也是同一级
    for (index, (_, _, md_level, _)) in lines.iter().enumerate() {
        if *md_level < 6 && levels[index] != *md_level {
            return content.to_string();
        }
    }
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (index, (start, end, md_level, text)) in lines.iter().enumerate() {
        let xml_level = levels[index];
        if *md_level != 6 || xml_level <= 6 {
            continue;
        }
        replacements.push((
            *start,
            *end,
            format!("###### [h{}] {}\n", xml_level, text),
        ));
    }
    if replacements.is_empty() {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len() + replacements.len() * 8);
    let mut cursor = 0usize;
    for (start, end, new_line) in replacements {
        out.push_str(&content[cursor..start]);
        out.push_str(&new_line);
        cursor = end;
    }
    out.push_str(&content[cursor..]);
    out
}

/// 逐行找出 ATX 标题行，返回 `(起始字节, 结束字节, 层级, 标题文本)`
///
/// 层级 = `#` 的个数（1~6）；`#` 后必须是空白或行尾，避免把 `#标签` 误判成标题；
/// 跳过围栏代码块（里面的 `# 标题` 是示例文本）。
fn heading_lines(content: &str) -> Vec<(usize, usize, usize, String)> {
    let fenced = fenced_code_ranges(content);
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let start = offset;
        let end = offset + line.len();
        offset = end;
        if fenced
            .iter()
            .any(|(fence_start, fence_end)| start >= *fence_start && start < *fence_end)
        {
            continue;
        }
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 {
            continue;
        }
        let rest = &trimmed[hashes..];
        if !(rest.is_empty() || rest.starts_with(' ')) {
            continue;
        }
        out.push((start, end, hashes, rest.trim().to_string()));
    }
    out
}

/// 按出现顺序取出 XML 里所有标题标签的层级（`<h7>` → 7）
fn xml_heading_levels(xml: &str) -> Vec<usize> {
    let heading_re = Regex::new(r"(?is)<h([1-9])\b").unwrap();
    let mut out = Vec::new();
    for caps in heading_re.captures_iter(xml) {
        if let Some(level) = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<usize>().ok())
        {
            out.push(level);
        }
    }
    out
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
    csv_to_markdown_table_marked(csv_text, max_rows, &[])
}

/// 同 [`csv_to_markdown_table`]，但把含图片的格子显式标成 `[图片]`。
///
/// 为什么需要：`+csv-get` 只给文本，图片格在 CSV 里就是空串——内联出来那格看着
/// 像"原表本来就是空的"。调用方先用 `+cells-get` 定位到含图片的格子，再由这里标出。
///
/// `image_cells`：`(真实行号 1-based, 列下标 0-based)`。CSV 由 `+csv-get` 从第 1 行
/// 起分块读取、每行都带回，所以第 i 条记录对应真实第 i+1 行；全空行会被丢弃，
/// 但**含图片的整行不丢**（否则只有一张图的行会连位置都看不见）。
pub fn csv_to_markdown_table_marked(
    csv_text: &str,
    max_rows: usize,
    image_cells: &[(u32, usize)],
) -> CsvTable {
    let mut rows: Vec<Vec<String>> = Vec::new();
    // 与 `rows` 一一对应：该行在真实表里的行号，用于把图片位置落到正确的输出行上
    let mut real_rows: Vec<u32> = Vec::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_text.as_bytes());
    for (line, record) in reader.records().flatten().enumerate() {
        let row: Vec<String> = record.iter().map(normalize_table_cell).collect();
        let row_number = (line + 1) as u32;
        let has_image = image_cells.iter().any(|(row, _)| *row == row_number);
        if row.iter().all(|cell| cell.is_empty()) && !has_image {
            continue;
        }
        rows.push(row);
        real_rows.push(row_number);
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

    // 列数：先取所有行的最大字段数，再裁掉**尾部全空列**。
    //
    // 为什么要裁：读接口按**子表网格尺寸**返回 CSV（网格常比实际用到的列宽，
    // 例如实际写了 30 列、网格是 40 列），原样内联就会在正文/预览里拖出一串
    // `|  |  |  |` 的噪音，还让"N 列"这个数字对不上原表。
    //
    // 安全边界：整列在**所有行**里都没内容、也没有图片标记时才裁；只裁尾部，
    // 中间的空列保留（那是表的真实结构，裁了会错位）。
    let mut columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    while columns > 1 {
        let index = columns - 1;
        let used = rows.iter().enumerate().any(|(row_index, row)| {
            !table_cell(row, index).is_empty()
                || image_cells.contains(&(real_rows[row_index], index))
        });
        if used {
            break;
        }
        columns -= 1;
    }
    let rows_used = rows_total.min(max_rows.max(1));

    // 单元格文本：含图片的格子补一个 `[图片]` 标记（原来是空的就直接写标记）
    let cell = |row_index: usize, index: usize| -> String {
        let text = table_cell(&rows[row_index], index);
        if image_cells.contains(&(real_rows[row_index], index)) {
            if text.is_empty() {
                "[图片]".to_string()
            } else {
                format!("{text} [图片]")
            }
        } else {
            text.to_string()
        }
    };

    let mut out = String::new();
    out.push('|');
    for index in 0..columns {
        out.push(' ');
        out.push_str(&cell(0, index));
        out.push_str(" |");
    }
    out.push('\n');
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row_index in 1..rows_used {
        out.push('|');
        for index in 0..columns {
            out.push(' ');
            out.push_str(&cell(row_index, index));
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

// ============================================================================
// 文档级原生表格（DocxXML `<table>…</table>` 片段）
// ============================================================================

/// 文档级 `<table>…</table>` 片段 → Markdown 管道表
///
/// 为什么需要：文档里**原生插入的表格**在 `--doc-format markdown` 下不做转换，
/// 正文里留的是一整段 HTML。不处理的话，`.md` 里就是一堆裸标签——列宽、底色全丢，
/// 单元格里的公式还会退回 `<latex>` 字面标签，读者看到的是"没转成功"而不是表格。
///
/// 取舍（Markdown 表达力所限）：
/// - 单元格内的行内标签转成 Markdown：`<b>`→`**`、`<em>`→`*`、`<del>`→`~~`、
///   `<code>`→`` ` ``、`<a>`→`[文字](链接)`、`<latex>`→`$…$`；`<u>` 这类没有对应
///   写法的保留原标签（照样能渲染）
/// - `colspan`/`rowspan`：Markdown 表格没有合并单元格，横向合并只在首格写内容、
///   其余补空；纵向合并在后续行的对应位置留空，保证每行列数一致、不错位
/// - `<colgroup>` 的列宽、`<th>` 的背景色无法表达，丢样式保内容
/// - 单元格里的图片交给 `image` 回调（`(src, caption) => 替换文本`），
///   便于调用方顺便把图下载到本地
///
/// 解析不出表格结构时返回 `None`，由调用方保持原文并给出提示，**不静默丢内容**。
pub fn html_table_to_markdown<F>(html: &str, mut image: F) -> Option<String>
where
    F: FnMut(&str, &str) -> String,
{
    let rows = parse_html_table_rows(html);
    if rows.is_empty() {
        return None;
    }

    // 组装网格：`None` 表示该位置还没被任何单元格占用
    let mut grid: Vec<Vec<Option<String>>> = Vec::new();
    let mut row_index = 0usize;
    let mut header_index: Option<usize> = None;
    for row in &rows {
        while grid.len() <= row_index {
            grid.push(Vec::new());
        }
        let mut col = 0usize;
        for cell in &row.cells {
            while grid[row_index].get(col).is_some() {
                col += 1;
            }
            let colspan = cell.colspan.max(1);
            let rowspan = cell.rowspan.max(1);
            let text = normalize_table_cell(&html_inline_to_markdown(&cell.html, &mut image));
            for offset in 0..colspan {
                let target = col + offset;
                ensure_cells(&mut grid[row_index], target + 1);
                grid[row_index][target] = Some(if offset == 0 {
                    text.clone()
                } else {
                    String::new()
                });
            }
            // 纵向合并：把后续行的同一列占住，避免那一行的单元格整体左移
            for down in 1..rowspan {
                let target_row = row_index + down;
                while grid.len() <= target_row {
                    grid.push(Vec::new());
                }
                ensure_cells(&mut grid[target_row], col + 1);
                grid[target_row][col] = Some(String::new());
            }
            col += colspan;
        }
        if row.is_header && header_index.is_none() {
            header_index = Some(row_index);
        }
        row_index += 1;
    }

    let columns = grid.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return None;
    }
    // 没有 `thead` 时，Markdown 也要求拿第一行当表头
    let header_index = header_index.unwrap_or(0);

    let mut out = String::new();
    push_table_row(&mut out, grid.get(header_index), columns);
    out.push('|');
    for _ in 0..columns {
        out.push_str(" --- |");
    }
    out.push('\n');
    for (index, row) in grid.iter().enumerate() {
        if index == header_index {
            continue;
        }
        push_table_row(&mut out, Some(row), columns);
    }
    Some(out)
}

fn ensure_cells(row: &mut Vec<Option<String>>, len: usize) {
    while row.len() < len {
        row.push(None);
    }
}

fn push_table_row(out: &mut String, row: Option<&Vec<Option<String>>>, columns: usize) {
    out.push('|');
    for index in 0..columns {
        let cell = row
            .and_then(|cells| cells.get(index))
            .and_then(|cell| cell.as_deref())
            .unwrap_or("");
        out.push(' ');
        out.push_str(cell);
        out.push_str(" |");
    }
    out.push('\n');
}

struct HtmlTableRow {
    is_header: bool,
    cells: Vec<HtmlTableCell>,
}

struct HtmlTableCell {
    colspan: usize,
    rowspan: usize,
    html: String,
}

fn parse_html_table_rows(html: &str) -> Vec<HtmlTableRow> {
    let thead_re = Regex::new(r"(?is)<thead\b[^>]*>.*?</thead\s*>").unwrap();
    let header_ranges: Vec<(usize, usize)> =
        thead_re.find_iter(html).map(|m| (m.start(), m.end())).collect();

    let tr_re = Regex::new(r"(?is)<tr\b[^>]*>(.*?)</tr\s*>").unwrap();
    let cell_re = Regex::new(r"(?is)<t([dh])\b([^>]*?)(?:/>|>(.*?)</t[dh]\s*>)").unwrap();

    let mut rows = Vec::new();
    for caps in tr_re.captures_iter(html) {
        let (Some(whole), Some(inner)) = (caps.get(0), caps.get(1)) else {
            continue;
        };
        let is_header = header_ranges
            .iter()
            .any(|(start, end)| whole.start() >= *start && whole.start() < *end);
        let mut cells = Vec::new();
        for cell in cell_re.captures_iter(inner.as_str()) {
            let attrs = cell.get(2).map(|m| m.as_str()).unwrap_or("");
            let body = cell.get(3).map(|m| m.as_str()).unwrap_or("");
            cells.push(HtmlTableCell {
                colspan: numeric_attr(attrs, "colspan"),
                rowspan: numeric_attr(attrs, "rowspan"),
                html: body.to_string(),
            });
        }
        if !cells.is_empty() {
            rows.push(HtmlTableRow { is_header, cells });
        }
    }
    rows
}

/// 取一个正整数属性（缺失/非法都按 1 处理）
fn numeric_attr(attrs: &str, name: &str) -> usize {
    attr_value(attrs, name)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1)
}

/// 从标签属性串里取值（支持双/单引号与无引号写法）
pub fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let needle = format!("{}=", name.to_ascii_lowercase());
    let mut search = 0usize;
    while let Some(relative) = lower[search..].find(&needle) {
        let start = search + relative;
        // 属性名必须是独立的一段：挡住 `data-href=` 被当成 `href=`
        let boundary_ok = start == 0 || attrs.as_bytes()[start - 1].is_ascii_whitespace();
        let mut value_start = start + needle.len();
        while value_start < attrs.len() && attrs.as_bytes()[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if boundary_ok {
            let rest = &attrs[value_start..];
            match rest.chars().next() {
                Some(quote) if quote == '"' || quote == '\'' => {
                    if let Some(end) = rest[1..].find(quote) {
                        return Some(decode_entities(&rest[1..1 + end]));
                    }
                }
                Some(_) => {
                    let end = rest
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(rest.len());
                    if end > 0 {
                        return Some(decode_entities(&rest[..end]));
                    }
                }
                None => {}
            }
        }
        search = start + needle.len();
    }
    None
}

/// 单元格内的行内 HTML → Markdown
fn html_inline_to_markdown<F>(html: &str, image: &mut F) -> String
where
    F: FnMut(&str, &str) -> String,
{
    let tag_re = Regex::new(r"(?is)<\s*(/?)\s*([a-zA-Z][a-zA-Z0-9_-]*)((?:\s[^<>]*?)?)\s*/?>")
        .unwrap();
    let mut out = String::new();
    let mut rest = html;
    loop {
        let Some(caps) = tag_re.captures(rest) else {
            out.push_str(&decode_entities(rest));
            break;
        };
        let Some(whole) = caps.get(0) else { break };
        out.push_str(&decode_entities(&rest[..whole.start()]));
        let closing = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = caps
            .get(2)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        let attrs = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        rest = &rest[whole.end()..];

        if closing == "/" {
            match name.as_str() {
                "b" | "strong" => out.push_str("**"),
                "em" | "i" => out.push('*'),
                "del" | "s" | "strike" => out.push_str("~~"),
                "code" => out.push('`'),
                "u" => out.push_str("</u>"),
                _ => {}
            }
            continue;
        }

        match name.as_str() {
            // 成对标签：内部内容递归转换，保证嵌套样式不丢
            "a" => {
                if let Some(start) = find_closing_tag(rest, "a") {
                    let href = attr_value(attrs, "href").unwrap_or_default();
                    out.push('[');
                    out.push_str(&html_inline_to_markdown(&rest[..start], image));
                    out.push_str("](");
                    out.push_str(&href);
                    out.push(')');
                    rest = &rest[after_tag(rest, start)..];
                }
            }
            "latex" => {
                if let Some(start) = find_closing_tag(rest, "latex") {
                    out.push('$');
                    out.push_str(&decode_entities(&rest[..start]));
                    out.push('$');
                    rest = &rest[after_tag(rest, start)..];
                }
            }
            // 图片交给调用方：它要下载落盘，还要决定写成本地相对路径
            "img" | "image" => {
                let src = attr_value(attrs, "src")
                    .or_else(|| attr_value(attrs, "token"))
                    .unwrap_or_default();
                let caption = attr_value(attrs, "caption").unwrap_or_default();
                if !src.trim().is_empty() {
                    out.push_str(&image(src.trim(), caption.trim()));
                }
            }
            "br" => out.push_str("<br>"),
            "b" | "strong" => out.push_str("**"),
            "em" | "i" => out.push('*'),
            "del" | "s" | "strike" => out.push_str("~~"),
            "code" => out.push('`'),
            "u" => out.push_str("<u>"),
            // `span` / `p` / `div` / `font` 这类只是排版容器：丢掉标签、保留内容
            _ => {}
        }
    }
    out
}

/// 找 `</name>`（大小写不敏感）里 `<` 的字节位置
fn find_closing_tag(haystack: &str, name: &str) -> Option<usize> {
    let mut search = 0usize;
    while let Some(relative) = haystack[search..].find('<') {
        let start = search + relative;
        let rest = &haystack[start + 1..];
        if let Some(after_slash) = rest.strip_prefix('/') {
            let name_len = after_slash
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(after_slash.len());
            if after_slash[..name_len].eq_ignore_ascii_case(name) {
                return Some(start);
            }
        }
        search = start + 1;
    }
    None
}

/// 标签起始位置之后、跳过 `>` 的下一个字节位置
fn after_tag(haystack: &str, start: usize) -> usize {
    match haystack[start..].find('>') {
        Some(relative) => start + relative + 1,
        None => haystack.len(),
    }
}

/// 还原 HTML 实体（`&amp;` / `&lt;` / `&gt;` / `&#xA;` / `&#39;` …）
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find('&') {
        out.push_str(&rest[..position]);
        let after = &rest[position..];
        // 实体名不会太长，限制一下搜索范围，避免把普通的 `&` 也吞掉
        let decoded = after
            .find(';')
            .filter(|semi| *semi <= 12)
            .and_then(|semi| decode_entity(&after[1..semi]).map(|ch| (ch, semi)));
        match decoded {
            Some((ch, semi)) => {
                out.push(ch);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => return Some('&'),
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        "nbsp" => return Some(' '),
        _ => {}
    }
    if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(decimal) = entity.strip_prefix('#') {
        return decimal.parse::<u32>().ok().and_then(char::from_u32);
    }
    None
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
    fn test_safe_filename_strips_control_chars_and_illegal_set() {
        // 全部 Windows 非法字符
        assert_eq!(
            safe_filename(r#"a\b/c:d*e?f"g<h>i|j"#),
            "a_b_c_d_e_f_g_h_i_j"
        );
        // 换行 / 制表 / 其他控制字符不进文件名（`<title>` 里可能解码出换行）
        assert_eq!(safe_filename("换行\n标题\t制表\u{7}响铃"), "换行_标题_制表_响铃");
        // 首尾空白与尾点
        assert_eq!(safe_filename("  点结尾..  "), "点结尾");
        // 全空白 → untitled
        assert_eq!(safe_filename("     "), "untitled");
    }

    #[test]
    fn test_safe_filename_truncation_does_not_end_on_dot_or_space() {
        // 第 100 个字符正好是点号：截断后必须再收尾，不能留下 "." 结尾
        let name = format!("{}.{}", "a".repeat(99), "b".repeat(10));
        let out = safe_filename(&name);
        assert_eq!(out, "a".repeat(99));
        assert!(!out.ends_with('.'));

        // 第 100 个字符正好是空格：同理
        let name = format!("{} {}", "a".repeat(99), "b".repeat(5));
        let out = safe_filename(&name);
        assert_eq!(out, "a".repeat(99));
        assert!(!out.ends_with(' '));

        // 恰好 100 个字符的标题原样保留
        assert_eq!(safe_filename(&"字".repeat(100)), "字".repeat(100));
    }

    #[test]
    fn test_safe_filename_reserved_names() {
        let expected = if cfg!(windows) { "_CON" } else { "CON" };
        assert_eq!(safe_filename("CON"), expected);
        assert_eq!(
            safe_filename("nul"),
            if cfg!(windows) { "_nul" } else { "nul" }
        );
    }

    /// 扩展块（callout/grid/column/bookmark/button/time/cite/figure/source）在飞书
    /// markdown 输出里的真实形态，必须逐个被"残留标签"扫描到——否则用户会以为
    /// 导出成功但其实内容被悄悄改了样。
    #[test]
    fn test_unresolved_tags_reports_extended_blocks() {
        let content = r#"<callout emoji="💡">
高亮块正文
- [ ] 高亮块内待办
</callout>

<grid>
<column width-ratio="0.500000">
左栏
</column>
</grid>

<bookmark name="书签" href="https://example.com"></bookmark>

<button action="OpenLink" src="https://example.com">按钮文字</button>

提醒：<time expire-time="1" should-notify="false"></time>时间文字

@人：<cite type="user" user-id="ou_x" user-name="lpk"></cite>

文献：<cite type="citation"><a href="https://example.com" url-type="5">文献标题</a></cite>

<figure view-type="Card"><source name="卡片附件.png" size="1" token="T"/></figure>

行内附件：<source name="行内附件.png" size="1" token="T"/>
"#;
        let tags = unresolved_tags(content);
        assert_eq!(
            tags,
            vec![
                "callout", "grid", "column", "bookmark", "button", "time", "cite", "figure",
                "source"
            ]
        );
        // 里面的 `<a>` 是常见 HTML 写法，不算"没还原"
        assert!(!tags.contains(&"a".to_string()));
    }

    #[test]
    fn test_restore_inline_server_tags_converts_bookmark_button_and_cite() {
        let content = r#"<bookmark name="飞书官网书签" href="https://www.feishu.cn"></bookmark>

<button action="OpenLink" src="https://www.feishu.cn">打开飞书官网按钮</button>

@人：<cite type="user" user-id="ou_x" user-name="lpk"></cite>

@文档：<cite doc-id="QVQ" file-type="docx" title="压测A · 符号公式排版地狱" type="doc"></cite>

引用文献：<cite type="citation"><a href="https://www.feishu.cn" url-type="5">飞书官网文献</a></cite>
"#;
        let out = restore_inline_server_tags(content);
        assert!(out.contains("[飞书官网书签](https://www.feishu.cn)"), "{out}");
        assert!(
            out.contains("[打开飞书官网按钮](https://www.feishu.cn)"),
            "{out}"
        );
        assert!(out.contains("@人：@lpk"), "{out}");
        assert!(out.contains("@文档：@压测A · 符号公式排版地狱"), "{out}");
        assert!(
            out.contains("[飞书官网文献](https://www.feishu.cn)"),
            "{out}"
        );
        // 转干净的这三个不再被算作"残留标签"
        let tags = unresolved_tags(&out);
        assert!(!tags.contains(&"bookmark".to_string()), "{tags:?}");
        assert!(!tags.contains(&"button".to_string()), "{tags:?}");
        assert!(!tags.contains(&"cite".to_string()), "{tags:?}");
    }

    #[test]
    fn test_restore_inline_server_tags_skips_fence_and_keeps_unrecoverable() {
        let content = "```html\n<bookmark name=\"示例\" href=\"https://example.com\"></bookmark>\n```\n\n<callout emoji=\"💡\">\n正文\n</callout>\n\n<source name=\"附件.png\" token=\"T\"/>\n";
        let out = restore_inline_server_tags(content);
        // 围栏代码块里的同名写法是示例，原样保留
        assert!(
            out.contains("<bookmark name=\"示例\" href=\"https://example.com\"></bookmark>"),
            "{out}"
        );
        // 不能就地还原的标签保持原文，交给文末提示兜底
        assert!(out.contains("<callout emoji=\"💡\">"), "{out}");
        assert!(out.contains("<source name=\"附件.png\" token=\"T\"/>"), "{out}");
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

    /// 网格比实际用到的列宽时，尾部全空列要裁掉（否则正文拖一串 `|  |  |`）。
    #[test]
    fn test_csv_to_markdown_table_trims_trailing_empty_columns() {
        // 网格 4 列，实际只用到前 2 列
        let table = csv_to_markdown_table("姓名,部门,,\n张三,技术部,,\n李四,产品部,,\n", 200);
        assert_eq!(table.columns, 2, "尾部全空列应裁掉：{}", table.markdown);
        assert_eq!(table.markdown, "| 姓名 | 部门 |\n| --- | --- |\n| 张三 | 技术部 |\n| 李四 | 产品部 |\n");
        // 中间的空列保留（表的真实结构，裁了会错位）
        let middle = csv_to_markdown_table("a,,c\n1,2,3\n", 200);
        assert_eq!(middle.columns, 3);
        // 只有图片标记撑着的列也不能裁
        let image_only = csv_to_markdown_table_marked("a,b\n1,\n", 200, &[(2, 1)]);
        assert_eq!(image_only.columns, 2, "{}", image_only.markdown);
    }

    #[test]
    fn test_csv_to_markdown_table_empty_input() {
        let table = csv_to_markdown_table("", 200);
        assert!(table.markdown.is_empty());
        assert_eq!(table.rows_total, 0);
        assert_eq!(table.columns, 0);
    }

    /// 含图片的格子补 `[图片]`，并且行号对齐真实表（空行被丢弃也不会标错格子）。
    #[test]
    fn test_csv_to_markdown_table_marks_image_cells_by_real_row() {
        // 第 2 行是空行（会被丢弃），图片在真实第 5 行 B 列（下标 1）
        let csv = "名称,备注\n,\n数据1,x\n数据2,y\n数据3,[占位]\n";
        let table = csv_to_markdown_table_marked(csv, 200, &[(5, 1)]);
        assert_eq!(
            table.markdown,
            "| 名称 | 备注 |\n| --- | --- |\n| 数据1 | x |\n| 数据2 | y |\n| 数据3 | [占位] [图片] |\n"
        );
    }

    /// 整行只有一张图时不能把它当空行丢掉，否则图片位置在正文里彻底消失。
    #[test]
    fn test_csv_to_markdown_table_keeps_image_only_row() {
        let csv = "名称,截图\n数据1,x\n,\n";
        let table = csv_to_markdown_table_marked(csv, 200, &[(3, 1)]);
        assert_eq!(table.rows_total, 3, "只有图片的行必须保留：{}", table.markdown);
        assert!(
            table.markdown.ends_with("|  | [图片] |\n"),
            "图片格应标成 [图片]：{}",
            table.markdown
        );
    }

    #[test]
    fn test_html_table_to_markdown_header_inline_and_spans() {
        let html = "<table><colgroup><col/><col/></colgroup>\
            <thead><tr><th vertical-align=\"top\">A</th><th vertical-align=\"top\">B</th></tr></thead>\
            <tbody>\
            <tr><td vertical-align=\"top\">1</td><td vertical-align=\"top\"><b>粗</b> &amp; <code>c</code></td></tr>\
            <tr><td colspan=\"2\" vertical-align=\"top\">跨列</td></tr>\
            <tr><td rowspan=\"2\" vertical-align=\"top\">纵跨</td><td vertical-align=\"top\">r1b</td></tr>\
            <tr><td vertical-align=\"top\"><latex>E=mc^2</latex></td></tr>\
            </tbody></table>";
        let table = html_table_to_markdown(html, |_, _| "[img]".to_string()).expect("应能解析");
        assert!(table.contains("| A | B |"), "{table}");
        assert!(table.contains("| 1 | **粗** & `c` |"), "{table}");
        assert!(table.contains("| 跨列 |  |"), "colspan 只写首格、其余留空：{table}");
        assert!(table.contains("| 纵跨 | r1b |"), "{table}");
        assert!(table.contains("|  | $E=mc^2$ |"), "rowspan 要向下占列：{table}");
    }

    #[test]
    fn test_html_table_to_markdown_escapes_pipes_and_calls_image() {
        let html = "<table><thead><tr><th>h</th></tr></thead><tbody>\
            <tr><td><img name=\"local1.png\" caption=\"截&#xA;图\" src=\"https://feishu.cn/file/TOKEN1\"/></td></tr>\
            <tr><td>a | b</td></tr></tbody></table>";
        let table =
            html_table_to_markdown(html, |src, caption| format!("[{caption}]({src})"))
                .expect("应能解析");
        assert!(
            table.contains("[截 图]"),
            "caption 要解码实体、再随单元格折叠空白：{table}"
        );
        assert!(table.contains("a \\| b"), "竖线要转义：{table}");
    }

    #[test]
    fn test_find_rich_refs_html5_block_has_data_ref() {
        let content = "# 1\n\n<html5-block alt=\"x\" data-ref=\"html5_1\"></html5-block>\n";
        let refs = find_rich_refs(content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, RichRefKind::Html5);
        assert_eq!(refs[0].data_ref.as_deref(), Some("html5_1"));
        assert!(
            refs[0].raw.contains("</html5-block>"),
            "成对闭合标签要一起吃掉：{}",
            refs[0].raw
        );
    }

    #[test]
    fn test_wrap_fence_adapts_to_backticks_and_keeps_lang() {
        assert_eq!(wrap_fence("html", "<p>hi</p>"), "```html\n<p>hi</p>\n```");
        // 内容自带三反引号时围栏必须更长，否则会被内容提前"闭合"
        let nested = wrap_fence("html", "a ``` b");
        assert!(nested.starts_with("````html\n"), "{nested}");
        assert!(nested.ends_with("\n````"), "{nested}");
        // 尾部换行由结构负责，不会多出空行
        assert_eq!(wrap_fence("html", "<p>hi</p>\n\n"), "```html\n<p>hi</p>\n```");
    }

    #[test]
    fn test_unresolved_tags_reports_only_unknown_tags() {
        let content = "<title>标题</title>\n\n<div>x</div>\n\n\
            <bookmark name=\"c\" href=\"u\"></bookmark>\n\n\
            点击 <button action=\"OpenLink\" src=\"u\">打开</button>\n\n\
            <https://www.feishu.cn>\n";
        assert_eq!(
            unresolved_tags(content),
            vec!["bookmark".to_string(), "button".to_string()]
        );
    }

    #[test]
    fn test_unresolved_tags_skips_fenced_code_and_benign_tags() {
        let content = "```\n<bookmark name=\"x\"/>\n```\n\n<bookmark name=\"y\"/>\n\n\
            <sub-page doc-id=\"d\" title=\"t\"/>\n";
        assert_eq!(unresolved_tags(content), vec!["bookmark".to_string()]);
    }

    #[test]
    fn test_unresolved_tags_needs_real_tag_shape() {
        // 作者手打的字面文本（无属性、无闭合）不该被报成"内容没还原"
        assert!(unresolved_tags("未完成 含 \\*\\* 星号 与 <tag>\n").is_empty());
        // 无属性但成对出现（`<grid>…</grid>`）算真标签
        assert_eq!(
            unresolved_tags("<grid>\n左栏\n</grid>\n"),
            vec!["grid".to_string()]
        );
        // 带属性（`<source name=…/>`）算真标签
        assert_eq!(
            unresolved_tags("<source name=\"a.png\" token=\"T\"/>\n"),
            vec!["source".to_string()]
        );
    }

    #[test]
    fn test_extract_document_title_decodes_entities() {
        assert_eq!(
            extract_document_title("<title>压测A · 符号&amp;公式</title>\n\n# 正文").as_deref(),
            Some("压测A · 符号&公式")
        );
        assert_eq!(extract_document_title("没有标题行"), None);
    }

    #[test]
    fn test_extract_document_title_falls_back_to_leading_heading() {
        // 没有 `<title>` 时，服务端把文档标题渲染成首个 `# 一级标题`
        assert_eq!(
            extract_document_title("# 压测H1 · 标题\n\n正文段落").as_deref(),
            Some("压测H1 · 标题")
        );
        // 空文档：正文只剩标题这一行
        assert_eq!(
            extract_document_title("# 压测H2 · 空文档").as_deref(),
            Some("压测H2 · 空文档")
        );
        // 有 `<title>` 时优先用它，不拿正文一级标题顶替
        assert_eq!(
            extract_document_title("<title>真标题</title>\n\n# 正文一级标题").as_deref(),
            Some("真标题")
        );
        // 首个非空行不是一级标题（二级 / 普通文本）就不猜，避免误用正文标题
        assert_eq!(extract_document_title("## 二级标题\n正文"), None);
        assert_eq!(extract_document_title("普通正文\n\n# 后面才有一级"), None);
        // 全空白
        assert_eq!(extract_document_title("   \n  "), None);
    }

    #[test]
    fn test_has_six_level_heading_ignores_fence_and_plain_hash() {
        assert!(!has_six_level_heading("# 一级\n\n#### 四级\n"));
        assert!(has_six_level_heading("###### 六级\n"));
        assert!(
            !has_six_level_heading("```markdown\n###### 示例\n```\n"),
            "围栏里的示例不算标题"
        );
        assert!(!has_six_level_heading("######不是标题\n"), "# 后没有空格不算标题");
    }

    #[test]
    fn test_fix_deep_headings_rewrites_only_beyond_six() {
        let md = "# 一级\n\n### 三级\n\n###### 深层七\n\n###### 深层八\n";
        let xml = "<h1>一级</h1><h3>三级</h3><h7>深层七</h7><h8>深层八</h8>";
        let fixed = fix_deep_headings(md, xml);
        assert!(fixed.contains("# 一级"), "{fixed}");
        assert!(fixed.contains("### 三级"), "{fixed}");
        assert!(fixed.contains("###### [h7] 深层七"), "{fixed}");
        assert!(fixed.contains("###### [h8] 深层八"), "{fixed}");
    }

    #[test]
    fn test_fix_deep_headings_aborts_when_sequences_disagree() {
        let md = "# 一级\n\n###### 深层七\n";
        // 数量对不上 → 整体放弃
        assert_eq!(fix_deep_headings(md, "<h1>一级</h1>"), md);
        // 前六级层级对不上 → 整体放弃（防止错位改坏正文）
        assert_eq!(fix_deep_headings(md, "<h2>别的</h2><h7>深层七</h7>"), md);
    }
}
