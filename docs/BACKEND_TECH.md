# LarkReader 后端技术说明

> **本文档锁死后端技术栈与架构设计，作为开发依据。**
> 前端（Vue 3 + TypeScript + Tailwind CSS）等后端全部完成后再开发，不在本文档讨论范围内。

---

## 一、项目定位

LarkReader 是一个基于 Tauri 2 的飞书文档读取与导出工具。

**核心价值**：用户在本地打开应用 → 输入飞书链接 → 在线预览文档 → 一键导出（Markdown / PDF / 图片）。

**核心原理**：通过 `lark-cli` 走飞书开放平台 API 读取文档内容。这是 **读取**，不是破解——只要账号有阅读权限，API 就能返回完整正文和图片，浏览器前端层的复制限制对 API 完全无效。

---

## 二、核心约束（不可妥协）

| # | 约束 | 原因 | 影响 |
|---|---|---|---|
| 1 | **必须依赖 lark-cli** | lark-cli 解决了飞书 OAuth 认证 + API 封装 + token 管理，不可替代；去掉它等于重写全部认证逻辑 | 应用必须运行在用户本地 |
| 2 | **必须有界面** | 目标用户不只是开发者，命令行门槛太高 | 不能只有 CLI，必须有 GUI |
| 3 | **双击打开** | 下载 → 安装 → 双击 → 用，不需要终端 | 桌面应用，不是 Web 服务 |
| 4 | **跨平台** | 飞书用户覆盖 Windows + macOS | 至少支持 Windows + macOS |

四条约束推导结论：**Tauri 2（Rust 后端 + Web 前端，打包为 .exe / .dmg）**。

---

## 三、后端技术栈（锁死）

| 层 | 技术 | 版本要求 | 锁死理由 |
|---|---|---|---|
| 桌面框架 | **Tauri 2** | 2.x 最新 | 体积小（~5MB），Rust 后端直接调系统命令，跨平台 |
| 后端语言 | **Rust** | 1.96+ | 直接调用 lark-cli（`Command::new`）、文件操作、JSON 解析 |
| 异步运行时 | **tokio** | 1.x | 图片并发下载、异步命令执行 |
| JSON 处理 | **serde + serde_json** | 1.x | 解析 lark-cli 返回的 JSON 响应 |
| 正则引擎 | **regex** | 1.x | 从 Markdown 中提取图片 URL 和 file_token |
| HTTP 请求 | **reqwest** | 0.12+ | 预留直接调飞书 API 的 fallback（不依赖 lark-cli 时的备选方案） |
| Markdown 解析 | **pulldown-cmark** | 0.10+ | 比 MVP 的正则更健壮，解析 Markdown 中的图片引用、代码块等 |
| 文件操作 | **std::fs + tokio::fs** | stdlib | 保存文档、图片，创建目录结构 |
| 错误处理 | **thiserror + anyhow** | 1.x | 库错误用 thiserror 精确定义，应用层用 anyhow 统一处理 |
| 日志 | **tracing + tracing-subscriber** | 0.1x | 结构化日志，调试和问题定位 |

### 不使用的技术（明确排除）

| 排除项 | 原因 |
|---|---|
| Electron | 体积 80MB 太大，内存占用高（~150MB vs Tauri ~30MB） |
| Python 后端 | 违反约束 3（不能双击打开），打包体验差 |
| 纯 CLI | 违反约束 2（必须有界面） |
| WinForms/WPF | 违反约束 4（仅 Windows） |
| PyQt | 跨平台但打包体积大、体验差 |

---

## 四、后端模块架构

```
src-tauri/src/
├── main.rs              # Tauri 入口，注册命令
├── commands.rs          # Tauri 命令注册，暴露给前端的接口
├── lark.rs              # lark-cli 调用封装（底层入口）
├── extract.rs           # 文档提取 + 图片下载 + 本地路径替换
├── wiki.rs              # Wiki 节点树递归遍历
├── env.rs               # 环境检测（Node.js / lark-cli / 登录状态）
├── markdown.rs          # Markdown 解析与图片引用处理
├── error.rs             # 统一错误类型定义
└── models.rs            # 数据结构定义（请求/响应/配置）
```

### 模块职责

