# 飞书 Wiki 提取工具 — 开发规划文档

> 本文档记录从 MVP 脚本到开源桌面应用的完整规划。
> MVP 版本已验证可用，不再改动。
> 后续开发在新仓库进行。

### MVP 最小实现（参考源码）

开发时直接对照以下文件，逻辑一一对应，只是 Python → Rust 换语言：可以进行修改，因为仅仅只是借鉴思路，因为这个项目可以完全运行，我要开发的项目无非是换语言，写细节，拓展功能，而且使用npm create tauri-app@latest去创建规范的项目结构

| 文件 | 路径 | 说明 |
|---|---|---|
| 提取脚本 | `04-Scripts/飞书Wiki提取工具/extract_generic.py` | 109 行，包含全部核心逻辑 |
| 配置脚本 | `04-Scripts/飞书Wiki提取工具/setup.ps1` | 131 行，包含环境检测+安装+登录流程 |
| 使用文档 | `04-Scripts/飞书Wiki提取工具/README.md` | 完整原理说明+注意事项 |

---

## 零、核心约束（不可妥协）

以下四条约束决定了整个项目的技术选型和产品形态，任何方案都必须同时满足：

| # | 约束 | 原因 | 影响 |
|---|---|---|---|
| 1 | **必须依赖 lark-cli** | lark-cli 解决了飞书 OAuth 认证 + API 封装，不可替代；去掉它等于重写全部认证和 API 调用逻辑 | 应用必须运行在用户本地电脑上，不能是纯云端服务 |
| 2 | **必须有界面** | 目标用户不只是开发者，命令行门槛太高 | 不能只有 CLI，必须有 GUI 窗口 |
| 3 | **双击打开** | 用户体验要求：下载→安装→双击→用，不需要终端/命令行 | 桌面应用，不是 Web 服务，不是 npm 包 |
| 4 | **跨平台** | 飞书用户覆盖 Windows + Mac | 至少支持 Windows + macOS，Linux 可选 |

**由这四条约束推导出的技术选型**：

```
约束 1 → 必须本地运行 → 排除纯云端方案
约束 2 → 必须有 GUI  → 排除纯 CLI 方案
约束 3 → 双击打开   → 排除 Web 应用、npm 包、pip 包
约束 4 → 跨平台     → 排除 WinForms/WPF（Windows 专属）
                          排除 PyQt（跨平台但打包体验差）

满足全部 4 条 → Tauri 2（Rust 后端 + Web 前端，打包成 .exe/.dmg）
```

---

## 一、项目背景

### 1.1 要解决什么问题

飞书知识库的内容无法直接复制导出：
- 浏览器前端禁了复制、右键、选择
- `media-download` API 需要导出权限，文档管理员可关闭
- 没有官方批量导出功能

### 1.2 已验证的解决方案

通过 `lark-cli`（飞书官方 CLI 工具）走开放平台 API：
- `docs +fetch --as user` → 用个人阅读权限拿文档正文（Markdown）
- `docs +media-preview --as user` → 用阅读权限下载图片（绕过导出权限）
- `wiki +node-get / +node-list --as user` → 递归遍历知识库目录树

**核心原理**：前端限制对 API 无效，只要账号有阅读权限就能提取。

### 1.3 MVP 验证结果（2026-09-05）

| 验证项 | 命令 | 结果 |
|---|---|---|
| lark-cli 安装 | `npm install -g @larksuite/cli` | ✅ v1.0.93 |
| 应用配置 | `lark-cli config init --new` | ✅ App ID: cli_aa11bcce79b8dcbc |
| 用户授权 | `lark-cli auth login --domain docs,drive,wiki` | ✅ 用户614265 |
| 文档提取 | `lark-cli docs +fetch --as user` | ✅ ok=true, 9998 字符 |
| 图片下载 | `lark-cli docs +media-preview --as user` | ✅ ok=true, 26KB PNG |
| 身份验证 | `lark-cli whoami` | ✅ identity=user, token=ready |

---

## 二、技术选型

### 2.1 选型决策

