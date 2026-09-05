# LarkReader 后端待处理问题清单（复核后）

> **状态更新（2026-09-05）**：本文记录的 O-01 ～ O-07 已全部修复。本文保留为历史审查证据，不再代表当前待办。
>
> - O-01：stdout/stderr 由独立线程持续读取，消除大输出管道死锁。
> - O-02：快速查询 15 秒、普通读取 120 秒、交互登录 600 秒。
> - O-03：单张图片旧文件删除失败降级为图片失败，不中断整篇文档。
> - O-04：循环检测改为当前祖先路径，节点总数独立计数。
> - O-05：相同 URL 图片去重下载。
> - O-06：预计算选中节点及祖先集合，递归查询降为 O(1)。
> - O-07：Windows 保留名仅在 Windows 生效，并检查扩展名前的主干名。

> 最近审查：2026-09-05（第三轮，对应提交 `fe0dcbf` / `b80a001` 之后）
> 审查范围：`src-tauri` Rust 后端
> 角色：只做检查，不改代码

## 1. 本文档与 `BACKEND_CORE_ISSUES.md` 的分工

| 文档 | 定位 |
|---|---|
| `BACKEND_CORE_ISSUES.md` | 首轮全面审查，问题编号 **B-01 ~ B-32**、测试缺口 **T-01 ~ T-06** |
| 本文档 | 修复批次之后的**复核发现**。已修复项见第 2 节（不必重复劳动），待处理项见第 3 节（编号 **O-xx** / **N-xx**） |

---

## 2. 已修复项（复核通过，无需再动）

### 2.1 O 系列（上一轮提出，本轮已全部修完除 O-06）

| 编号 | 问题 | 修复方式 |
|---|---|---|
| **O-01** | `run_lark` 管道死锁，输出 > 64KB 必然超时 | `lark.rs` 改用 `std::thread::spawn` 起两个线程分别读 stdout/stderr，主线程 `wait_timeout`，超时 `kill` 后 `join` 读线程。**这是原 `Command::output()` 的能力等价恢复，死锁已消除** |
| **O-02** | 统一 120 秒超时打断交互式命令 | 拆出三个包装：`run_lark_quick` 15 秒（`whoami`/`config show`）、`run_lark` 120 秒（默认）、`run_lark_interactive` 600 秒（`config init`/`auth login` 阻塞/device-code） |
| **O-03** | 图片目标文件删除失败中断整篇文档 | `extract.rs` 改为记录错误 + `images_failed += 1` + `continue`，与同函数内 `rename` 失败处理一致 |
| **O-04** | Wiki 循环检测用全局集合会误判 | 改名为 `ancestors`，递归前 `insert`、递归后 `remove`（真正的路径级检测）；节点计数改用独立的 `node_count` |
| **O-05** | 重复图片去重逻辑已回退 | `extract.rs` 重新加入 `processed_urls: HashSet`，同一 URL 只下载一次 |
| **O-07** | `safe_filename` 保留名规则平台无关 | 改为 `cfg!(windows) &&` 限定，并用 `split('.').next()` 取主干匹配，覆盖 `CON.txt` 形式 |

### 2.2 本轮额外修完的 B 系列

| 编号 | 问题 | 修复方式 |
|---|---|---|
| **B-19** | 图片解析只覆盖简单 Markdown 语法，`pulldown-cmark` 依赖白装 | `markdown.rs::extract_images` 改用 `pulldown_cmark` 的 `Parser` / `Tag::Image` 事件流，不再靠正则 |
| **B-16** | 非 Doc 类型（Sheet/Bitable/Other）静默跳过 | 新增 `SkippedNode` / `skipped_count`，带中文 `reason` 说明 |
| **B-02** | 同名目录互相覆盖（部分修） | 新增 `unique_directory()`，冲突时创建 `名称 (2)`、`名称 (3)`… 不再覆盖已有导出 |
| **B-17** | 任务系统缺失（部分修） | 新增 `AppState.tasks`、`TaskControl`、`get_progress`、`cancel_task`、`start_extract_wiki`；`Progress` 增加 `TaskStatus`（Pending/Running/Completed/Failed/Cancelled） |

### 2.3 更早批次已修（此前已复核）

B-01、B-03（部分）、B-05、B-06、B-07、B-08、B-09、B-14、B-15、B-23、B-26、B-30；Clippy 9 类 → 0；单元测试 7 → 8。

---

## 3. 待处理问题

### 3.1 上一轮遗留

#### O-06【P2】Wiki 选择判定为 O(n²)

- 位置：`src-tauri/src/wiki.rs` 的 `collect_docs_recursive` 与 `collect_skipped_recursive`
- 两者都会对每个节点调用 `is_node_or_descendant_selected`，后者完整扫描该节点子树

新增 `collect_skipped_recursive` 后**同一个判定被执行了两遍**，复杂度从 O(n²) 变成 2×O(n²)，10,000 节点上限时最坏约 2 亿次字符串比较。

**建议**：进入递归前一次性预计算「被选中节点及其全部祖先」的 `HashSet`，供两个递归函数共用，查询降为 O(1)。

---

### 3.2 本轮新增

#### N-01【P0-新】`cargo fmt --check` 失败，CI 基线被破坏

上一轮复核时该项通过，当前失败，共 2 处：

```
src-tauri/src/env.rs:100       status.logged_in 赋值需要换行
src-tauri/src/markdown.rs:6    use regex::Regex 与 use pulldown_cmark::... 顺序需调整
```

说明提交前未跑 `cargo fmt`。这只是格式问题，跑一次 `cargo fmt` 即可，但会阻塞任何把 `fmt --check` 纳入 CI 的计划。

#### N-02【P1-新】任务结果被丢弃，前端拿不到导出明细

