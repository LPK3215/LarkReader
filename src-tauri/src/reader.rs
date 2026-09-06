//! 本地阅读：浏览已落盘的导出目录
//!
//! 纯本地文件系统操作，不依赖飞书登录/网络。Reader「本地阅读」页的后端基座：
//! - `list_reader_dir`：一次列一层（惰性加载），目录优先、按名排序
//! - `read_reader_md`：读取 .md 文本（供 markdown 渲染）
//! - `read_reader_binary`：读取二进制资源（图片），返回 data URL（供 <img> 内联）
//! - `find_first_reader_doc`：在目录树中查找第一篇 Markdown 文档（用于历史→阅读自动定位）
//!
//! 安全边界：
//! - 读取路径来自用户在前端显式选择/浏览的本地目录，桌面应用正常能力。
//! - 二进制读取有体积上限，防止超大文件把 IPC/内存打爆。

use std::path::{Path, PathBuf};

use base64::Engine;

use crate::error::{AppError, AppResult};
use crate::models::{ReaderBinary, ReaderEntry, ReaderEntryKind};

/// 二进制资源内联体积上限（16 MiB，base64 后约 22 MB，够所有导出图片用）
const MAX_BINARY_BYTES: u64 = 16 * 1024 * 1024;
/// Markdown 文本体积上限（8 MiB，正文通常远小于此）
const MAX_MD_BYTES: u64 = 8 * 1024 * 1024;

/// 判断扩展名是否为 Markdown
fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

/// 判断是否为导出的图片资源目录（`{stem}_images`）
///
/// 前端文件树会把它当资源目录弱化展示，避免每个文档旁都挂一个噪音目录。
pub fn is_images_dir_name(name: &str) -> bool {
    name.ends_with("_images")
}

/// 列出目录的一层子项：目录优先，同级按名称排序（忽略大小写，Windows 更自然）。
pub fn list_reader_dir(dir_path: &str) -> AppResult<Vec<ReaderEntry>> {
    let dir = PathBuf::from(dir_path);
    let meta = std::fs::metadata(&dir)?;
    if !meta.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "不是目录，无法浏览: {dir_path}"
        )));
    }

    let mut entries: Vec<ReaderEntry> = Vec::new();
    for item in std::fs::read_dir(&dir)? {
        let item = item?;
        let file_type = item.file_type()?;
        let name = item.file_name().to_string_lossy().to_string();
        let path = item.path();
        let kind = if file_type.is_dir() {
            ReaderEntryKind::Dir
        } else if is_markdown(&path) {
            ReaderEntryKind::Md
        } else {
            ReaderEntryKind::Other
        };
        let size_bytes = if file_type.is_file() {
            item.metadata().ok().map(|m| m.len())
        } else {
            None
        };
        entries.push(ReaderEntry {
            name,
            path: path.to_string_lossy().to_string(),
            kind,
            size_bytes,
        });
    }

    entries.sort_by(|a, b| {
        let a_dir = matches!(a.kind, ReaderEntryKind::Dir);
        let b_dir = matches!(b.kind, ReaderEntryKind::Dir);
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// 递归搜索条目数上限（防止超大目录把「自动打开第一篇」拖慢）
const MAX_FIRST_DOC_VISITED: usize = 4000;

/// 在目录树里找「第一篇」 Markdown 文档，返回其绝对路径。
///
/// 用于「任务历史 → 应用内阅读」：跳进阅读页后自动渲染第一篇，而不是停在空态。
/// 遍历顺序与目录树一致，且同层优先返回 Markdown（跳过 `*_images` 资源目录）；
/// 有遍历上限，搜不到或超限都返回 None，不影响阅读页正常空态。
pub fn find_first_reader_doc(root_path: &str) -> AppResult<Option<String>> {
    let root = PathBuf::from(root_path);
    let meta = std::fs::metadata(&root)?;
    if !meta.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "不是目录，无法搜索: {root_path}"
        )));
    }
    let mut visited = 0usize;
    let mut pending_dirs: Vec<PathBuf> = vec![root];
    while let Some(dir) = pending_dirs.pop() {
        let entries = list_reader_dir(&dir.to_string_lossy())?;
        let mut child_dirs = Vec::new();
        for entry in entries {
            visited += 1;
            if visited > MAX_FIRST_DOC_VISITED {
                crate::logger::warn(format!(
                    "查找第一篇文档时已遍历 {MAX_FIRST_DOC_VISITED} 个条目，放弃继续"
                ));
                return Ok(None);
            }
            match entry.kind {
                ReaderEntryKind::Md => return Ok(Some(entry.path)),
                ReaderEntryKind::Dir => {
                    if !is_images_dir_name(&entry.name) {
                        child_dirs.push(PathBuf::from(entry.path));
                    }
                }
                ReaderEntryKind::Other => {}
            }
        }
        // 当前层没有 .md：进入子目录继续找。后进先出保证贴近阅读页目录顺序。
        pending_dirs.extend(child_dirs.into_iter().rev());
    }
    Ok(None)
}