| 层 | 技术 | 选型理由 |
|---|---|---|
| 桌面框架 | **Tauri 2** | 体积小（~5MB），Rust 后端直接调 lark-cli，Win/Mac 跨平台 |
| 后端 | **Rust** | 调用 lark-cli（`Command::new`）、文件操作、JSON 解析 |
| 前端 | **Vue 3 + TypeScript** | 生态成熟，Tauri 官方模板支持（**后端完成后开发**） |
| 样式 | **Tailwind CSS** | 快速开发（**后端完成后开发**） |
| 打包 | Tauri 内置 | 生成 NSIS 安装包（Win）/ DMG（Mac） |

### 2.2 为什么选 Tauri 不选 Electron

| 对比项 | Tauri 2 | Electron |
|---|---|---|
| 打包体积 | ~5MB | ~80MB |
| 内存占用 | ~30MB | ~150MB |
| 后端语言 | Rust（直接调系统命令） | Node.js（也能调，但重） |
| 前端 | 任意框架（选 Vue 3） | 任意框架 |
| 安全性 | 默认禁用 Node API | 默认开放 Node API |
| 跨平台 | Win/Mac/Linux | Win/Mac/Linux |

参考案例：钛极工具箱（Tauri 2 + Rust + React 18，Windows 桌面系统调度工具）

### 2.3 为什么必须依赖 lark-cli

| 如果不依赖 lark-cli | 如果依赖 lark-cli |
|---|---|
| 需要自己实现飞书 OAuth Device Flow | lark-cli 已经实现 |
| 需要自己封装 3+ 个 API 调用 | lark-cli 已经封装 |
| 需要自己管理 token 刷新 | lark-cli 自动管理 |
| 用户需手动创建飞书应用 | `lark-cli config init --new` 自动引导 |
| 开发周期长 | 已验证可用 |

**结论**：lark-cli 解决了最麻烦的认证和 API 封装，不可替代。

### 2.4 排除的方案

| 方案 | 排除原因 |
|---|---|
| 纯 Python CLI | 违反约束 2（必须有界面）和约束 3（双击打开） |
| Electron | 体积 80MB 太大，内存占用高 |
| Web 应用 | 违反约束 3（不能双击打开），且云端无法调本地 lark-cli |
| WinForms/WPF | 违反约束 4（仅 Windows） |
| PyQt | 跨平台但打包体验差，体积大 |
| npm/pip 包 | 违反约束 3（需要命令行安装） |

---

## 三、后端架构（先开发）

> **开发顺序：后端优先，前端等后端完全处理完再说。**

### 3.1 后端职责

Rust 后端负责所有与 lark-cli 的交互和文件操作：

| 模块 | 职责 | 对应 lark-cli 命令 |
|---|---|---|
| `lark.rs` | lark-cli 调用封装 | 所有 `lark-cli` 调用的底层入口 |
| `extract.rs` | 文档提取 + 图片下载 | `docs +fetch` + `docs +media-preview` |
| `wiki.rs` | Wiki 节点树递归遍历 | `wiki +node-get` + `wiki +node-list` |
| `env.rs` | 环境检测与配置 | `lark-cli whoami` / `lark-cli config show` |
| `commands.rs` | Tauri 命令注册 | 暴露给前端的接口 |

### 3.2 后端核心调用链

```
前端 invoke("extract_doc", {url, output_dir})
    │
    ├── env.rs: 清除 HERMES_HOME 等干扰变量
    │
    ├── extract.rs:
    │   ├── lark.rs: lark-cli docs +fetch --doc <url> --as user
    │   ├── 解析 JSON → 提取 Markdown 正文
    │   ├── 正则匹配图片引用 → 提取 file_token
    │   ├── lark.rs: lark-cli docs +media-preview --token <token> --as user
    │   ├── 下载图片到本地
    │   └── 替换 Markdown 中的远程 URL → 本地相对路径
    │
    └── 返回结果给前端
```

### 3.3 后端需处理的关键问题

| 问题 | 说明 | MVP 中的现状 |
|---|---|---|
| HERMES_HOME 干扰 | CatPaw 等 AI 工具会设置此变量，导致 lark-cli 报错 | 需在 Rust 中 `env_remove` |
| 图片正则 | `!\[([^\]]*)\]\(([^)]+)\)` 可能漏匹配特殊字符 | 需增强或换 Markdown 解析器 |
| 编码处理 | Windows 上 lark-cli 输出可能有编码问题 | 需统一 UTF-8 |
| 错误重试 | 网络波动导致下载失败 | MVP 无重试，需加 |
| 并发下载 | 174 张图逐张下载很慢 | MVP 无并发，需加 tokio |
| 断点续传 | 中断后从头开始 | MVP 无续传，需记录进度 |