- 位置：`src-tauri/src/commands.rs:243-295`（`start_extract_wiki`）

```rust
let result = crate::wiki::extract_wiki_controlled(...).await;
if let Ok(mut p) = progress.lock() {
    if cancelled.load(Ordering::Relaxed) {
        p.status = TaskStatus::Cancelled;
    } else if result.is_ok() {
        p.status = TaskStatus::Completed;
    } else {
        p.status = TaskStatus::Failed;
        p.errors.push(result.err()...);
    }
}
```

`WikiExtractResult` 里的 `results`（每篇文档的输出路径）、`failures`（失败清单）、`skipped`（不支持的节点）**全部丢弃**，只用了 `is_ok()`。

后果：前端用 `start_extract_wiki` 拿到 task_id 后轮询进度，任务结束时只能看到 `Completed`，**无法得知导出了哪些文件、哪些失败了、输出目录在哪**。而这些信息后端明明已经算出来了。

**建议**：把结果存进 `TaskControl`，并新增 `get_task_result(task_id)` 命令返回 `WikiExtractResult`。

#### N-03【P1-新】任务对象永不清理，持续内存泄漏

- 位置：`src-tauri/src/commands.rs:292`

```rust
let _ = (tasks, task_id_for_run);
```

这行只是为了消除 `unused` 警告（`tasks` 被 clone 进闭包却没用）。任务结束后，`state.tasks` 里的 `TaskControl` **永远不会被移除**，也没有任何清理接口。

每次导出泄漏一个 `Progress`（含完整的 `errors: Vec<String>`）。长时间反复导出会持续累积。

**建议**：在 spawn 的闭包末尾将任务从 `tasks` 中移除；若希望前端还能查询终态，可保留结果并延迟清理（例如完成后 5 分钟移除），或提供显式的 `remove_task`。

#### N-04【P2-新】`Progress.total` 初始为 0

- 位置：`src-tauri/src/commands.rs:253`

`Progress::new(task_id, 0)`，真实 total 要等 `extract_wiki_controlled` 内部遍历完目录树后才写入（`wiki.rs:198`）。在此之前前端会看到 `0 / 0`，进度条需要容错处理。

#### N-05【P2-新】取消是粗粒度的，且不终止子进程

- 位置：`src-tauri/src/wiki.rs:207-219`

取消标志只在**每篇文档之间**检查。单篇文档内部（含几十张图片串行下载）无法中断；已经发出去的 `lark-cli` 子进程也不会被 kill，只能等它自然结束或走 120 秒超时。

用户点「取消」后可能要等很久才有响应。

**建议**：在图片下载循环内也检查取消标志；长期方案是给 `run_lark_with_timeout` 增加取消句柄，超时或取消时 kill 子进程树。

#### N-06【P2-新】`unique_directory` 存在 TOCTOU 竞态

- 位置：`src-tauri/src/wiki.rs:302-314`

用 `candidate.exists()` 判断后再 `create_dir_all`，并发任务同时导出同名知识库时可能撞名。另外兜底分支会生成 `<名称>_<PID>`，导出目录名带进程 ID 对用户不友好。

#### N-07【P2-新】改用 AST 后，`dest_url` 可能与 Markdown 原文不一致

- 位置：`src-tauri/src/markdown.rs:18-47` 与 `extract.rs` 的 `content.replace(&img.url, &local_ref)`

`pulldown_cmark` 的 `dest_url` 是**解码后**的值（如 `%20` → 空格、`&amp;` → `&`），而 `extract.rs` 替换图片引用时用的是**原文字符串匹配** `content.replace()`。若 URL 含转义字符，两者不一致会导致替换失败，导出的 Markdown 里仍保留远程 URL，图片实际已下载到本地但引用不到。

**建议**：替换改为基于 `Parser::into_offset_iter()` 的源码 span 定位，或对 `dest_url` 做与原文一致的转义还原。建议先用含 `%20`、中文、括号的飞书图片链接做一次实测确认。

#### N-08【P2】`Settings.concurrency` 仍未生效

校验了 1–32 范围，但图片下载仍是 `for` 串行循环，设置并发数不产生任何效果。属于 B-17 未完部分。

---

## 4. 需要产品 / 架构决策的事项（暂缓）

| 编号 | 事项 |
|---|---|
| B-02（剩余） | 文件级覆盖策略（当前只解决了目录级，同名文档仍会覆盖） |
| B-04 | 导出事务性（临时目录 + 原子提交，失败不破坏上次导出） |
| B-12 | 结构化错误码（当前 `AppError` 仅序列化中文字符串，前端只能匹配文本） |
| B-24 | `lark-cli` 固定版本（当前安装 `@latest`） |
| B-17（剩余） | 图片并发下载语义、任务并发上限 |

---

## 5. 建议处理顺序

1. **N-01** —— `cargo fmt` 一行命令，先让 CI 基线恢复绿色
2. **N-02** —— 否则新任务系统对前端不可用（只能看到状态，拿不到结果）
3. **N-03** —— 否则每次导出都泄漏，长跑会累积
4. **N-04**、**N-05**、**N-06**、**N-07** —— 体验与正确性，建议 N-07 先实测确认是否真有问题
5. **O-06**、**N-08** —— 性能与未兑现设置

## 6. 完成标准

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --lib`、`pnpm build` **四项全绿**
- 前端能通过 `start_extract_wiki` + `get_progress` + 结果查询接口拿到完整导出明细（N-02）
- 任务结束后 `state.tasks` 不残留（N-03）
- 含转义字符的图片 URL 能正确替换为本地路径（N-07）
- 反复导出同一知识库不会覆盖上次结果，命名可预期（B-02 / N-06）