/// 读取 Markdown 文档文本。仅允许 .md/.markdown，防止误读任意文件。
pub fn read_reader_md(file_path: &str) -> AppResult<String> {
    let path = PathBuf::from(file_path);
    if !is_markdown(&path) {
        return Err(AppError::InvalidInput(format!(
            "暂不支持渲染该文件（仅 .md）：{file_path}"
        )));
    }
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() {
        return Err(AppError::InvalidInput(format!(
            "不是文件，无法阅读: {file_path}"
        )));
    }
    if meta.len() > MAX_MD_BYTES {
        return Err(AppError::Other(format!(
            "文档过大（超过 {} MiB），无法在阅读器中渲染: {file_path}",
            MAX_MD_BYTES / (1024 * 1024)
        )));
    }
    std::fs::read_to_string(&path).map_err(AppError::from)
}

/// 阅读器支持应用内纯文本预览的扩展名（等宽字体展示）
const TEXT_EXTS: &[&str] = &[
    "txt", "csv", "tsv", "json", "ndjson", "xml", "log", "yaml", "yml", "toml", "html", "htm",
    "ini", "cfg", "conf", "sql", "sh", "bat", "ps1", "py", "js", "ts", "css", "java", "c", "h",
    "cpp", "go", "rs", "rb",
];

/// 判断扩展名是否为可在应用内做纯文本预览的文件
fn is_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| TEXT_EXTS.iter().any(|t| ext.eq_ignore_ascii_case(t)))
}

/// 读取文本类附件（txt/csv/json/xml/log 等）用于应用内纯文本预览。
///
/// 与 read_reader_md 的区别：不渲染 Markdown，仅按原始文本展示；
/// 仅放行白名单扩展名，防止误读任意文件。
pub fn read_reader_text(file_path: &str) -> AppResult<String> {
    let path = PathBuf::from(file_path);
    if !is_text_file(&path) {
        return Err(AppError::InvalidInput(format!(
            "该文件类型不支持应用内文本预览: {file_path}"
        )));
    }
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() {
        return Err(AppError::InvalidInput(format!(
            "不是文件，无法阅读: {file_path}"
        )));
    }
    if meta.len() > MAX_MD_BYTES {
        return Err(AppError::Other(format!(
            "文件过大（超过 {} MiB），无法在阅读器中预览: {file_path}",
            MAX_MD_BYTES / (1024 * 1024)
        )));
    }
    std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            AppError::Other("文件不是有效的 UTF-8 文本，无法预览".to_string())
        } else {
            AppError::from(e)
        }
    })
}

/// 读取二进制资源（图片等），拼成 data URL 返回。
pub fn read_reader_binary(file_path: &str) -> AppResult<ReaderBinary> {
    let path = PathBuf::from(file_path);
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() {
        return Err(AppError::InvalidInput(format!(
            "不是文件，无法读取: {file_path}"
        )));
    }
    if meta.len() > MAX_BINARY_BYTES {
        return Err(AppError::Other(format!(
            "资源过大（超过 {} MiB），未内联: {}",
            MAX_BINARY_BYTES / (1024 * 1024),
            file_path
        )));
    }
    let bytes = std::fs::read(&path)?;
    let mime = mime_from_path(&path);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(ReaderBinary {
        data_url: format!("data:{mime};base64,{encoded}"),
    })
}

