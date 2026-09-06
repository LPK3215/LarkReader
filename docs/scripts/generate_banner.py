#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate_banner.py — 生成 README 顶部品牌横幅

用途：README 顶部横幅，展示项目名、定位、关键版本指标与 E2E 实测数据。
依赖：仅 Python 3 标准库，无第三方包。
运行：python docs/scripts/generate_banner.py          （仓库根目录执行）
      python scripts/generate_banner.py               （任意 cwd 亦可，脚本自动定位仓库根）
输出：docs/assets/banner.svg

数值来源（改动后请同步 DATA 再重跑）：
  - version        -> package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json
  - license        -> package.json "license"
  - cli            -> src-tauri/src/env.rs  SUPPORTED_LARK_CLI_VERSION
  - commands       -> src-tauri/src 中 #[tauri::command] 的数量
  - unit_tests     -> cargo test --lib 的通过数
  - e2e_*          -> docs/e2e-download-case/README.md
"""

from pathlib import Path

W, H = 1280, 320
FONT = "'PingFang SC','Microsoft YaHei','Segoe UI',Helvetica,Arial,sans-serif"

C = {
    "bg1": "#0B1220",
    "bg2": "#17293F",
    "panel": "#16212F",
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
    "version": "0.1.0",
    "license": "GPL-3.0",
    "cli": "1.0.93",
    "commands": 27,
    "unit_tests": 26,
    "e2e_success": 38,
    "e2e_total": 38,
    "failed": 0,
    "skipped": 0,
    "files": 42,
    "extensions": 18,
}


def esc(s: str) -> str:
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def t(x, y, s, size=14, fill=C["text"], weight="400", anchor="start", extra=""):
    return (
        f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" font-weight="{weight}" '
        f'text-anchor="{anchor}" {extra}>{esc(s)}</text>'
    )


def rect(x, y, w, h, fill, rx=10, stroke="none", sw=1):
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" ry="{rx}" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'
    )


def chip(x, y, label, color, width):
    """底部信息胶囊：左侧色点 + 文本。"""
    parts = [
        rect(x, y, width, 34, "#111C2B", rx=17, stroke=color, sw=1),
        f'<circle cx="{x + 16}" cy="{y + 17}" r="4" fill="{color}"/>',
        t(x + 28, y + 22, label, size=13.5, fill=C["muted"]),
    ]
    return parts


def stat_card(x, y, w, h, value, unit, label, color):
    cy = y + h / 2
    return [
        rect(x, y, w, h, "#111C2B", rx=12, stroke=C["border"], sw=1),
        t(x + w / 2, cy + 2, value, size=30, fill=color, weight="700", anchor="middle"),
        t(x + w / 2 + 30, cy + 2, unit, size=13, fill=C["dim"], anchor="start"),
        t(x + w / 2, cy + 26, label, size=12, fill=C["muted"], anchor="middle"),
    ]


def build() -> str:
    p = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
        f'viewBox="0 0 {W} {H}" font-family="{FONT}">',
        "<defs>",
        '<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">',
        f'<stop offset="0" stop-color="{C["bg1"]}"/>',
        f'<stop offset="1" stop-color="{C["bg2"]}"/>',
        "</linearGradient>",
        '<linearGradient id="accent" x1="0" y1="0" x2="0" y2="1">',
        f'<stop offset="0" stop-color="{C["blue"]}"/>',
        f'<stop offset="1" stop-color="{C["purple"]}"/>',
        "</linearGradient>",
        "</defs>",
        rect(0, 0, W, H, "url(#bg)", rx=0),
    ]

    # 背景网格装饰
    for i in range(1, 32):
        x = i * 40
        p.append(f'<line x1="{x}" y1="0" x2="{x}" y2="{H}" stroke="#FFFFFF" stroke-opacity="0.03"/>')
    for j in range(1, 9):
        y = j * 40
        p.append(f'<line x1="0" y1="{y}" x2="{W}" y2="{y}" stroke="#FFFFFF" stroke-opacity="0.03"/>')

    # 右侧光晕
    p.append('<circle cx="1180" cy="30" r="180" fill="#4C8DFF" fill-opacity="0.07"/>')
    p.append('<circle cx="1090" cy="300" r="140" fill="#A97BFF" fill-opacity="0.06"/>')

    # ---------- 左侧主标题区 ----------
    p.append(rect(64, 84, 6, 66, "url(#accent)", rx=3))
    p.append(t(90, 140, "LarkReader", size=62, fill="#FFFFFF", weight="700"))
    p.append(t(92, 178, "飞书文档 · 本地阅读与导出工具", size=23, fill="#C7D6E6", weight="500"))
    p.append(
        t(
            92,
            208,
            "Tauri 2 + Rust + Vue 3 桌面应用 · 知识库递归导出 · 纯本地运行、数据不出机",
            size=14.5,
            fill=C["muted"],
        )
    )

    # 底部信息胶囊
    chips = [
        (f'v{DATA["version"]}', C["green"], 84),
        (DATA["license"], C["blue"], 96),
        (f'lark-cli {DATA["cli"]}', C["purple"], 140),
        (f'{DATA["commands"]} 个 IPC 命令', C["orange"], 142),
        (f'{DATA["unit_tests"]} 项单测通过', C["green"], 142),
    ]
    cx = 92
    for label, color, width in chips:
        p.extend(chip(cx, 232, label, color, width))
        cx += width + 12

    # ---------- 右侧 E2E 数据面板 ----------
    px, py, pw, ph = 830, 36, 390, 248
    p.append(rect(px, py, pw, ph, C["panel"], rx=16, stroke=C["border"], sw=1))
    p.append(t(px + 28, py + 38, "E2E 全量下载实测 · 2026-09-05", size=14, fill=C["muted"], weight="600"))

    cw, gap = 160, 14
    x1, x2 = px + 28, px + 28 + cw + gap
    p.extend(
        stat_card(
            x1, py + 52, cw, 74,
            f'{DATA["e2e_success"]}', f'/ {DATA["e2e_total"]}',
            "项成功 / 总数", C["green"],
        )
    )
    p.extend(
        stat_card(
            x2, py + 52, cw, 74,
            f'{DATA["failed"]}', "",
            "失败 · 跳过", C["blue"],
        )
    )
    p.extend(
        stat_card(
            x1, py + 140, cw, 74,
            f'{DATA["files"]}', "",
            "产物文件", C["orange"],
        )
    )
    p.extend(
        stat_card(
            x2, py + 140, cw, 74,
            f'{DATA["extensions"]}', " 种",
            "扩展名覆盖", C["purple"],
        )
    )

    p.append("</svg>")
    return "\n".join(p)


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    out = root / "docs" / "assets" / "banner.svg"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(build() + "\n", encoding="utf-8")
    print(f"[ok] wrote {out}  ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