### 3.4 后端暴露给前端的接口（Tauri commands）

| 命令 | 功能 | 状态 |
|---|---|---|
| `check_env` | 检测 Node.js / lark-cli / 飞书登录状态 | 待开发 |
| `login_feishu` | 引导用户浏览器登录飞书 | 待开发 |
| `extract_doc` | 提取单篇文档（正文+图片） | 待开发 |
| `extract_wiki` | 递归遍历并提取整个知识库 | 待开发 |
| `get_wiki_tree` | 获取知识库目录树（供前端展示） | 待开发 |

---

## 四、前端设计（后端完成后开发）

> 前端等后端完全处理完再说，这里只记录规划方向。

### 4.1 界面规划

| 界面 | 功能 | 优先级 |
|---|---|---|
| 主界面 | 输入飞书链接 + 选输出目录 + 提取按钮 + 日志区 | P0 |
| 配置向导 | 首次检测环境 + 引导安装 + 引导登录 | P1 |
| 目录树 | 知识库节点可视化 + 勾选提取 | P2 |
| 进度条 | 提取进度显示 | P1 |

### 4.2 前端技术栈（确定但暂不开发）

- Vue 3 + TypeScript
- Tailwind CSS
- Tauri invoke 通信

---

## 五、用户依赖与使用流程

### 5.1 运行时依赖

| 依赖 | 需要原因 | 谁负责安装 | 必须性 |
|---|---|---|---|
| **Node.js 18+** | lark-cli 的运行时 | 应用引导用户安装 | 必须 |
| **lark-cli** | 飞书 API 调用 + 认证 | 应用自动 `npm install -g` | 必须 |
| **应用本身** | 界面 + 提取逻辑 | 用户下载安装包 | 必须 |
| WebView2 | 渲染前端界面 | Windows 10+ 自带 | 自动满足 |

### 5.2 用户首次使用流程

```
1. 下载安装包（.exe / .dmg）
2. 双击打开应用
3. 应用自动检测环境：
   ├── 检测 Node.js → 未安装则提示安装（提供下载链接）
   ├── 检测 lark-cli → 未安装则自动 npm install -g @larksuite/cli
   └── 检测飞书登录 → 未登录则引导浏览器授权
4. 配置完成，进入主界面
```

### 5.3 用户日常使用流程

```
1. 打开应用
2. 粘贴飞书 Wiki 链接
3. 选择输出目录
4. 点击「提取」
5. 实时查看进度
6. 提取完成，文件已在输出目录
```

---

## 六、开发路线图

### 阶段 1：后端核心（当前优先）

**目标**：Rust 后端完全实现提取逻辑，通过命令行测试通过

- [ ] 初始化 Tauri 2 项目
- [ ] `lark.rs`：lark-cli 调用封装（含 HERMES_HOME 处理）
- [ ] `extract.rs`：文档提取 + 图片下载 + 本地路径替换
- [ ] `wiki.rs`：Wiki 节点树递归遍历
- [ ] `env.rs`：环境检测（Node.js / lark-cli / 飞书登录）
- [ ] `commands.rs`：Tauri 命令注册
- [ ] 命令行测试全部通过

### 阶段 2：前端界面（后端完成后开发）

**目标**：从命令行变成可双击的桌面应用

- [ ] `App.vue`：主界面布局
- [ ] `ExtractPanel.vue`：输入框 + 按钮 + 目录选择
- [ ] `LogViewer.vue`：实时日志显示
- [ ] 前后端通信调试
- [ ] 打包测试：生成 .exe，双击能跑

### 阶段 3：体验优化

- [ ] 配置向导（检测环境 + 引导安装 + 引导登录）
- [ ] 进度条
- [ ] 图片并发下载
- [ ] 断点续传
- [ ] 拖拽飞书链接
- [ ] 错误处理和提示

### 阶段 4：发布

- [ ] GitHub Release（.exe / .dmg）
- [ ] README 文档
- [ ] GitHub Actions 自动打包
- [ ] 开源协议（MIT）

---

## 七、与 MVP 版本的对应关系