| 模块 | 职责 | 对应 MVP（Python） | 对应 lark-cli 命令 |
|---|---|---|---|
| `lark.rs` | lark-cli 调用的底层封装，处理环境变量清理、命令构造、输出捕获 | `subprocess.run(["lark-cli", ...])` | 所有 lark-cli 调用 |
| `extract.rs` | 文档提取主流程：获取正文 → 解析图片 → 下载图片 → 替换本地路径 | `fetch_doc()` + `preview_image()` + `main()` | `docs +fetch` + `docs +media-preview` |
| `wiki.rs` | Wiki 知识库目录树递归遍历 | MVP 中未实现（手动传列表） | `wiki +node-get` + `wiki +node-list` |
| `env.rs` | 环境检测与配置引导 | `setup.ps1` | `lark-cli whoami` / `lark-cli config show` |
| `markdown.rs` | Markdown 解析，提取/替换图片引用 | `re.findall(r'!\[...]', content)` | — |
| `commands.rs` | Tauri 命令注册层，暴露给前端 | 命令行参数入口 | — |
| `error.rs` | 统一错误类型 | Python 异常 | — |
| `models.rs` | 数据结构定义 | Python dict | — |

---

## 五、核心调用链

### 5.1 单文档提取流程

```
前端 invoke("extract_doc", { url, output_dir })
    │
    ├── env.rs: 清除 HERMES_HOME / OPENCLAW_HOME / LARK_CHANNEL 环境变量
    │
    ├── lark.rs: 构造并执行命令
    │   lark-cli docs +fetch --doc <url> --doc-format markdown --as user
    │
    ├── 解析 JSON 响应 → 提取 data.document.content（Markdown 正文）
    │
    ├── markdown.rs: 解析 Markdown，提取所有图片引用
    │   匹配 ![描述](https://feishu.cn/file/<token>) → 取 file_token
    │
    ├── extract.rs: 逐张（或并发）下载图片
    │   lark-cli docs +media-preview --token <token> --output <path> --as user
    │   下载到 <文档名>_images/img_01.png 等
    │
    ├── markdown.rs: 将 Markdown 中的远程 URL 替换为本地相对路径
    │   https://feishu.cn/file/<token> → <文档名>_images/img_01.png
    │
    └── 返回结果给前端
        { status: "success", char_count, image_count, filepath }
```

### 5.2 Wiki 知识库遍历流程

```
前端 invoke("extract_wiki", { wiki_url, output_dir })
    │
    ├── wiki.rs: 获取根节点信息
    │   lark-cli wiki +node-get --node-token <token> --as user --format json
    │   → 获取 space_id、node_token、has_child
    │
    ├── wiki.rs: 递归遍历子节点
    │   lark-cli wiki +node-list --space-id <space_id> \
    │     --parent-node-token <node_token> --page-all --as user --format json
    │   → 对 has_child=true 的节点继续递归，直到遍历完整棵树
    │
    ├── 得到完整的节点列表 [{ node_token, title, has_child, ... }]
    │
    └── 对每个文档节点，调用 5.1 的单文档提取流程
```

---

## 六、lark-cli 命令清单

后端通过 `Command::new("lark-cli")` 调用以下命令：

| 用途 | 命令 | 参数 | 说明 |
|---|---|---|---|
| 获取文档正文 | `docs +fetch` | `--doc <wiki_url>` `--doc-format markdown` `--as user` | 返回 JSON，正文在 `data.document.content` |
| 下载文档图片 | `docs +media-preview` | `--token <file_token>` `--output <path>` `--as user` | 只需阅读权限（`media-download` 需导出权限，不用） |
| 获取 Wiki 节点 | `wiki +node-get` | `--node-token <token>` `--as user` `--format json` | 获取 space_id、has_child 等节点信息 |
| 列出子节点 | `wiki +node-list` | `--space-id <id>` `--parent-node-token <token>` `--page-all` `--as user` `--format json` | 列出某节点下所有子节点 |
| 检测登录状态 | `whoami` | — | 返回 identity / tokenStatus |
| 查看应用配置 | `config show` | — | 返回 appId / brand |
| 初始化应用 | `config init --new` | `--brand feishu` `--lang zh` | 首次配置，会打开浏览器 |
| 登录授权 | `auth login` | `--domain docs --domain drive --domain wiki` | 浏览器扫码登录 |

