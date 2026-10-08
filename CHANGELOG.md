# 更新日志

本文件记录 LarkReader 的用户可感知变更。版本号语义遵循 [SemVer](https://semver.org/lang/zh-CN/)；
每次发版由 `npm run release -- <版本>` 触发，详细发布流程见 [docs/release-and-update.md](docs/release-and-update.md)。

## [0.2.1] - 2026-10-08

### 修复

- **电子表格导出失败**（[#1](https://github.com/LPK3215/LarkReader/issues/1)）：应用后台未开通
  「导出云文档」权限点（`docs:document:export`）时，`sheets +workbook-export` 必然报
  `user lacks permission for the requested resource` —— 登录申请清单 `LOGIN_SCOPES`
  由 13 项补至 14 项，补上该权限点（等价权限点 `drive:export:readonly`）。
  文档 / 图片 / 附件 / 多维表格均走非导出 API，故此前症状表现为「只有表格导不出」
- **权限类报错难懂**：lark-cli 返回的 `missing_scopes` / `console_url` 此前被丢弃，
  用户只看到一句 `user lacks permission…` 无从下手 → 现一并透出，并新增中文权限引导文案

### 文档

- `docs/FEISHU_AUTH.md`：修正 §4.1 中 `sheets +workbook-export` 的权限映射
  （它走 drive 的「创建导出任务」API，需要的是 `docs:document:export`，而非
  `sheets:spreadsheet:read`），补 ⚠️ 说明与 §6 对应故障行；申请清单计数 13 → 14

## [0.2.0] - 2026-09-15

### 新增

- **首次启动合规确认**：引导页前置「使用前请阅读」声明——学习实践项目定位、经官方 API 以本人账号授权读取有权限内容、导出内容版权归原作者、遵守平台协议与组织规定；
  勾选同意后方可继续，确认结果本地记忆，仅首次出现

### 修复

- **本地阅读空态**：来源列表被截断、页面中部大片空白 → 改为自上而下整页滚动排布

### 文档

- README：新增使用声明（学习交流定位、内容版权归原作者、禁止传播商用、滥用责任归使用者）、非隶属声明与社区认可板块
- 新增界面截图（工作台节点树 / 本地阅读 / 设置 / 飞书终端 / 任务历史 / 运行日志）并接入 README 与使用指南
- 新增项目概览页（GitHub Pages）
- 修正扫码 / 续期 / 批注 / 默认并发等失实描述，补充引导双方式安装、设备码放入剪贴板、重新运行引导等说明

## [0.1.0] - 2026-09-06

首个公开版本。

### 新增

- **环境与认证**：自动检测 Node.js / lark-cli / 应用配置 / 登录状态（并行检测，区分 5 种异常）；
  固定安装 `@larksuite/cli@1.0.93`，支持自动 / 手动双方式安装（自动失败重试 3 次并写入日志）；
  设备码登录，设备码与授权链接可一键复制，配置向导自动弹浏览器；
  首次使用自动进入引导页，设置页可「重新运行引导」
- **单文档导出**：接受 Wiki URL 或节点 token；Markdown 正文 + 图片并发下载（1–32 并发）并本地化 URL；
  同名自动 `(2)(3)` 编号不覆盖；事务式落盘，失败不留半截文件
- **Wiki 递归导出**：保留目录层级与飞书排序；选文件夹自动含全部后代；
  循环 / 深度 ≤ 64 层 / 节点 ≤ 10,000 三重保护；同名知识库原子建目录不互覆
- **表格与数据库**：Sheet → XLSX；Bitable 每张数据表 → NDJSON + `.manifest.json` 元数据
- **文件附件**：Wiki 页面挂载的 `file` 节点按原始字节下载，18 种扩展名，字节数与上传一致
- **后台任务**：任务立即返回 ID 后台执行；8 阶段进度；协作式取消并返回部分结果；
  历史持久化（24 小时 / ≤ 100 条）
- **本地阅读**：Reader 页浏览导出目录、渲染 Markdown 与图片（data URL 内联），离线可用
- **应用内更新**：设置页一键「检查更新」；下载进度可见；公钥签名校验；
  Windows 自动安装并重启，macOS/Linux 安装后自动 relaunch
- **可靠性**：结构化错误协议（`code / message / retryable`）、输出目录预检（可写性 + 磁盘空间）、
  设置临时文件 / 备份 / 回滚、临时故障指数退避重试、运行日志页

### 测试

- 单元测试 26 项全通过；E2E 真实知识库实测 38 项成功 / 0 失败 / 0 跳过
  （产物快照见 [docs/e2e-download-case/](docs/e2e-download-case/)）

[0.1.0]: https://github.com/LPK3215/LarkReader/releases/tag/v0.1.0
[0.2.0]: https://github.com/LPK3215/LarkReader/releases/tag/v0.2.0
[0.2.1]: https://github.com/LPK3215/LarkReader/releases/tag/v0.2.1
