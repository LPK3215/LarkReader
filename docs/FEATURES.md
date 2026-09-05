# LarkReader 功能规划文档

> **本文档定义 LarkReader 的完整功能边界与开发优先级。**
> 后端优先开发，前端等后端完成后再做。功能按版本迭代，不是一次性全部实现。

---

## 一、产品核心流程

```
用户双击打开应用
  → 输入飞书文档/Wiki 链接
  → 应用读取文档内容（通过 lark-cli API）
  → 前端渲染预览（带样式、图片直接显示）
  → 用户点击「导出」
  → 保存为本地 Markdown 文件（图片自动下载到本地）
```

---

## 二、功能清单

### P0 — 核心功能（第一版必须完成）

| # | 功能 | 模块 | 说明 |
|---|---|---|---|
| 1 | 环境检测 | `env.rs` | 检测 Node.js、lark-cli 是否安装，飞书是否已登录 |
| 2 | 环境引导 | `env.rs` | 未安装 lark-cli 时自动安装，未登录时引导浏览器授权 |
| 3 | 单文档读取 | `extract.rs` | 输入飞书文档/Wiki 链接，获取 Markdown 正文 |
| 4 | 单文档预览 | `extract.rs` + 前端 | 返回 Markdown 正文，前端渲染为带样式的富文本预览 |
| 5 | 源码切换查看 | 前端 | 预览界面可切换「渲染预览 / 原始 Markdown 源码」 |
| 6 | 图片下载 | `extract.rs` | 从 Markdown 中提取图片引用，用 `media-preview` 下载到本地 |
| 7 | 图片路径替换 | `markdown.rs` | 将 Markdown 中的远程 URL 替换为本地相对路径 |
| 8 | Markdown 导出 | `extract.rs` | 保存为 .md 文件 + 同名图片目录 |
| 9 | HERMES_HOME 清理 | `lark.rs` | 每次调用 lark-cli 前清除干扰环境变量 |
| 10 | 默认输出路径 | `models.rs` | 应用提供默认输出目录（如 `~/Documents/LarkReader/`），也支持用户自定义 |

### P1 — 增强功能（第二版）

| # | 功能 | 模块 | 说明 |
|---|---|---|---|
| 11 | Wiki 知识库遍历 | `wiki.rs` | 输入知识库根节点链接，递归遍历整棵目录树 |
| 12 | 批量提取（全自动） | `wiki.rs` + `extract.rs` | 默认模式：一键提取知识库全部文档 |
| 13 | 批量提取（树形勾选） | `wiki.rs` + 前端 | 返回目录树结构，前端展示，用户勾选要提取的节点 |
| 14 | 批量提取进度 | `models.rs` + 前端 | 实时显示提取进度（已完成/总数、当前文档、成功/失败计数） |
| 15 | 图片并发下载 | `extract.rs` | tokio 并发下载图片，可配置并发数（默认 5） |
| 16 | 错误重试 | `lark.rs` | 网络失败自动指数退避重试 3 次（1s → 2s → 4s） |
| 17 | 图片下载开关 | `models.rs` | 设置中可关闭图片下载（有些用户只要文本） |

### P2 — 体验优化（第三版）

| # | 功能 | 模块 | 说明 |
|---|---|---|---|
| 18 | 断点续传 | `models.rs` | 中断后可恢复，本地 JSON 记录已完成/未完成进度 |
| 19 | 拖拽链接 | 前端 | 直接拖拽飞书链接到窗口即可提取 |
| 20 | 提取历史记录 | `models.rs` + 前端 | 保留提取过的文档/知识库列表，可快速重新打开 |
| 21 | 收藏夹 | `models.rs` + 前端 | 常读的文档链接保存下来，下次直接点开 |
| 22 | 自定义飞书域名 | `models.rs` | 不同企业的飞书域名可能不同，支持配置 |
| 23 | 导出为 PDF | `extract.rs` | 将渲染后的文档导出为 PDF |
| 24 | 导出为 HTML | `extract.rs` | 导出为自包含 HTML 单文件（图片 base64 内嵌） |

### P3 — 远期规划

| # | 功能 | 模块 | 说明 |
|---|---|---|---|
| 25 | 导出为 Word | `extract.rs` | .docx 格式 |
| 26 | API fallback | `lark.rs` | lark-cli 不可用时，直接用 reqwest 调飞书 API |
| 27 | GitHub Release | CI/CD | GitHub Actions 自动打包 .exe / .dmg |