---

## 七、关键问题与解决方案

| # | 问题 | MVP 现状 | 后端解决方案 |
|---|---|---|---|
| 1 | HERMES_HOME 环境变量干扰 | 手动 `$env:HERMES_HOME = $null` | Rust 中 `Command::new().env_remove("HERMES_HOME").env_remove("OPENCLAW_HOME").env_remove("LARK_CHANNEL")` |
| 2 | 图片正则漏匹配特殊字符 | `re.findall(r'!\[([^\]]*)\]\(([^)]+)\)')` | 用 `pulldown-cmark` 解析 Markdown AST，精确提取图片节点 |
| 3 | Windows 编码问题 | Python 默认 UTF-8 没问题，但 lark-cli 输出可能乱码 | Rust 统一接收 `Vec<u8>` → 转 UTF-8，处理 BOM 头 |
| 4 | 无错误重试 | 失败直接跳过 | 指数退避重试 3 次（1s → 2s → 4s） |
| 5 | 无并发下载 | 逐张下载，174 张很慢 | `tokio::spawn` + `tokio::sync::Semaphore` 限制并发数（默认 5） |
| 6 | 无断点续传 | 中断后从头开始 | 本地 JSON 文件记录进度（已完成的文档/图片列表） |
| 7 | node_token vs obj_token | 推荐用 wiki URL 格式 | 统一构造 `https://xxx.feishu.cn/wiki/<node_token>` 格式 |
| 8 | 图片格式识别 | lark-cli 自动识别 | 读取 `saved_path` 的扩展名，按序号重命名 |

---

## 八、后端暴露给前端的接口（Tauri Commands）

| 命令 | 参数 | 返回 | 功能 |
|---|---|---|---|
| `check_env` | — | `EnvStatus { node, lark_cli, logged_in, app_id, user }` | 检测 Node.js / lark-cli / 飞书登录状态 |
| `login_feishu` | — | `Result<(), Error>` | 引导用户浏览器登录飞书 |
| `setup_lark_cli` | — | `Result<(), Error>` | 自动安装 lark-cli（npm install -g） |
| `extract_doc` | `{ url, output_dir }` | `ExtractResult { title, char_count, image_count, filepath }` | 提取单篇文档（正文 + 图片） |
| `extract_wiki` | `{ wiki_url, output_dir }` | `WikiExtractResult { total, success, failed, nodes }` | 递归遍历并提取整个知识库 |
| `get_wiki_tree` | `{ wiki_url }` | `WikiNode { node_token, title, has_child, children }` | 获取知识库目录树（供前端展示） |
| `preview_doc` | `{ url }` | `PreviewResult { title, content_markdown }` | 获取文档正文用于前端预览（不下载图片） |
| `get_progress` | `{ task_id }` | `Progress { total, done, current, errors }` | 查询批量提取进度 |

---

## 九、数据结构定义

### 9.1 环境状态

```rust
pub struct EnvStatus {
    pub node_installed: bool,
    pub node_version: Option<String>,
    pub lark_cli_installed: bool,
    pub lark_cli_version: Option<String>,
    pub logged_in: bool,
    pub app_id: Option<String>,
    pub user: Option<String>,
}
```

### 9.2 提取结果

```rust
pub struct ExtractResult {
    pub title: String,
    pub filename: String,
    pub char_count: usize,
    pub image_count: usize,
    pub images_downloaded: usize,
    pub images_failed: usize,
    pub filepath: String,
    pub status: ExtractStatus,  // Success | Partial | Failed
    pub errors: Vec<String>,
}
```

### 9.3 Wiki 节点树

```rust
pub struct WikiNode {
    pub node_token: String,
    pub title: String,
    pub obj_type: String,           // doc / sheet / bitable / folder ...
    pub has_child: bool,
    pub obj_token: Option<String>,  // 文档的实际 token（用于 docs +fetch）
    pub position: usize,            // 在同级节点中的排序位置（飞书返回的顺序）
    pub depth: usize,              // 在树中的深度（根节点 depth=0）
    pub children: Vec<WikiNode>,    // 子节点，递归结构
}
```