| MVP（本目录） | 桌面应用版 | 说明 |
|---|---|---|
| `extract_generic.py` | `src-tauri/src/extract.rs` | Python → Rust |
| `setup.ps1` | 前端配置向导 | 脚本 → 界面 |
| `subprocess.run(["lark-cli", ...])` | `Command::new("lark-cli")` | Python subprocess → Rust std::process |
| `json.loads(result.stdout)` | `serde_json::from_slice()` | Python json → Rust serde_json |
| `re.findall(r'!\[...]', content)` | `regex::Regex` | Python re → Rust regex |
| 命令行参数 | 前端输入框 | CLI → GUI |
| `print()` 日志 | 前端日志区 | 终端 → 界面 |
| `$env:HERMES_HOME = $null` | `cmd.env_remove("HERMES_HOME")` | PowerShell → Rust |

---

## 八、已知限制与风险

### 8.1 依赖 lark-cli 的风险

| 风险 | 影响 | 应对 |
|---|---|---|
| lark-cli 版本更新导致命令变化 | API 调用失败 | 锁定版本 + 兼容层 |
| lark-cli 停止维护 | 无法调用飞书 API | 预留直接调 API 的 fallback |
| npm 全局安装权限问题 | 用户无法安装 | 提供 `npx` 方式或本地安装 |

### 8.2 飞书平台风险

| 风险 | 影响 | 应对 |
|---|---|---|
| 企业管理员关闭 OpenAPI | 所有调用失败 | 无法绕过，提示用户 |
| token 过期 | 需重新登录 | 应用检测过期 → 引导重新登录 |
| API 频率限制 | 批量提取被限流 | 加延迟 + 重试机制 |
| 文档无阅读权限 | 提取失败 | 提示无权限，跳过该文档 |

### 8.3 MVP 脚本的已知缺陷（桌面版需修复）

| 缺陷 | MVP 中的表现 | 桌面版修复方案 |
|---|---|---|
| 正则不够健壮 | `]` 等特殊字符导致漏匹配 | 用更强的正则或 Markdown 解析器 |
| 无并发 | 图片逐张下载，174张很慢 | Rust tokio 异步并发 |
| 无断点续传 | 中断后从头开始 | 本地 JSON 记录进度 |
| 无错误重试 | 失败就跳过 | 指数退避重试 3 次 |
| 编码问题 | Windows 上中文可能乱码 | Rust 统一 UTF-8 处理 |

---

## 九、开发环境（已就绪）

本机已安装全部开发依赖，明天可直接开始：

| 工具 | 版本 | 用途 | 状态 |
|---|---|---|---|
| Rust (rustc + cargo) | 1.96.0 | 后端开发 | ✅ |
| Rustup | 1.29.0 | Rust 版本管理 | ✅ |
| Node.js | 24.14.1 | 前端开发 + lark-cli 运行时 | ✅ |
| npm | 11.15.0 | 包管理 | ✅ |
| pnpm | 11.3.0 | 包管理（可选） | ✅ |
| Tauri CLI | 2.11.4 | 桌面应用脚手架 + 打包 | ✅ |
| lark-cli | 1.0.93 | 飞书 API 调用（已配置已登录） | ✅ |

**初始化命令**（明天直接跑）：
```bash
npm create tauri-app@latest
# 项目名: feishu-wiki-extractor
# 前端: Vue + TypeScript
# 包管理: pnpm
```

**不需要额外安装的**：
- Vue CLI 不需要（Tauri 用 Vite 模板，不依赖全局 vue 命令）
- WebView2 不需要装（Windows 10+ 自带）

---

## 十、技术参考

### 10.1 参考项目

| 项目 | 技术栈 | 参考点 |
|---|---|---|
| 钛极工具箱 | Tauri 2 + Rust + React 18 | Tauri 桌面应用完整实践 |
| lark-cli | Node.js | 飞书 API 封装和认证机制 |

### 10.2 关键文档

- Tauri 2 官方文档：https://v2.tauri.app/
- 飞书开放平台文档：https://open.feishu.cn/document/
- lark-cli npm 包：https://www.npmjs.com/package/@larksuite/cli

### 10.3 MVP 验证记录

- 验证日期：2026-09-05
- 验证环境：Windows 11, Node.js v24.14.1, lark-cli v1.0.93
- 验证结果：全部通过（文档提取、图片下载、身份验证）
- 验证数据：143篇文档 + 174张图片 + 1附件，59MB
