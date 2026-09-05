# 飞书登录"点击授权无反应"问题排查报告（2026-09-05）

> **【已解决，见 §7】** 根因：**浏览器侧状态问题**（换浏览器后立即成功）。此前所有失败都发生在同一个浏览器里——页面正常渲染但"开通并授权"提交静默失败。应用创建方式、权限大小、环境变量均非根因。排查教训：网页按钮无反应，第一时间换浏览器/无痕窗口复测。

---

## §7. 最终结论（解决记录）

> **⚠️ 修正（用户事后验证）**：真正的根因是**浏览器**。换一个浏览器后授权立即成功。此前所有失败尝试都发生在同一个浏览器里——页面能正常渲染、按钮点击却静默失败，属于浏览器侧状态问题（可能原因：陈旧的飞书登录会话、feishu.cn 脏 cookie、广告/隐私类扩展拦截提交请求）。本文档中"应用创建方式是根因"的推断不成立，向导的"自动完成所有配置"只是同时改变的两个变量之一。

当晚重建全链路（清空 `~/.lark-cli` → `config init --new` 向导创建新应用 → `auth login --scope <8个只读权限>` → **换浏览器**授权）**一次成功**：
`tokenStatus: ready`，token 实际获得 113 个 scope，`missing: []`。

**排查教训（重要）**：当"网页按钮点击无反应"时，**第一时间换浏览器/无痕窗口复测**，再排查其他变量。本次因始终没换过浏览器，先后误判为环境变量、权限大小、应用配置方式，多绕了数小时。

**若在原浏览器复现**：退出飞书重新登录 → 清除 feishu.cn 站点 cookie → 禁用广告/隐私扩展（或无痕窗口）后重试。

**经验法则**：应用创建仍推荐走 `lark-cli config init --new`（配置齐全、含发布，少踩空骨架应用的坑），但这不是本次故障的根因。

---

> 现象：新建自建应用后走 `auth login` 设备码授权，授权页能正常显示权限清单，但点击"开通并授权"没有任何反应，服务端始终停在"等待授权"状态，token 永远落不了盘。
> 本报告记录当日全部排查结论、已排除项与后续待办。证据均来自本机实测与 git 历史。

---

## 1. 问题精确定位（实测确认）

- 卡点位置：**授权页（accounts.feishu.cn）的"开通并授权"提交环节**，点击后飞书服务端**没有收到任何授权请求**。
  - 证据：用 `auth login --device-code <code>` 反查挂起的设备码，服务端持续返回"等待授权"（authorization_pending）。
- 授权页本身正常：能显示应用名（"测试"）、能列出申请的权限清单。
- 当日 14 次登录尝试全部失败于同一位置（`~/.lark-cli/cache/auth_login_scopes/` 留有全部申请记录）。

## 2. 已排除项（这些不是原因，不用再查）

| # | 排除项 | 排除依据 |
|---|---|---|
| 1 | **代码被改坏** | git 全历史对比（5b30809 → 58e067b 共 10 个提交）：登录命令 `--domain docs --domain drive --domain wiki` 从初始版本**从未变过**。已提交历史里没有任何改动触碰权限申请；工作区未提交的改动只有 `--recommend` 和 `whoami --as user` 两处 |
| 2 | **hermes 环境变量污染（作为最终根因）** | `HERMES_HOME`（C:\Users\17538\AppData\Local\hermes）确实会导致 lark-cli 拒绝执行（报 `hermes context detected but lark-cli is not bound to it`，是配置错误**不是权限错误**）。但它只挡住"命令启动"，清掉后命令能跑——它挡不到授权页的按钮，所以不是本次卡点的根因。Tauri 后端已有 `env_remove` 防护，不受影响 |
| 3 | **本地没有绑定应用 / 绑错应用** | `config show` 确认绑定 `cli_aa16a5021ab85bc2`；授权页显示的应用"测试"就是它（设备码流程只能属于发起方 app），无错绑 |
| 4 | **申请的权限太大导致被拒** | 用显式 `--scope` 只申请 **8 个只读权限** 复测（docx:document:readonly、docs:document.content:read、docs:document.media:download、drive:file:download、drive:drive.metadata:readonly、wiki:node:read、wiki:node:retrieve、wiki:space:retrieve），服务端确认申请清单只有 8 项，授权页也只显示这几项，**点击依然无反应** → 与申请大小无关 |
| 5 | **后台权限没开通** | 用户已确认该应用后台权限点均为"已开通"状态 |
| 6 | **本地 token 还有效着（旧登录顶用）** | `whoami --as user` 显示 `tokenStatus: missing`、`users: (no logged-in users)`。本机**从未成功持有过这个新应用的 token** |

