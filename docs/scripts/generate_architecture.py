#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate_architecture.py — 生成四层架构图

用途：README「架构」章节的分层架构图（前端 / Tauri IPC / Rust 后端 / 外部依赖）。
依赖：仅 Python 3 标准库，无第三方包。
运行：python docs/scripts/generate_architecture.py
输出：docs/assets/architecture.svg

数值来源（改动后请同步 DATA 再重跑）：
  - fe_counts.*   -> src/ 下 .vue / .ts 的实际文件数
  - commands      -> src-tauri/src 中 #[tauri::command] 的数量
  - rust_files / rust_loc -> find src-tauri/src -name '*.rs' | xargs wc -l
  - cli           -> src-tauri/src/env.rs  SUPPORTED_LARK_CLI_VERSION
"""

from pathlib import Path

W, H = 1280, 800
FONT = "'PingFang SC','Microsoft YaHei','Segoe UI',Helvetica,Arial,sans-serif"

C = {
    "bg": "#0B1220",
    "band": "#111B28",
    "box": "#16212F",
    "box2": "#1A2634",
    "border": "#2A3A50",
    "text": "#E6EDF3",
    "muted": "#93A6BD",
    "dim": "#6B7F97",
    "blue": "#4C8DFF",
    "green": "#3FCF8E",
    "orange": "#FFB454",
    "purple": "#A97BFF",
}

DATA = {
    "commands": 29,
    "rust_files": 12,
    "rust_loc": 5120,
    "unit_tests": 23,
    "cli": "1.0.93",
}

# ---------------------------------------------------------------- 基础绘制工具
def esc(s: str) -> str:
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def t(x, y, s, size=13, fill=C["text"], weight="400", anchor="start"):
    return (
        f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" font-weight="{weight}" '
        f'text-anchor="{anchor}">{esc(s)}</text>'
    )


def rect(x, y, w, h, fill, rx=10, stroke="none", sw=1):
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" ry="{rx}" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'
    )


def arrow_down(x, y1, y2, color=C["dim"]):
    return (
        f'<line x1="{x}" y1="{y1}" x2="{x}" y2="{y2 - 6}" stroke="{color}" '
        f'stroke-width="1.6" marker-end="url(#ah)"/>'
    )


def arrow_right(x1, x2, y, color=C["dim"]):
    return (
        f'<line x1="{x1}" y1="{y}" x2="{x2 - 6}" y2="{y}" stroke="{color}" '
        f'stroke-width="1.6" marker-end="url(#ah)"/>'
    )


def band(x, y, w, h, title, accent, note=""):
    """一个分层色带：底色 + 左侧强调条 + 标题。"""
    out = [
        rect(x, y, w, h, C["band"], rx=14, stroke=accent, sw=1),
        f'<rect x="{x}" y="{y + 14}" width="4" height="{h - 28}" rx="2" fill="{accent}"/>',
        t(x + 24, y + 30, title, size=15, fill=accent, weight="600"),
    ]
    if note:
        out.append(t(x + w - 24, y + 30, note, size=11.5, fill=C["dim"], anchor="end"))
    return out


def box_item(x, y, w, h, title, lines, accent):
    """色带内的子卡片：标题 + 1~2 行说明。"""
    out = [rect(x, y, w, h, C["box"], rx=9, stroke=C["border"], sw=1),
           f'<rect x="{x}" y="{y}" width="3" height="{h}" rx="1.5" fill="{accent}" fill-opacity="0.75"/>']
    out.append(t(x + 14, y + 25, title, size=13.5, fill=C["text"], weight="600"))
    for i, line in enumerate(lines):
        out.append(t(x + 14, y + 45 + i * 15, line, size=10.8, fill=C["muted"]))
    return out


# ---------------------------------------------------------------- 内容数据
FE_BOXES = [
    ("views/", ["7 个页面 · Workspace / Reader / History", "Logs / Terminal / Settings / Onboarding"]),
    ("components/", ["11 个组件 · NodeTree / TaskPanel", "ReaderTree / DirPicker / ResultCard"]),
    ("stores/", ["5 个 store · auth / task", "settings / history / onboarding"]),
    ("api/", ["8 个模块 · types.ts 类型契约", "reader / auth / env / log / task / wiki"]),
    ("composables/", ["2 个 · useTaskProgress", "useMessage"]),
]

IPC_BOXES = [
    ("命令通道", [f'{DATA["commands"]} 个 #[tauri::command] · 前端 invoke() 双向调用 · CSP 已启用']),
    ("数据契约", ["入参 camelCase · 出参 snake_case · 统一 AppError{code, message, retryable}"]),
]

RUST_BOXES = [
    ("commands.rs", ["29 个 IPC 命令 / 设置 / 任务与历史"]),
    ("env.rs", ["Node · CLI · 应用配置 · 登录检测"]),
    ("lark.rs", ["子进程执行 / 超时 / 取消 / 重试 / cwd 白名单"]),
    ("extract.rs", ["单文档预览 / 导出 / 图片落盘 / 事务提交"]),
    ("wiki.rs", ["Wiki 遍历 / 选择 / 目录映射 / 导出分流"]),
    ("reader.rs", ["本地目录导航 / md 读取 / 图片 data URL"]),
    ("markdown.rs", ["图片解析 / URL 本地化替换 / 安全文件名"]),
    ("models.rs", ["可序列化数据模型（WikiNode / Progress）"]),
    ("error.rs", ["统一结构化错误协议 AppError"]),
    ("logger.rs", ["结构化运行日志与日志文件管理"]),
    ("lib.rs", ["应用初始化 / 状态恢复 / 命令注册"]),
]

EXT_BOXES = [
    (f'lark-cli {DATA["cli"]}', ["子进程调用 · 凭据由 CLI 托管，后端不保存密码/token"]),
    ("飞书开放平台", ["云端内容源 · 只读拉取，无自建服务端"]),
    ("本地输出目录", ["Markdown / 图片 / XLSX / NDJSON / 附件原样"]),
]

# ---------------------------------------------------------------- 布局
BX, BW = 60, 1160          # 色带左边界与宽度
PAD = 24                    # 色带内边距
IX = BX + PAD               # 内部起始 x = 84
IW = BW - PAD * 2           # 内部宽度 = 1112

B1 = (BX, 70, BW, 140)
B2 = (BX, 232, BW, 104)
B3 = (BX, 358, BW, 282)
B4 = (BX, 662, BW, 110)


def build() -> str:
    p = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
        f'viewBox="0 0 {W} {H}" font-family="{FONT}">',
        "<defs>",
        '<marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
        'orient="auto-start-reverse">',
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{C["dim"]}"/>',
        "</marker>",
        "</defs>",
        rect(0, 0, W, H, C["bg"], rx=0),
        t(BX, 40, "LarkReader 分层架构", size=22, fill="#FFFFFF", weight="700"),
        t(W - BX, 40, "数据快照 2026-09-05 · 实现为准", size=12, fill=C["dim"], anchor="end"),
    ]

    # ---------------- ① 前端 ----------------
    p.extend(band(*B1, "① 前端 · src/ — Vue 3.5 + TypeScript + Vite 8 + Pinia + Naive UI", C["blue"],
                  "7,161 行 · 19 .vue + 18 .ts"))
    gw, gap = 212, 12
    for i, (title, lines) in enumerate(FE_BOXES):
        x = IX + i * (gw + gap)
        p.extend(box_item(x, B1[1] + 48, gw, 68, title, lines, C["blue"]))

    # ---------------- ② Tauri IPC ----------------
    p.append(arrow_down(640, B1[1] + B1[3], B2[1]))
    p.append(t(654, B1[1] + B1[3] + 16, "invoke()", size=11, fill=C["dim"]))
    p.extend(band(*B2, "② Tauri IPC 边界", C["purple"]))
    iw = (IW - gap) // 2
    for i, (title, lines) in enumerate(IPC_BOXES):
        x = IX + i * (iw + gap)
        p.extend(box_item(x, B2[1] + 42, iw, 48, title, lines, C["purple"]))

    # ---------------- ③ Rust 后端 ----------------
    p.append(arrow_down(640, B2[1] + B2[3], B3[1]))
    p.append(t(654, B2[1] + B2[3] + 16, "命令分发", size=11, fill=C["dim"]))
    p.extend(band(*B3, "③ Rust 后端 · src-tauri/src", C["green"],
                  f'{DATA["rust_files"]} 个源文件 / {DATA["rust_loc"]:,} 行 · {DATA["unit_tests"]} 项单测通过'))
    cols = 4
    rgw = (IW - (cols - 1) * gap) // cols  # 4 列网格，容纳 12 个模块
    for i, (title, lines) in enumerate(RUST_BOXES):
        col, row = i % cols, i // cols
        x = IX + col * (rgw + gap)
        y = B3[1] + 46 + row * (62 + 16)
        p.extend(box_item(x, y, rgw, 62, title, lines, C["green"]))

    # ---------------- ④ 外部依赖 ----------------
    p.append(f'<line x1="640" y1="{B3[1] + B3[3]}" x2="640" y2="{B4[1] - 14}" stroke="{C["dim"]}" stroke-width="1.6"/>')
    p.append(f'<line x1="265" y1="{B4[1] - 14}" x2="1013" y2="{B4[1] - 14}" stroke="{C["dim"]}" stroke-width="1.6"/>')
    p.append(arrow_down(265, B4[1] - 14, B4[1] + 4))
    p.append(arrow_down(1013, B4[1] - 14, B4[1] + 4))
    p.append(t(276, B4[1] - 18, "子进程调用", size=11, fill=C["dim"]))
    p.append(t(1003, B4[1] - 18, "写盘", size=11, fill=C["dim"], anchor="end"))

    p.extend(band(*B4, "④ 外部依赖与产出", C["orange"]))
    ew = (IW - 2 * gap) // 3
    for i, (title, lines) in enumerate(EXT_BOXES):
        x = IX + i * (ew + gap)
        p.extend(box_item(x, B4[1] + 18, ew, 72, title, lines, C["orange"]))
    p.append(arrow_right(IX + ew, IX + ew + gap, B4[1] + 18 + 36))

    # ---------------- 底部横切能力 ----------------
    p.append(t(BX, H - 16,
               "横切能力：结构化错误（retryable）· 后台任务 8 阶段进度与协作式取消 · 任务历史持久化（24h / ≤100 条）· 原子写盘与配置备份回滚 · 运行日志",
               size=11.5, fill=C["dim"]))

    p.append("</svg>")
    return "\n".join(p)


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    out = root / "docs" / "assets" / "architecture.svg"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(build() + "\n", encoding="utf-8")
    print(f"[ok] wrote {out}  ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
