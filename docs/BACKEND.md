# LarkReader 后端功能与维护手册

> 当前状态：2026-09-05。本文是后端功能、接口和验证状态的唯一主文档；代码实现始终是最终依据。

## 1. 项目定位

LarkReader 是基于 Tauri 2 和 Rust 的飞书文档本地导出工具。后端通过固定版本 `lark-cli 1.0.93` 获取飞书内容，负责环境检测、登录、文档预览、Markdown 与图片落盘、知识库遍历、批量任务及 Sheet/Bitable 导出。

当前后端已经具备可用版本所需的核心能力。前端展示与交互不属于本文范围。

## 2. 当前功能

### 2.1 环境与认证

- 检测 Node.js、lark-cli、飞书应用配置和用户登录状态。
- 配置检测与登录检测并行执行。
- 区分未安装、未配置、未登录、版本不兼容和检测失败。
- 自动安装已验证的 `@larksuite/cli@1.0.93`。
- 支持设备码非阻塞登录，也保留阻塞登录回退路径。
- `ready` 和 `needs_refresh` 均视为有效用户登录状态。
- 后端不保存飞书密码或自行维护 token，凭据由 lark-cli 管理。

### 2.2 单文档处理

- 接受飞书 Wiki URL 或节点 token。
- 获取真实文档标题和 Markdown 正文。
- 统计 Unicode 字符数量。
- 使用 Markdown 解析器识别图片，并按源码位置精确替换本地引用。
- 相同图片 URL 去重下载。
- 图片下载并发数由 `Settings.concurrency` 控制，范围为 1–32。
- 图片失败会生成部分成功结果，不会错误报告为完全成功。
- 文档先写入临时位置，成功后再提交到正式目录。
- 同名文件自动使用 `(2)`、`(3)` 等后缀，避免覆盖。
- 文件名兼容 Windows 非法字符、保留名称和超长 Unicode 标题。

### 2.3 Wiki 知识库

- 递归读取并保留目录层级与飞书排序。
- 选择文件夹时自动包含所有后代项目。
- 选择判断使用预计算集合，避免重复遍历造成 O(n²) 查询。
- 设有循环、最大深度 64 层和最多 10,000 节点保护。
- 导出根目录使用原子创建，同名知识库不会互相覆盖。
- Doc 导出为 Markdown 和本地图片。
- Sheet 导出为 XLSX。
- Bitable 的每张数据表导出为 NDJSON。
- 不支持的节点、文档失败、特殊资源失败分别报告。
- 返回统一 `items` 列表，同时保留文档结果、特殊导出、失败和跳过分类。

### 2.4 后台任务体验

- `start_extract_wiki` 创建任务后立即返回任务 ID，Wiki 扫描也在后台执行。
- 状态：`pending`、`running`、`completed`、`failed`、`cancelled`。
- 当前实际阶段：排队、扫描 Wiki、导出文档、导出 Sheet、导出 Bitable、收尾、完成。
- 进度包含总数、完成数、当前标题、路径、项目类型、成功/失败数和错误列表。
- 返回创建、开始、完成时间，已运行时间及有足够样本时的预计剩余时间。
- 取消信号会进入文档、图片和特殊导出流程，并终止当前受控子进程。
- 取消后返回 `cancelled: true`、实际完成数及部分结果。
- 完成结果可重复读取，不会因一次查询而消失。
- 支持主动删除任务结果和列出任务历史。
- 历史持久化到本地，保留最近 24 小时且最多 100 条，按完成时间淘汰最旧记录。

### 2.5 文件、配置和可靠性

- 输出前创建临时探测文件，确认目录真实可写。
- 输出预检返回磁盘可用字节数。
- 提供只接受现有绝对目录的安全打开目录命令。
- 设置保存采用临时文件、备份和回滚流程。
- 配置损坏时备份原文件、恢复默认设置并通过状态接口返回警告。
- 默认输出目录优先使用 Documents，其次为用户目录、当前目录和系统临时目录，不生成空路径。
- 外部命令同时读取 stdout/stderr，避免大输出管道死锁。
- 快速检查超时 15 秒，普通操作 120 秒，交互登录/配置 600 秒。
- 临时网络或命令故障最多重试三次，采用 1 秒、2 秒退避；永久性错误不盲目重试。
- Tauri CSP 已启用。

## 3. Tauri 接口