---

## 三、功能详情

### 3.1 环境检测与引导

**检测链**：
```
检测 Node.js → 检测 lark-cli → 检测飞书登录状态
```

| 检测项 | 命令 | 判定 |
|---|---|---|
| Node.js | `node --version` | 返回版本号则已安装 |
| lark-cli | `lark-cli --version` | 返回版本号则已安装 |
| 飞书登录 | `lark-cli whoami` | `tokenStatus: "ready"` 则已登录 |
| 应用配置 | `lark-cli config show` | 返回 appId 则已配置 |

**引导流程**：
```
Node.js 未安装 → 提示下载安装（提供链接 https://nodejs.org/）
lark-cli 未安装 → 自动执行 npm install -g @larksuite/cli
飞书未配置 → 引导 lark-cli config init --new
飞书未登录 → 引导 lark-cli auth login
```

### 3.2 输入链接类型支持

| 类型 | 示例 | 处理方式 |
|---|---|---|
| 单篇文档 | `https://xxx.feishu.cn/wiki/<node_token>` | 直接调 `docs +fetch` |
| 知识库根节点 | `https://xxx.feishu.cn/wiki/<root_token>` | 递归遍历整棵树 |
| 知识库目录节点 | `https://xxx.feishu.cn/wiki/<folder_token>` | 遍历该节点子树 |

应用自动识别链接类型：先用 `wiki +node-get` 获取节点信息，根据 `has_child` 判断是文档还是目录。

### 3.2.1 目录结构保持（核心需求）

**LarkReader 是一个本地阅读器，不是简单的文件下载器。** 飞书知识库本身是有完整目录树的——根节点下有子目录，子目录下有文档和更深的子目录，文档之间有严格的顺序。提取到本地后，必须完整还原这个层级结构和顺序。

**后端职责**：
- 遍历 Wiki 树时，必须保留每个节点的 **位置信息**：父节点是谁、兄弟节点的顺序、在树中的深度
- 返回给前端的数据结构必须是 **完整的树**，不能是扁平列表
- 导出时，本地文件夹结构必须映射飞书的目录层级

**数据结构**：

```rust
pub struct WikiNode {
    pub node_token: String,
    pub title: String,
    pub obj_type: String,        // doc / sheet / bitable / folder ...
    pub has_child: bool,
    pub obj_token: Option<String>,  // 文档的实际 token（用于 docs +fetch）
    pub position: usize,            // 在同级节点中的排序位置（飞书返回的顺序）
    pub depth: usize,              // 在树中的深度（根节点 depth=0）
    pub children: Vec<WikiNode>,    // 子节点，递归结构
}
```

**本地导出结构（还原飞书目录层级）**：

```
<输出目录>/
├── 根节点标题/                         # depth=0
│   ├── 01_第一章子目录/                # depth=1，position=0
│   │   ├── 01_第一篇文档.md            # depth=2，position=0
│   │   ├── 01_第一篇文档_images/
│   │   │   └── img_01.png
│   │   ├── 02_第二篇文档.md            # depth=2，position=1
│   │   └── 02_第二篇文档_images/
│   │       └── ...
│   ├── 02_第二章子目录/                # depth=1，position=1
│   │   ├── 01_某篇文档.md
│   │   └── ...
│   ├── 01_根目录下的直接文档.md        # depth=1，position=2
│   └── 02_根目录下的另一篇文档.md      # depth=1，position=3
```

**文件命名规则**：
- 目录和文件名前加 `{position:02d}_` 前缀，确保本地排序与飞书一致
- 文件名过滤非法字符（`\ / : * ? " < > |`），替换为 `_`
- 文件名截断到 100 字符以内

**关键细节**：
- 飞书 `wiki +node-list` 返回的节点列表本身就是按顺序排好的，后端只需保留这个顺序
- 同级节点中，文档和子目录可能混排，必须保持原始顺序
- 前端展示目录树时，也必须按 `position` 排序，与飞书浏览器中看到的层级和顺序一致
- 导出时用户可选择性导出：全选一键导出、或勾选部分节点导出（但仍保持原有目录结构）

### 3.3 预览功能

**两种视图，可切换**：

| 视图 | 说明 | 数据来源 |
|---|---|---|
| 渲染预览 | 富文本展示，带标题样式、代码高亮、图片直接显示 | Markdown → 前端渲染 |
| 源码查看 | 显示原始 Markdown 文本 | API 返回的原始内容 |

