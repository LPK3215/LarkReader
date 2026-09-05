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

本项目提供一个飞书知识库，用于体验与端到端测试「本地阅读 / 导出」功能（覆盖文本排版、媒体、表格数据库等典型元素）：

- **知识库名称**：LarkReader-E2E-测试库
- **space_id**：`7681927602327538869`
- **Web 链接**（以你飞书实际域名为准）：
  - 空间：`https://www.feishu.cn/wiki/space/7681927602327538869`
  - 节点示例：`https://www.feishu.cn/wiki/LuQ7wEqgmiqITJkL49zcjyA0nif`
- **测试数据**：`docs/e2e-fixtures/` 保存了 Sheet（`sheet_cells.json`）、Bitable（`bitable_records.json`）的写入数据、用于上传的文档正文样例（`doc_*.md`）、媒体与附件素材，以及生成脚本（`gen_assets.py`）。
- **下载案例留档**：`docs/e2e-download-case/` 是测试库全量下载的稳定快照（含 README 说明），可作为离线样例查看导出结构与命名规则。

> 该库为测试专用，可随时删除，不影响任何正式资料。如需改空间名，请在飞书网页端操作（lark-cli 暂不提供 space 重命名命令）。