/// 从扩展名推断 MIME（仅图片资源需要；未知一律 image/octet-stream 兜底）
fn mime_from_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_markdown_extensions() {
        assert!(is_markdown(Path::new("a.md")));
        assert!(is_markdown(Path::new("a.markdown")));
        assert!(is_markdown(Path::new("01_第一章.MD")));
        assert!(!is_markdown(Path::new("a.txt")));
        assert!(!is_markdown(Path::new("a.md.txt")));
        assert!(!is_markdown(Path::new("noext")));
    }

    #[test]
    fn test_images_dir_name_rule() {
        assert!(is_images_dir_name("01_第一章_images"));
        assert!(!is_images_dir_name("01_第一章"));
        assert!(!is_images_dir_name("images"));
    }

    #[test]
    fn test_list_reader_dir_orders_dirs_first() {
        // 用临时目录验证：目录排在文件前，且排序稳定
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("bbb_dir")).unwrap();
        std::fs::create_dir(tmp.path().join("aaa_dir")).unwrap();
        std::fs::write(tmp.path().join("z.md"), "# z").unwrap();
        std::fs::write(tmp.path().join("a.md"), "# a").unwrap();
        std::fs::write(tmp.path().join("b.png"), "x").unwrap();

        let entries = list_reader_dir(&tmp.path().to_string_lossy()).unwrap();
        let kinds: Vec<ReaderEntryKind> = entries.iter().map(|e| e.kind.clone()).collect();
        assert_eq!(kinds[0], ReaderEntryKind::Dir);
        assert_eq!(kinds[1], ReaderEntryKind::Dir);
        assert_eq!(
            kinds[2..],
            [
                ReaderEntryKind::Md,
                ReaderEntryKind::Other,
                ReaderEntryKind::Md
            ]
        );
        assert_eq!(entries[0].name, "aaa_dir");
        assert_eq!(entries[1].name, "bbb_dir");
        // 文件按名称（忽略大小写）排序：a.md < b.png < z.md
        assert_eq!(entries[2].name, "a.md");
        assert_eq!(entries[3].name, "b.png");
        assert_eq!(entries[4].name, "z.md");
    }

    #[test]
    fn test_read_reader_md_rejects_non_md() {
        let tmp = tempfile::tempdir().unwrap();
        let txt = tmp.path().join("a.txt");
        std::fs::write(&txt, "hello").unwrap();
        assert!(read_reader_md(&txt.to_string_lossy()).is_err());
    }

    #[test]
    fn test_read_reader_text_whitelist_and_content() {
        let tmp = tempfile::tempdir().unwrap();
        let csv = tmp.path().join("a.csv");
        std::fs::write(&csv, "id,name\n1,张三\n").unwrap();
        let out = read_reader_text(&csv.to_string_lossy()).unwrap();
        assert!(out.contains("张三"));

        // 白名单外的扩展名拒绝
        let bin = tmp.path().join("a.xlsx");
        std::fs::write(&bin, "fake").unwrap();
        assert!(read_reader_text(&bin.to_string_lossy()).is_err());
        // 无扩展名拒绝
        let noext = tmp.path().join("noext");
        std::fs::write(&noext, "x").unwrap();
        assert!(read_reader_text(&noext.to_string_lossy()).is_err());
    }

    #[test]
    fn test_read_reader_text_rejects_non_utf8() {
        let tmp = tempfile::tempdir().unwrap();
        let txt = tmp.path().join("gbk.txt");
        // 0xFF 不是合法 UTF-8 序列，应得到友好错误而不是 panic
        std::fs::write(&txt, [0xFF, 0xFE, 0x41]).unwrap();
        let err = read_reader_text(&txt.to_string_lossy())
            .unwrap_err()
            .to_string();
        assert!(err.contains("UTF-8"), "实际错误: {err}");
    }

    #[test]
    fn test_read_reader_binary_builds_data_url() {
        let tmp = tempfile::tempdir().unwrap();
        let png = tmp.path().join("a.png");
        // 1x1 红色像素 PNG 的合法头，body 随意（只验证编码与 mime 拼接）
        std::fs::write(&png, [0x89, 0x50, 0x4e, 0x47, 0x01, 0x02, 0x03]).unwrap();
        let out = read_reader_binary(&png.to_string_lossy()).unwrap();
        assert!(out.data_url.starts_with("data:image/png;base64,"));
        assert!(out.data_url.len() > "data:image/png;base64,".len());
    }
}
