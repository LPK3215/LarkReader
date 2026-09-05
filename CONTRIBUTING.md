# 贡献指南

欢迎任何形式的贡献：提 issue、修 bug、补文档、加功能。在动手前请先花两分钟读下面的约定。

## 这是什么项目

LarkReader 是一个**纯本地**的飞书文档阅读与导出桌面工具：前端（Vue + TS）只负责界面，真正的拉取/解析/导出逻辑全部在 Rust 后端（`src-tauri/src/`）通过 Tauri IPC 命令暴露。它不会把你的任何数据传出去——所有产物只写到你本地选择的目录。

## 开发环境

- Node.js 22+，包管理器 `npm`（勿再切换 pnpm，`pnpm tauri add` 会破坏 node_modules 结构）
- Rust stable（建议通过 `rustup` 安装）
- 前端：`src/`（Vue 3 + Vite + Pinia）
- 后端：`src-tauri/`（Tauri 2）

## 常用命令

```bash
# 安装依赖（前端 + 后端构建工具）
npm install

# 本地开发（带热更新的桌面窗口）
npm run tauri dev

# 只跑前端网页模式（无 Rust 后端时部分页面可用 mock 数据）
npm run dev

# 发布门禁：fmt / clippy / lib 单测 + 前端 vue-tsc / vite build
npm run verify

# 清空 E2E 下载回归输出目录（每次真实下载测试前执行）
npm run clean:e2e

# 后端单元/集成测试（真实网络相关的测试已用 mock fixture，可离线跑）
cd src-tauri && cargo test
```

## 前后端契约（重要）

前后端通过 `invoke` 通信，命令在 Rust 侧 `#[tauri::command]` 定义，由 `src-tauri/src/commands` 注册。改动时请遵守：

1. **入参 camelCase、出参 snake_case**，这是历史约定，别破坏；
2. 字段形状以 `src-tauri/src/models.rs` 为准，前端类型定义在 `src/api/types.ts`——改了后端模型，必须同步改这里，反之亦然；
3. 业务错误统一走 `AppError`（含 `code` / `message` / `retryable`），**不要**依赖把中文文本当作错误码来匹配；
4. 任务类命令是长任务，走 `task_status` 轮询 + `task_cancel` 协作式取消，不要在 `task_start` 里做阻塞等待。

## 提交规范

- 提交信息建议遵循约定式：`feat:` / `fix:` / `refactor:` / `chore:` / `docs:` 等。
- 一个提交只做一件事，别混入无关格式化。
- 提交前跑一遍 `npm run verify`（含 fmt / clippy / 单测 / `vue-tsc` / `vite build`），真实下载类集成测试按 `scripts/README.md` 手动跑。

## 许可

本项目以 **GPL-3.0** 授权。提交 PR 即表示你同意你的贡献以本项目同一许可（GPL-3.0）发布。

如果对流程有疑问，直接开 issue 问即可。