## 3. 排查中发现的三个重要事实

### 3.1 `--recommend` 在 lark-cli 1.0.93 上基本无效

- 实测：`--domain docs drive wiki` 申请 **101** 个 scope；加 `--recommend` 后仍申请 **95** 个，其中大量写入类权限（建文档、传文件、删知识库节点、评论删改、白板创建……）。
- 工作区代码注释"--recommend 只申请自动审批权限"的假设**不成立**，勿依赖。

### 3.2 "不知道什么命令把权限都打开了"之谜已解开

- 就是历次 `auth login --domain ...` 的巨型 scope 申请：授权页上"一并开通并授权……免审权限修改"会把这些权限点批量开通到应用后台。
- 用户在后台看到的大量已开通权限，来自这些登录尝试，不是某个神秘命令。

### 3.3 hermes 的干扰机制与防护边界

- `HERMES_HOME` 是 Windows **用户级**环境变量，重装应用/lark-cli 都不会清除它；新开的终端窗口都会继承。
- Tauri 后端每次构造 lark-cli 子进程时会 `env_remove` 掉 `HERMES_HOME`/`OPENCLAW_HOME`/`LARK_CHANNEL`，所以**应用内调用不受影响**；但 `scripts/*.ps1` 和终端手动执行会中招。
- `~/.lark-cli/hermes/` 目录的出现即说明某次命令是在 hermes 上下文里跑的。

## 4. 剩余嫌疑（按概率排序，下一步从这里查）

1. **应用未发布版本（头号嫌疑）**：自建应用若从未在"应用发布 → 版本管理与发布"里创建并发布过版本（未上线），用户授权无法完成——恰好表现为"页面正常、按钮提交静默失败"。旧应用当年能用，很可能是发布过版本。
2. **可用范围不包含当前用户**：同在"应用发布"下，若可用范围为空或不含当前账号，授权同样失败。
3. **浏览器登录账号与应用主体不一致**：授权页右上角账号必须与创建该应用的开放平台账号同主体（用户名下有多个应用，可能存在多主体）。

**取证手段（一锤定音）**：授权页按 F12 → Network 面板 → 点"开通并授权" → 看哪个请求变红，其 URL 与响应体即服务端拒绝的原始错误。

## 5. 待办清单

- [ ] 开放平台后台检查"测试"应用的**版本发布状态**，若未发布：创建版本 → 可用范围勾选自己 → 发布 → 重新授权
- [ ] 检查**可用范围**与**浏览器登录账号主体**
- [ ] 若仍失败：F12 Network 取证，拿原始报错
- [ ] 代码改进：把登录命令从 `--domain`（捆绑 95+ scope）改为显式最小 `--scope` 列表（上面 8 个只读权限，已实测可行），位置：`src-tauri/src/env.rs:start_login` / `src-tauri/src/lark.rs:auth_login_blocking / auth_login_no_wait`
- [ ] 删除或修正 `--recommend` 相关注释（3.1 已证伪）
- [x] ~~`scripts/login.ps1`、`scripts/check-status.ps1` 开头清理 `$env:HERMES_HOME`~~（已失效：4 个 setup/login/check 脚本已整体删除，`HERMES_HOME` 清理仅存在于 Tauri 后端 `lark.rs::build_command` 的 `env_remove`，该防护已覆盖所有调用路径）
- [ ] 更新 `docs/FEISHU_AUTH.md`：补 `whoami --as user`、修正 `--recommend` 描述、记录本次排查结论

## 6. 本次排查建立起来的"防护线"（已避免重复踩坑）

- 单次阻塞等待授权，不做 3 秒轮询（轮询会作废 device code）——提交 ef6e7bd
- 禁止并发/重启 `auth login`
- 子进程统一清除 `HERMES_HOME`/`OPENCLAW_HOME`/`LARK_CHANNEL`
- `whoami --as user` 与业务命令身份判定一致（工作区未提交）
- 授权页域名校验：`open.feishu.cn` = 开发者动作，`accounts.feishu.cn` = 用户授权
- 登录状态检查口令：`env -u HERMES_HOME lark-cli whoami --as user`（PowerShell：先 `Remove-Item Env:HERMES_HOME`）
