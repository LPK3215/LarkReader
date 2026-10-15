# LarkReader 项目分析（2026-10-15）

> 快照记录：本文基于 2026-10-15 的仓库状态（`main` @ `cff0d26`）与一次真实执行的
> `npm run verify` 得出。文中「发布门禁实测」一节是当时事实，代码继续演进后以实际
> 运行结果为准。
> 本文只做分析与记录，未改动任何代码。

## 1. 项目定位

Tauri 2 + Rust + Vue 3 桌面应用，把飞书知识库导出为本地可直接阅读、离线可用的文件：

- 导出：Wiki 递归遍历 → docx 转 Markdown（图片本地化）、Sheet 转 XLSX（官方导出被拒时自动降级为读单元格生成；可切换「纯数据」模式）、Bitable 转 NDJSON、`file` 附件按原始字节下载，整棵目录树按飞书层级与排序落盘；
- 阅读：内置「本地阅读」页，离线浏览已导出的 Markdown 与图片；
- 认证：通过官方 `lark-cli`（固定 `1.0.93`）完成环境体检、设备码登录、隔离浏览器授权、授权范围巡检，凭据由 lark-cli 本机托管；
- 边界：纯本地工具，无自建服务端、无遥测。

版本 0.2.3，GPL-3.0-only。分析时工作区干净，无未提交改动。

## 2. 架构与模块

前后端通过 Tauri IPC 通信，共 **31 个命令**（`src-tauri/src/lib.rs` 注册）。契约纪律明确：

- 入参 camelCase / 出参 snake_case / 枚举 snake_case；
- 业务错误统一 `AppError { code, message, retryable }`（`error.rs` 序列化，`retryable` 标记可重试类错误）；
- 类型契约双向镜像：`src-tauri/src/models.rs` ↔ `src/api/types.ts`（后者是前端唯一类型源）。

### 2.1 后端（约 5.0k 行，13 个文件）

| 模块 | 职责 |
|---|---|
| `commands.rs` | IPC 注册层；任务生命周期（后台执行、进度、取消、历史持久化 24h / 100 条） |
| `wiki.rs` | 知识库树扫描（Auto / FullSpace）、导出编排；循环 / 深度 <= 64 / 节点 <= 10000 三重保护 |
| `extract.rs` | 单文档导出（图片并发 1-32、事务式落盘）、内嵌 sheet / bitable / 图片处理、失败重试与分类上报 |
| `markdown.rs` | 富对象还原（内嵌表格、html5-block、深层标题 h7~h9、HTML 表格转管道表）、文件名清洗、图片引用替换 |
| `lark.rs` | lark-cli 子进程封装：超时熔断（15 / 120 / 300 / 600 秒分档）、进程树终止、退避重试、输出路径白名单适配、表格导出降级 |
| `env.rs` | 环境体检（Node / lark-cli / 应用配置 / 登录态）、登录 / 退出 / 清除授权 |
| `isolated_browser.rs` | 隔离 profile 的授权浏览器（Chrome 优先 / Edge 兜底） |
| `reader.rs` | 本地阅读：目录一层惰性列举、md / 文本白名单读取（8 MiB 上限）、图片 data URL（16 MiB 上限） |
| `models.rs` / `error.rs` / `logger.rs` | 数据结构、错误协议、文件日志 |

### 2.2 前端（约 7.8k 行）

- `api/` 8 个模块封装 IPC；`stores/` 5 个 Pinia store（auth / task / settings / history / onboarding）；
- `views/` 7 个页面（Workspace / Reader / History / Logs / Terminal / Settings / Onboarding）；
- `components/` 11 个业务组件（含节点树、任务面板、结果卡片、布局组件）；
- 路由为哈希模式；未完成引导时由路由守卫送往 `/onboarding`。

### 2.3 任务体系

`start_extract_wiki` 立即返回任务 ID，后台执行；8 阶段进度（queued / checking_output / scanning_wiki /
exporting_document / exporting_sheet / exporting_bitable / exporting_file / finalizing）；协作式取消并
返回部分结果；结果与历史落盘持久化；知识库树扫描结果缓存复用（同一棵树不重复扫描）。

