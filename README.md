# LarkReader

LarkReader 是一个基于 Tauri 2、Rust 和 Vue 的飞书文档本地导出工具。

当前后端支持环境与登录检测、单文档 Markdown/图片导出、Wiki 目录递归导出、Sheet XLSX、Bitable NDJSON、文件附件（file 节点）原样下载、后台任务进度与取消、任务历史、结构化错误和安全文件保存。

完整的功能、接口、架构、验证状态和后续边界统一记录在：

- [后端功能与维护手册](docs/BACKEND.md)

## 开发与验证

```powershell
pnpm install
pnpm tauri dev
```

完整验证命令见后端维护手册。

## 体验链接 / 测试链接

本项目提供一个飞书知识库，用于体验与端到端测试「本地阅读 / 导出」功能（覆盖文本排版、媒体、表格数据库、边界情况、附件等典型元素）：

- **知识库名称**：LarkReader-E2E-测试库
- **space_id**：`7681927602327538869`
- **Web 链接**（以你飞书实际域名为准）：
  - 空间：`https://www.feishu.cn/wiki/space/7681927602327538869`
  - 节点示例：`https://www.feishu.cn/wiki/LuQ7wEqgmiqITJkL49zcjyA0nif`
- **结构**：该库共有 8 个互不相连的顶层节点（文本排版单篇/带子文档、媒体、表格数据库、边界情况、排序验证、特殊字符标题、附件容器），全量下载产物即下方的 `docs/e2e-download-case/`。各节点 URL、规模统计与目录树详见 `docs/e2e-download-case/README.md`。

### 仓库内测试相关目录的关系

| 目录 | 定位 |
|---|---|
| `docs/e2e-fixtures/` | **素材来源**：当初往测试库上传的原材料——正文样例（`doc_headings.md` / `doc_code.md` / `doc_table.md` / `doc_images.md` / `doc_attach.md`）、Sheet 写入数据（`sheet_cells.json` + `员工数据.csv`）、Bitable 记录与字段定义（`bitable_records.json` + `field_update.json`）、媒体与附件素材（图片 `.png`、`sample_audio.wav`、`test_attachment.pdf`、`tok_01.txt`）、节点 token 映射（`tokens.json`）、生成脚本（`gen_assets.py`） |
| `e2e_download_tmp/`、`e2e_download_tmp_big/` | **测试工作目录**：回归跑批的临时输出，已被 `.gitignore` 忽略，每次测试前清空，不入库 |
| `docs/e2e-download-case/` | **下载结果案例**：E2E 测试库 8 个顶层节点全量下载的稳定快照（42 文件、18 种扩展名、38 项成功），含 `README.md` 说明，可作离线样例查看导出结构与命名规则 |

> 该库为测试专用，可随时删除，不影响任何正式资料。如需改空间名，请在飞书网页端操作（lark-cli 暂不提供 space 重命名命令）。
