# 更新日志

本文件记录 LarkReader 的用户可感知变更。版本号语义遵循 [SemVer](https://semver.org/lang/zh-CN/)；
每次发版由 `npm run release -- <版本>` 触发，详细发布流程见 [docs/release-and-update.md](docs/release-and-update.md)。

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