**预览时图片处理**：预览阶段直接用远程 URL 显示（不下载），导出时才下载到本地。

### 3.4 导出功能

**第一版只支持 Markdown 导出**，后续逐步加其他格式。

| 格式 | 版本 | 说明 |
|---|---|---|
| Markdown (.md) | P0 | 正文 + 本地图片目录，图片引用为相对路径 |
| PDF | P2 | 渲染后的文档，带样式 |
| HTML | P2 | 自包含单文件，图片 base64 内嵌 |
| Word (.docx) | P3 | 远期规划 |

**输出目录**：
- 默认路径：`~/Documents/LarkReader/`（Windows 上为 `C:\Users\<user>\Documents\LarkReader\`）
- 用户可在设置中修改默认输出路径
- 每次导出时也可临时选择输出位置

**单文档导出结构**（无目录层级）：
```
<输出目录>/
├── 文档标题A.md
├── 文档标题A_images/
│   ├── img_01.png
│   ├── img_02.webp
│   └── ...
├── 文档标题B.md
├── 文档标题B_images/
│   └── ...
```

**知识库导出结构**（还原飞书目录层级，见 3.2.1 节）：
```
<输出目录>/
└── 知识库名称/
    ├── 01_子目录A/
    │   ├── 01_文档1.md
    │   ├── 01_文档1_images/
    │   └── 02_文档2.md
    ├── 02_子目录B/
    │   └── ...
    └── 01_根目录直接文档.md