## 3. 工程质量亮点

- **错误处理成体系**：统一错误协议 + 环境体检把异常分类成 5 种 + 授权范围巡检采用保守判定（只有确认缺项 `missing` 才拦截导出）。
- **可靠性细节密度高**：临时文件 + rename 原子提交、输出目录写探测与磁盘空间预检、配置文件损坏自动备份恢复、路径词法展开（规避 Windows `..` rename 失败）、日志文件名路径穿越校验、lark-cli 输出路径白名单适配（`--flag=false` 写法、子进程 cwd 技巧）。
- **降级链完整**：表格官方导出被拒（`1069902`）自动改走「读单元格 → 本地生成 xlsx」；权限报错按错误码分流透出。
- **单测聚焦边界**：77 个用例集中在最易错的 Markdown 解析（围栏保护、标签检出、转义、CSV → 表格、深层标题、图片单元格）、scope 判定、reader 白名单。
- **文档与脚本体系**：`BACKEND.md` 主文档、用户指南、人工验收清单、发布手册、压测记录、E2E 全量下载快照（42 文件）；`verify.mjs`（门禁）、`release.mjs`（门禁 → 版本三处回写 → tag → CI）、`clean-e2e.mjs`。

## 4. 发布门禁实测（2026-10-15）

真实执行 `npm run verify`，结果：

| 步骤 | 结果 |
|---|---|
| `cargo fmt --check` | **未通过**（4 个文件有 diff） |
| `cargo clippy --all-targets -- -D warnings` | **未通过**（4 处） |
| `cargo test --lib` | 通过：77 passed / 0 failed / 1 ignored |
| `npm run build`（vue-tsc + vite） | 通过（2773 模块，约 6.3s） |

### 4.1 fmt 未通过明细

涉及 `env.rs`、`extract.rs`、`lark.rs`、`markdown.rs` 四个文件，共 22 处 diff，全部是
rustfmt 的换行 / 聚合类差异，无逻辑问题。`git blame` 归因：

- `env.rs`：来自 `b3a48d7`（清除授权功能，2026-10-15）；
- `extract.rs` / `lark.rs` / `markdown.rs`：来自 `cff0d26`（压测修复，2026-10-15）。

### 4.2 clippy 未通过明细

| 位置 | 问题 | 引入提交 | 修复建议 |
|---|---|---|---|
| `extract.rs:404` | 字符串字面量上多余的 `.to_string()` | cff0d26 | 直接传字面量（`&str`） |
| `lark.rs:1440` | `&extract_json(&stdout)` 取引用后立即解引用 | cff0d26 | 去掉多余的 `&` |
| `markdown.rs:851` | `row_index` 手写循环计数器 | cff0d26 | 该循环每轮恰好 `+1`（第 886 行），可改为 `rows.iter().enumerate()`，行为等价 |
| `extract.rs:1124` | `render_embedded_sheet` 参数 8 个（clippy 上限 7） | 51b41c3d | 调用点仅一处（`extract.rs:816`）；建议把 `label / token / sheet_id / origin / index / produce_file` 收进结构体。仓库目前没有任何 `#[allow(clippy::...)]` 先例，倾向不引入 |

### 4.3 影响

1. `npm run release` 默认先跑 `npm run verify`：**下一次发版会被该红门禁直接拦住**（除非显式 `--no-verify`）；
2. CI（`.github/workflows/publish.yml`）只做构建与发布，**不含** fmt / clippy / test 步骤，因此红门禁不会在 CI 暴露，完全依赖本机拦截；
3. 根因倾向：最近两次提交（`b3a48d7`、`cff0d26`）提交前未完成一次 `npm run verify`（`cff0d26` 的提交信息显示跑过 `cargo test`，但 fmt / clippy 状态与提交内容不符）。

## 5. 文档一致性观察

分析时点上，README 与代码存在以下出入：