> **关键**：`position` 和 `depth` 字段确保本地阅读器的目录结构和顺序与飞书浏览器中一致。详见 FEATURES.md 3.2.1 节。

### 9.4 批量提取进度

```rust
pub struct Progress {
    pub task_id: String,
    pub total: usize,
    pub done: usize,
    pub current_doc: Option<String>,
    pub success_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}
```

---

## 十、开发顺序

| 阶段 | 内容 | 验收标准 |
|---|---|---|
| 1 | `error.rs` + `models.rs`：定义错误类型和数据结构 | 编译通过 |
| 2 | `lark.rs`：lark-cli 调用封装（含环境变量清理） | 能成功调用 `lark-cli whoami` 并解析返回 |
| 3 | `env.rs`：环境检测 | `check_env` 命令返回完整环境状态 |
| 4 | `extract.rs` + `markdown.rs`：单文档提取 | `extract_doc` 命令成功提取文档 + 图片 |
| 5 | `wiki.rs`：Wiki 树遍历 | `get_wiki_tree` 返回完整目录树 |
| 6 | 批量提取 + 进度追踪 | `extract_wiki` 完整提取整个知识库 |
| 7 | 并发下载 + 错误重试 | 图片下载速度提升，失败自动重试 |
| 8 | 断点续传 | 中断后可恢复 |
| 9 | `preview_doc`：前端预览接口 | 返回 Markdown 供前端渲染 |
| 10 | `commands.rs` + `main.rs`：Tauri 命令注册 | 全部命令可通过 Tauri invoke 调用 |

---

## 十一、项目初始化

```bash
# 使用 Tauri 2 官方脚手架创建项目
npm create tauri-app@latest
# 项目名: LarkReader
# 前端: Vue + TypeScript（后续开发，现在只搭骨架）
# 包管理: pnpm
```

### Cargo 依赖（`src-tauri/Cargo.toml`）

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
regex = "1"
reqwest = { version = "0.12", features = ["json"] }
pulldown-cmark = "0.10"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## 十二、与 MVP 的对应关系

| MVP（Python） | LarkReader（Rust） | 映射说明 |
|---|---|---|
| `extract_generic.py` | `extract.rs` + `markdown.rs` | 核心提取逻辑，Python → Rust |
| `setup.ps1` | `env.rs` + 前端配置向导 | 环境检测与引导，脚本 → Rust + 界面 |
| `subprocess.run(["lark-cli", ...])` | `Command::new("lark-cli")` | Python subprocess → Rust std::process |
| `json.loads(result.stdout)` | `serde_json::from_slice()` | Python json → Rust serde_json |
| `re.findall(r'!\[...]', content)` | `pulldown-cmark` AST 解析 | Python re → Rust Markdown 解析器（更健壮） |
| `os.makedirs()` / `open()` | `std::fs::create_dir_all()` / `File::create()` | 文件操作 |
| `$env:HERMES_HOME = $null` | `cmd.env_remove("HERMES_HOME")` | PowerShell → Rust |
| 命令行参数 | Tauri command invoke | CLI → GUI |
| `print()` 日志 | `tracing` + 前端日志区 | 终端 → 结构化日志 + 界面 |
| 无并发 | `tokio::spawn` + `Semaphore` | 逐张 → 并发下载 |
| 无重试 | 指数退避重试 3 次 | 失败跳过 → 自动重试 |
| 无续传 | 本地 JSON 记录进度 | 从头开始 → 断点续传 |

---

## 十三、已知风险

| 风险 | 影响 | 应对 |
|---|---|---|
| lark-cli 版本更新导致命令变化 | API 调用失败 | 锁定版本 + 兼容层 |
| lark-cli 停止维护 | 无法调用飞书 API | 预留直接调 API 的 fallback（reqwest） |
| npm 全局安装权限问题 | 用户无法安装 lark-cli | 提供 npx 方式或本地安装 |
| 企业管理员关闭 OpenAPI | 所有调用失败 | 无法绕过，提示用户联系管理员 |
| token 过期 | 需重新登录 | 应用检测过期 → 引导重新登录 |
| API 频率限制 | 批量提取被限流 | 加延迟 + 重试机制 |