```

> **核心原则**：本地阅读器的浏览体验必须与飞书浏览器中看到的目录结构、层级、顺序一致。

### 3.5 批量提取与目录树展示

**核心原则**：LarkReader 是本地阅读器，目录树展示和导出必须还原飞书的层级结构和顺序。详见 3.2.1 节。

**两种模式**：

| 模式 | 触发方式 | 流程 |
|---|---|---|
| 全自动（默认） | 粘贴知识库链接 → 点「提取全部」 | 递归遍历目录树 → 按目录顺序逐个提取所有文档 |
| 树形勾选 | 粘贴知识库链接 → 点「选择文档」 → 展示目录树 | 前端展示树形结构（保留层级和顺序）→ 用户勾选 → 只提取勾选的文档（勾选父目录则包含其全部子节点） |

**树形勾选的关键行为**：
- 勾选一个目录节点 → 自动选中该目录下所有子文档和子目录
- 取消勾选一个目录节点 → 自动取消其下所有子节点
- 导出时仍然保持原有目录结构，只导出勾选的部分
- 前端树形展示必须按飞书原始顺序排列

**进度展示**：
- 总文档数 / 已完成数
- 当前正在提取的文档标题和所在目录路径
- 成功数 / 失败数
- 失败列表（可查看错误详情）
- 进度按目录树顺序推进（不是随机的）

### 3.6 图片处理

| 阶段 | 行为 | 说明 |
|---|---|---|
| 预览时 | 不下载，直接用远程 URL 显示 | 快速预览，不产生本地文件 |
| 导出时 | 下载到本地 `<文档名>_images/` 目录 | 用 `media-preview`（只需阅读权限） |
| 设置开关 | 可关闭图片下载 | 有些用户只要文本 |

**图片下载方式**：`lark-cli docs +media-preview --token <file_token> --output <path> --as user`

**并发下载**：tokio + Semaphore 限制并发数（默认 5，可配置）

### 3.7 设置项

| 设置项 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| 输出目录 | 路径选择 | `~/Documents/LarkReader/` | 导出文件的保存位置 |
| 并发下载数 | 数字 | 5 | 图片并发下载数量，控制带宽占用 |
| 图片下载开关 | 布尔 | true | 是否下载文档中的图片 |

### 3.8 历史记录与收藏夹（P2 规划，第一版不做）

| 功能 | 说明 | 优先级 |
|---|---|---|
| 提取历史 | 保留提取过的文档/知识库列表，可重新打开 | P2 |
| 收藏夹 | 常读的文档链接保存下来，下次直接点开 | P2 |

> 第一版先不做，但数据结构预留扩展空间。

---

## 四、后端模块与功能映射

| 模块 | 负责功能 | 暴露的 Tauri Command |
|---|---|---|
| `env.rs` | 环境检测、环境引导 | `check_env`, `setup_lark_cli`, `login_feishu` |
| `lark.rs` | lark-cli 调用封装、HERMES_HOME 清理 | （内部模块，不直接暴露） |
| `extract.rs` | 单文档提取、图片下载、Markdown 导出 | `preview_doc`, `extract_doc` |
| `wiki.rs` | Wiki 树遍历、批量提取 | `get_wiki_tree`, `extract_wiki` |
| `markdown.rs` | Markdown 解析、图片引用提取与替换 | （内部模块，不直接暴露） |
| `models.rs` | 数据结构、设置管理、进度管理 | `get_settings`, `set_settings`, `get_progress` |
| `error.rs` | 统一错误类型 | （内部模块，不直接暴露） |
| `commands.rs` | Tauri 命令注册 | — |

---

## 五、Tauri Command 接口定义

### P0 接口（第一版必须实现）

| 命令 | 参数 | 返回 | 功能 |
|---|---|---|---|
| `check_env` | — | `EnvStatus` | 检测 Node.js / lark-cli / 登录状态 |
| `setup_lark_cli` | — | `Result<(), AppError>` | 自动安装 lark-cli |
| `login_feishu` | — | `Result<(), AppError>` | 引导浏览器登录飞书 |
| `preview_doc` | `{ url }` | `PreviewResult { title, content_markdown, images: Vec<ImageRef> }` | 获取文档正文供前端预览（不下载图片） |
| `extract_doc` | `{ url, output_dir? }` | `ExtractResult` | 提取单篇文档（正文 + 图片下载 + 保存 .md） |
| `get_settings` | — | `Settings` | 获取当前设置 |
| `set_settings` | `Settings` | `Result<(), AppError>` | 保存设置 |

### P1 接口（第二版）

| 命令 | 参数 | 返回 | 功能 |
|---|---|---|---|
| `get_wiki_tree` | `{ wiki_url }` | `WikiNode` | 获取知识库目录树 |
| `extract_wiki` | `{ wiki_url, output_dir?, node_tokens? }` | `WikiExtractResult` | 批量提取（node_tokens 为空则全部提取，非空则只提取指定节点） |
| `get_progress` | `{ task_id }` | `Progress` | 查询批量提取进度 |

---

## 六、开发路线（按版本迭代）

### v0.1 — 核心读取与导出（对应 P0）

- [ ] `error.rs` + `models.rs`：错误类型、数据结构
- [ ] `lark.rs`：lark-cli 调用封装（HERMES_HOME 清理）
- [ ] `env.rs`：环境检测与引导
- [ ] `markdown.rs`：Markdown 解析、图片引用提取
- [ ] `extract.rs`：单文档提取 + 图片下载 + 路径替换
- [ ] `commands.rs` + `main.rs`：Tauri 命令注册
- [ ] `preview_doc` + `extract_doc` 接口可用
- [ ] 设置管理（输出目录、图片开关）

### v0.2 — 知识库批量提取（对应 P1）

- [ ] `wiki.rs`：Wiki 树递归遍历
- [ ] `get_wiki_tree` + `extract_wiki` 接口
- [ ] 批量提取进度追踪
- [ ] 图片并发下载
- [ ] 错误重试机制

### v0.3 — 体验优化（对应 P2）

- [ ] 断点续传
- [ ] 提取历史记录
- [ ] 收藏夹
- [ ] 自定义飞书域名
- [ ] 导出 PDF / HTML

### v0.4 — 远期（对应 P3）

- [ ] 导出 Word
- [ ] API fallback（不依赖 lark-cli）
- [ ] GitHub Actions 自动打包

---

## 七、用户问题确认记录

以下为需求确认结论，作为开发依据：

| 问题 | 用户回答 | 结论 |
|---|---|---|
| 支持哪些链接类型？ | 全部支持 | 单篇文档、知识库根节点、目录节点 |
| 预览做到什么程度？ | 渲染预览 + 可切换源码查看 | 两种视图都有，用户可切换 |
| 导出哪些格式？ | 第一步只有 Markdown | Markdown 优先，PDF/HTML/Word 后续加 |
| 批量提取模式？ | 全自动 + 树形勾选 | 默认全自动，可切换树形勾选 |
| 图片怎么处理？ | 下载到本地，引用本地路径 | 预览时用远程 URL，导出时下载到本地 |
| 历史记录？ | 第一版不做，先记录下来 | P2 规划，预留数据结构 |
| 设置项？ | 输出目录、并发数、图片开关、默认路径 | 默认输出路径 + 可自定义，并发数可配，图片可开关 |