| 命令 | 主要参数 | 返回 | 说明 |
|---|---|---|---|
| `check_env` | — | `EnvStatus` | 检测运行环境、配置和登录 |
| `setup_lark_cli` | — | `String` | 安装固定版本 CLI 并返回版本 |
| `init_app` | `brand`, `lang` | `String` | 初始化飞书应用配置 |
| `start_login` | — | `DeviceInfo` | 发起设备码登录 |
| `complete_login` | `device_code` | `LoginResult` | 完成设备码登录 |
| `login_feishu_blocking` | — | `LoginResult` | 阻塞式登录回退 |
| `preview_doc` | `url` | `PreviewResult` | 获取正文与图片清单，不落盘 |
| `extract_doc` | `url`, `output_dir?` | `ExtractResult` | 导出单篇文档 |
| `get_settings` | — | `Settings` | 获取设置 |
| `get_settings_status` | — | `SettingsStatus` | 获取设置及配置恢复警告 |
| `set_settings` | `settings` | — | 验证可写性并持久化设置 |
| `preflight_output_dir` | `path` | `OutputPreflight` | 检查可写性和磁盘空间 |
| `open_output_dir` | `path` | — | 使用系统文件管理器打开目录 |
| `get_wiki_tree` | `wiki_url` | `WikiNode` | 获取完整 Wiki 树 |
| `extract_wiki` | `wiki_url`, `output_dir?`, `selected_tokens?` | `WikiExtractResult` | 同步等待批量导出结果 |
| `start_extract_wiki` | 同上 | `String` | 创建后台任务并立即返回 ID |
| `get_progress` | `task_id` | `Progress` | 查询任务状态和阶段 |
| `cancel_task` | `task_id` | — | 取消活动任务 |
| `get_task_result` | `task_id` | `WikiTaskResult` | 非破坏性读取完成结果 |
| `dismiss_task_result` | `task_id` | — | 删除指定完成结果 |
| `list_task_history` | — | `Vec<WikiTaskResult>` | 查询最近任务历史 |

所有命令错误统一序列化为：

```json
{
  "code": "AUTH_REQUIRED",
  "message": "飞书未登录或 token 已过期，请重新登录",
  "retryable": false
}
```

## 4. 主要数据含义

### `WikiExtractResult`

- `output_root`：本次知识库导出的根目录。
- `total` / `completed_count`：计划项目数与实际已处理项目数。
- `success_count` / `partial_count` / `failed_count`：汇总状态。
- `cancelled`：是否被用户取消。
- `results`：文档结果。
- `exports`：Sheet/Bitable 成功结果。
- `failures`：文档失败。
- `export_failures`：特殊资源导出失败。
- `skipped`：不支持而跳过的节点。
- `items`：供调用方统一展示的全部项目结果。

### `Progress`

- `status` 表示任务生命周期，`phase` 表示当前业务步骤。
- `total` 在 Wiki 扫描完成前可以为 0；此时应显示“正在扫描”，而不是百分比。
- `estimated_remaining_seconds` 只是基于当前平均速度的估算，样本不足时为 `null`。

## 5. 模块结构

| 文件 | 职责 |
|---|---|
| `commands.rs` | Tauri 接口、设置存储、任务生命周期与历史 |
| `env.rs` | Node/CLI/配置/登录检测与初始化 |
| `lark.rs` | lark-cli 执行、超时、取消、重试和 JSON 解析 |
| `extract.rs` | 单文档预览、导出、图片下载和事务提交 |
| `wiki.rs` | Wiki 遍历、选择、目录映射与批量导出 |
| `markdown.rs` | 图片解析、URL 替换和安全文件名 |
| `models.rs` | 可序列化数据模型 |
| `error.rs` | 结构化统一错误协议 |
| `lib.rs` | 应用初始化、状态恢复和命令注册 |

## 6. 当前验证状态

最近一次完整验证：

```text
cargo fmt --all -- --check                 通过
cargo clippy --all-targets -- -D warnings  通过
cargo test --all-targets                   52 项通过
cargo build                                通过
pnpm build                                 通过
```

真实飞书账号验证已经覆盖：

- 环境、应用配置和用户身份检测；
- 真实文档标题与 Markdown 预览；
- 一篇 9,998 字、5 张图片的文档完整导出；
- 一篇 4,961 字、18 张图片的文档完整导出；
- 重复导出自动编号且不覆盖；
- 22 个顶层节点、143 篇文档的 Wiki 目录扫描；
- 空输入、无效 URL 和垃圾输入错误处理。

验证边界：当前真实样本没有充分覆盖 Sheet/Bitable，因此这两类已通过代码检查、构建和回归测试，但不能表述为与 Doc 相同强度的真实数据验证。新增任务历史、打开目录、损坏配置恢复等接口也应在正式 UI 接入后进行一次桌面端交互验收。

## 7. 构建与验证

```powershell
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build

cd ..
pnpm build
```

开发启动：

```powershell
pnpm tauri dev
```

## 8. 后续增强边界

以下是未来版本能力，不是当前核心缺陷：

- 暂停与继续；
- 应用重启后的导出断点续传；
- 基于远端更新时间或内容摘要的增量导出；
- 更准确的导出空间预估；
- 多个 Wiki 任务的全局并发调度；
- PDF、DOCX 等额外输出格式。