- README 写「27 个命令」，`lib.rs` 实际注册 **31 个**（0.2.2 后新增 `purge_auth_data`、`check_scopes`、`qr_svg`、`open_isolated_browser`、`find_first_reader_doc` 等）；
- README 徽章与「验证与实测」写 26 项测试 / fmt 通过，实际为 **77 项测试、fmt 未通过**；
- 以上属文档滞后，不影响功能。

## 6. 仓库杂项（可选清理）

- 仓库根有 `build-out3.log`、`build-out4.log`、`dev-run.log`（均在 .gitignore 内）；
- `e2e_download_tmp/`、`e2e_download_tmp_big/` 两个 E2E 临时目录仍在（由 `npm run clean:e2e` 负责）；
- `docs/e2e-download-case/` 内有一个文件名含 `--obj-type ... --parent-node-token ...` 的历史文件（早期 bug 产物，作为快照证据保留，无害）；
- 全库无 TODO / FIXME / HACK 残留，无未跟踪源码文件。

## 7. 建议的后续动作（均未执行，供维护者决定）

1. 格式化：`cd src-tauri && cargo fmt`，消除全部 22 处 fmt diff；
2. 按 4.2 表格修 4 处 clippy（其中 `render_embedded_sheet` 建议收参数进结构体）；
3. 复跑 `npm run verify` 确认全绿后再提交 / 发版；
4. 同步 README 的命令数（31）、测试数（77）与「验证与实测」一节；
5. 可选：清理根目录构建日志与 E2E 临时目录（`npm run clean:e2e`）。

## 附录：复现方式

```powershell
# 全量门禁（fmt / clippy / 单测 / 前端构建）
npm run verify

# 仅 Rust
node scripts/verify.mjs --rust-only

# 单项
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
```

本次分析环境：Windows，rustc 1.96.0 / rustfmt 1.9.0-stable，Node 22 + Vite 8。

## 复验记录（2026-10-16，afcac4a 之后）

HEAD = `afcac4a`（2026-10-16 00:16，「第二轮压测」修复提交）时复跑 `npm run verify`：

| 步骤 | 结果 |
|---|---|
| `cargo fmt --check` | 仍未通过：diff 从 22 处增加到 27 处（markdown.rs 新代码带入 5 处；env.rs / extract.rs / lark.rs 的既有 diff 未被处理） |
| `cargo clippy --all-targets -- -D warnings` | 仍未通过：同样 4 处，行号随代码偏移（extract.rs:409、extract.rs:1129、lark.rs:1440、markdown.rs:992） |
| `cargo test --lib` | 通过：84 passed / 0 failed / 1 ignored（较 cff0d26 的 77 增加 7 个测试） |
| `npm run build` | 通过 |

两点说明：

- `afcac4a` 只改动 `docs/press-test-findings.md`、`extract.rs`、`markdown.rs`，未触及门禁红项所在代码
  （env.rs 的格式化、lark.rs 的格式化与 needless borrow），第 4 节结论不变：门禁仍红，
  `npm run release` 仍会被拦；
- `afcac4a` 提交信息写「新增 4 个单元测试（80 → 84 passed）」，实测基线为 77（cff0d26），
  本次新增 7 个测试函数，计数与实际有出入。

## 修复完成记录（2026-10-16，v0.3.0）

- `674d47a fix(build): 修绿发布门禁（cargo fmt + clippy）` 修掉了第 4 节全部红项
  （4 个文件的 fmt diff + 4 处 clippy，`render_embedded_sheet` 按建议收敛为 `SheetTarget` 结构体）；
- 本机在 HEAD = `4dae5bd`（v0.3.0 发布后）复跑 `npm run verify`：**全绿** ——
  fmt / clippy 通过，lib 单测 84 passed / 0 failed / 1 ignored，前端构建通过；
- 第 5 节文档漂移项由 `9c82834` 同步（README 命令数 27 → 31、测试数 26 → 84、版本 0.3.0）；
- tag `v0.3.0` 已推送远程（指向 `9bbb59a` 版本回写提交；其后 `4dae5bd` 仅同步
  Cargo.lock 版本号，无代码差异）。
