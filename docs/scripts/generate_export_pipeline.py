#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate_export_pipeline.py — 生成导出流水线图

用途：README「导出流程」章节的流水线图（输入 → 检测 → 遍历 → 四类节点分流 → 事务落盘 → 统一结果）。
依赖：仅 Python 3 标准库，无第三方包。
运行：python docs/scripts/generate_export_pipeline.py
输出：docs/assets/export-pipeline.svg

数值来源（改动后请同步 DATA 再重跑）：
  - max_depth / max_nodes -> src-tauri/src/wiki.rs 的循环/深度/节点数保护常量
  - concurrency           -> Settings.concurrency 取值范围
  - history_*             -> src-tauri/src/commands.rs 任务历史保留策略
  - phases                -> TaskPhase 枚举的实际阶段数
"""

from pathlib import Path

W, H = 1200, 640
FONT = "'PingFang SC','Microsoft YaHei','Segoe UI',Helvetica,Arial,sans-serif"

C = {
    "bg": "#0B1220",
    "box": "#16212F",
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
    "max_depth": 64,
    "max_nodes": 10000,
    "concurrency": "1–32",
    "history_hours": 24,
    "history_max": 100,
    "phases": 8,
}


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


def arrow_down(x, y1, y2):
    return (
        f'<line x1="{x}" y1="{y1}" x2="{x}" y2="{y2 - 6}" stroke="{C["dim"]}" '
        f'stroke-width="1.6" marker-end="url(#ah)"/>'
    )


def arrow_right(x1, x2, y):
    return (
        f'<line x1="{x1}" y1="{y}" x2="{x2 - 6}" y2="{y}" stroke="{C["dim"]}" '
        f'stroke-width="1.6" marker-end="url(#ah)"/>'
    )


def line(x1, y1, x2, y2):
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{C["dim"]}" stroke-width="1.6"/>'


def node(x, y, w, h, step, title, lines, accent):
    out = [
        rect(x, y, w, h, C["box"], rx=12, stroke=accent, sw=1),
        f'<rect x="{x}" y="{y}" width="4" height="{h}" rx="2" fill="{accent}"/>',
        t(x + 20, y + 26, step, size=11.5, fill=C["dim"], weight="600"),
    ]
    cx = x + 20
    out.append(t(cx, y + 50, title, size=16, fill="#FFFFFF", weight="600"))
    for i, ln in enumerate(lines):
        out.append(t(cx, y + 68 + i * 16, ln, size=11.5, fill=C["muted"]))
    return out


def branch(x, y, w, h, kind, target, lines, accent):
    """分流卡片：类型标签 + 目标格式 + 说明。"""
    out = [
        rect(x, y, w, h, C["box"], rx=12, stroke=accent, sw=1),
        f'<rect x="{x}" y="{y}" width="4" height="{h}" rx="2" fill="{accent}"/>',
        t(x + 18, y + 28, kind, size=14, fill=accent, weight="700"),
        t(x + 18, y + 52, target, size=15.5, fill="#FFFFFF", weight="600"),
    ]
    for i, ln in enumerate(lines):
        out.append(t(x + 18, y + 74 + i * 16, ln, size=11, fill=C["muted"]))
    return out


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
        t(40, 34, "导出流水线 · 从 URL 到本地文件", size=21, fill="#FFFFFF", weight="700"),
        t(W - 40, 34, "Wiki 批量与单文档共用同一落盘链路", size=12, fill=C["dim"], anchor="end"),
    ]

    # ---------- 第一排：输入 → 检测 → 遍历 ----------
    r1y, r1w, r1h = 76, 280, 96
    r1x = [120, 460, 800]
    p.extend(node(r1x[0], r1y, r1w, r1h, "STEP 1", "输入",
                  ["Wiki / 文档 URL 或节点 token", "支持 space、单节点、父文档带子页"], C["blue"]))
    p.extend(node(r1x[1], r1y, r1w, r1h, "STEP 2", "环境与登录检测",
                  ["Node / lark-cli / 应用配置 / 登录状态", "并行执行，区分 5 种异常状态"], C["purple"]))
    p.extend(node(r1x[2], r1y, r1w, r1h, "STEP 3", "Wiki 递归遍历",
                  [f'保留目录层级与飞书排序 · 深度 ≤ {DATA["max_depth"]}',
                   f'节点上限 {DATA["max_nodes"]:,} · 选文件夹自动含后代'], C["blue"]))
    p.append(arrow_right(r1x[0] + r1w, r1x[1], r1y + 48))
    p.append(arrow_right(r1x[1] + r1w, r1x[2], r1y + 48))

    # ---------- 扇出 ----------
    r2y, r2w, r2h = 238, 250, 100
    r2x = [40, 330, 620, 910]
    centers = [x + r2w / 2 for x in r2x]
    fan_y, in_y = 204, 370
    p.append(line(940, r1y + r1h, 940, fan_y))
    p.append(line(centers[0], fan_y, centers[-1], fan_y))
    for cx in centers:
        p.append(arrow_down(cx, fan_y, r2y))

    p.extend(branch(r2x[0], r2y, r2w, r2h, "Doc", "→ Markdown + 图片",
                    [f'图片并发下载（{DATA["concurrency"]}）并本地化 URL',
                     "同名自动 (2)(3) 编号，不覆盖"], C["green"]))
    p.extend(branch(r2x[1], r2y, r2w, r2h, "Sheet", "→ XLSX",
                    ["整表导出，保留数值与表头", "写命令走 cwd 白名单规避"], C["orange"]))
    p.extend(branch(r2x[2], r2y, r2w, r2h, "Bitable", "→ NDJSON",
                    ["每张数据表一份 NDJSON", "附带同表名 .manifest.json 元数据"], C["purple"]))
    p.extend(branch(r2x[3], r2y, r2w, r2h, "File 附件", "→ 原样字节",
                    ["位置前缀 + 原标题 + 原扩展名", "字节级往返一致，支持 zip/pdf/docx"], C["orange"]))

    # ---------- 扇入 ----------
    r3y, r3w, r3h = 396, 560, 96
    r3x, r3c = 320, 600
    for cx in centers:
        p.append(line(cx, r2y + r2h, cx, in_y))
    p.append(line(centers[0], in_y, r3c, in_y))
    p.append(arrow_down(r3c, in_y, r3y))

    p.extend(node(r3x, r3y, r3w, r3h, "STEP 4", "事务提交与安全落盘",
                  ["先写临时文件 → 原子 rename 落盘 → 目录预检可写性 + 磁盘空间",
                   "失败回滚，不会留下半截文件"], C["green"]))

    # ---------- 结果 ----------
    r4y, r4w, r4h = 516, 760, 96
    r4x = 220
    p.append(arrow_down(r3c, r3y + r3h, r4y))
    p.extend(node(r4x, r4y, r4w, r4h, "STEP 5", "统一结果与任务体验",
                  [f'items 统一列表 · 后台任务 {DATA["phases"]} 阶段进度 · 协作式取消 · 部分成功如实上报',
                   f'历史持久化：最近 {DATA["history_hours"]} 小时 / 最多 {DATA["history_max"]} 条'],
                  C["blue"]))

    p.append("</svg>")
    return "\n".join(p)


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    out = root / "docs" / "assets" / "export-pipeline.svg"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(build() + "\n", encoding="utf-8")
    print(f"[ok] wrote {out}  ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
