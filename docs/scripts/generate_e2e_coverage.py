#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate_e2e_coverage.py — 生成 E2E 覆盖统计图

用途：README「验证与实测」章节的统计卡 + 扩展名分布条形图。
依赖：仅 Python 3 标准库，无第三方包。
运行：python docs/scripts/generate_e2e_coverage.py
输出：docs/assets/e2e-coverage.svg

数值来源（改动后请同步 DATA 再重跑）：
  docs/e2e-download-case/README.md
  find docs/e2e-download-case -type f ! -name 'README.md' | wc -l
  find docs/e2e-download-case -mindepth 1 -type d | wc -l
  du -sk docs/e2e-download-case
"""

from pathlib import Path

W, H = 1200, 620
FONT = "'PingFang SC','Microsoft YaHei','Segoe UI',Helvetica,Arial,sans-serif"

C = {
    "bg": "#0B1220",
    "card": "#16212F",
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
    "snapshot": "2026-09-05",
    "space_id": "7681927602327538869",
    "top_nodes": 8,
    "success": 38,
    "total": 38,
    "failed": 0,
    "skipped": 0,
    "files": 42,
    "extensions": 18,
    "dirs": 10,
    "size_kb": 1644,
    "attachments": 18,
}

BARS = [
    (".md", 20, "18 篇 docx 正文 + 2 个 md 格式附件", C["green"]),
    (".png", 4, "3 张正文图片 + 1 个图片附件", C["orange"]),
    (".xlsx", 2, "Sheet 导出 1 + 表格附件 1", C["blue"]),
    (".json", 2, "附件 config.json + Bitable manifest", C["purple"]),
    ("其他 14 种", 14, "csv / xml / log / txt / zip / pdf / docx … 各 1", C["orange"]),
]


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


def stat_card(x, y, w, h, value, unit, title, note, color):
    cy = y + h / 2
    return [
        rect(x, y, w, h, C["card"], rx=14, stroke=C["border"], sw=1),
        f'<rect x="{x}" y="{y + 16}" width="4" height="{h - 32}" rx="2" fill="{color}"/>',
        t(x + 24, cy - 12, value, size=38, fill=color, weight="700"),
        t(x + 24 + len(str(value)) * 23 + 4, cy - 12, unit, size=14, fill=C["dim"]),
        t(x + 24, cy + 16, title, size=14, fill=C["text"], weight="600"),
        t(x + 24, cy + 36, note, size=11, fill=C["muted"]),
    ]


def build() -> str:
    p = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
        f'viewBox="0 0 {W} {H}" font-family="{FONT}">',
        rect(0, 0, W, H, C["bg"], rx=0),
        t(30, 40, "E2E 全量下载实测覆盖", size=21, fill="#FFFFFF", weight="700"),
        t(W - 30, 40, f'LarkReader-E2E-测试库 · space_id {DATA["space_id"]} · 快照 {DATA["snapshot"]}',
          size=11.5, fill=C["dim"], anchor="end"),
    ]

    # ---------- 统计卡 3 x 2 ----------
    cw, ch, gap = 360, 110, 30
    cards = [
        (f'{DATA["top_nodes"]}', "", "顶层根节点", "一个 space 下 8 个互不相连的根，逐个递归导出", C["blue"]),
        (f'{DATA["success"]}', f'/ {DATA["total"]}', "项成功 / 总数", "18 个 docx 正文 + 20 个特殊导出", C["green"]),
        (f'{DATA["failed"]}', "", "失败 · 跳过", f'不支持节点 0 项，附件 {DATA["attachments"]} 个全部成功', C["green"]),
        (f'{DATA["files"]}', "", "产物文件", f'{DATA["dirs"]} 个子目录 · 约 {DATA["size_kb"] / 1024:.1f} MB', C["orange"]),
        (f'{DATA["extensions"]}', " 种", "扩展名覆盖", "覆盖当前支持的全部导出形态", C["purple"]),
        ("100", " %", "字节级一致", "文件附件下载字节数与上传一致", C["blue"]),
    ]
    for i, (value, unit, title, note, color) in enumerate(cards):
        col, row = i % 3, i // 3
        x = 30 + col * (cw + gap)
        y = 70 + row * (ch + gap)
        p.extend(stat_card(x, y, cw, ch, value, unit, title, note, color))

    # ---------- 扩展名分布 ----------
    p.append(t(30, 360, f'扩展名分布（共 {DATA["files"]} 个产物文件）', size=15, fill=C["text"], weight="600"))
    max_val = max(v for _, v, _, _ in BARS)
    max_w = 620
    x0 = 210
    y0 = 384
    for i, (name, val, note, color) in enumerate(BARS):
        y = y0 + i * 44
        bw = int(val / max_val * max_w)
        p.append(t(x0 - 14, y + 18, name, size=13, fill=C["text"], weight="600", anchor="end"))
        p.append(rect(x0, y, max_w, 26, "#111B28", rx=6))
        p.append(rect(x0, y, bw, 26, color, rx=6, stroke="none"))
        p.append(t(x0 + bw + 12, y + 19, str(val), size=13, fill=color, weight="700"))
        p.append(t(x0 + max_w + 56, y + 19, note, size=11, fill=C["muted"]))

    p.append(t(30, H - 18,
               f'数据来源：docs/e2e-download-case/ · 结果 {DATA["success"]} 项成功 / {DATA["failed"]} 失败 / {DATA["skipped"]} 跳过',
               size=11.5, fill=C["dim"]))

    p.append("</svg>")
    return "\n".join(p)


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    out = root / "docs" / "assets" / "e2e-coverage.svg"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(build() + "\n", encoding="utf-8")
    print(f"[ok] wrote {out}  ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
