#!/usr/bin/env python3
"""提取指定文档列表：文本+图片，自动更新本地引用"""
import subprocess
import json
import os
import re
import sys

OUTPUT_DIR = sys.argv[1] if len(sys.argv) > 1 else "."
WIKI_BASE = "https://gcnyv4rcw1jv.feishu.cn/wiki/"

# 从命令行参数读取文档列表: node_token,title
DOCS = []
for arg in sys.argv[2:]:
    parts = arg.split("|", 1)
    if len(parts) == 2:
        DOCS.append((parts[0], parts[1]))

def safe_filename(name):
    name = re.sub(r'[\\/:*?"<>|]', '_', name)
    return name.strip()[:100]

def fetch_doc(node_token):
    url = WIKI_BASE + node_token
    cmd = ["lark-cli", "docs", "+fetch", "--doc", url, "--doc-format", "markdown", "--as", "user"]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if result.returncode != 0:
        return None, result.stderr
    try:
        data = json.loads(result.stdout)
        if data.get("ok"):
            return data["data"]["document"]["content"], None
        return None, str(data)
    except Exception as e:
        return None, f"解析失败: {e}"

def extract_images(content):
    return re.findall(r'!\[([^\]]*)\]\(([^)]+)\)', content)

def preview_image(token, output_path):
    cmd = ["lark-cli", "docs", "+media-preview", "--token", token, "--output", output_path, "--as", "user"]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if result.returncode != 0:
        return False, None, result.stderr
    try:
        data = json.loads(result.stdout)
        if data.get("ok"):
            return True, data["data"]["saved_path"], None
        return False, None, str(data)
    except Exception as e:
        return False, None, f"解析失败: {e}"

def main():
    summary = []
    for node_token, title in DOCS:
        print(f"提取: {title} ...", flush=True)
        content, err = fetch_doc(node_token)
        if err:
            print(f"  ❌ 失败: {err[:100]}", flush=True)
            summary.append({"title": title, "status": "failed", "error": err})
            continue

        filename = safe_filename(title) + ".md"
        filepath = os.path.join(OUTPUT_DIR, filename)
        with open(filepath, "w", encoding="utf-8") as f:
            f.write(content)

        images = extract_images(content)
        img_count = len(images)

        # 下载图片
        if img_count > 0:
            img_dir_name = os.path.splitext(filename)[0] + "_images"
            img_dir = os.path.join(OUTPUT_DIR, img_dir_name)
            os.makedirs(img_dir, exist_ok=True)

            for i, (desc, url) in enumerate(images):
                token = url.rstrip('/').split('/')[-1]
                output_base = os.path.join(img_dir, f"img_{i+1:02d}")
                print(f"  图片 {i+1}/{img_count}: {token} ...", end='', flush=True)
                success, saved_path, err = preview_image(token, output_base)
                if success and saved_path and os.path.exists(saved_path):
                    ext = os.path.splitext(saved_path)[1]
                    if os.path.dirname(saved_path) != img_dir:
                        final_path = os.path.join(img_dir, f"img_{i+1:02d}{ext}")
                        os.rename(saved_path, final_path)
                    else:
                        final_path = saved_path
                    local_ref = f"{img_dir_name}/img_{i+1:02d}{ext}"
                    content = content.replace(url, local_ref)
                    size_kb = os.path.getsize(final_path) / 1024
                    print(f" ✅ ({size_kb:.0f}KB)", flush=True)
                else:
                    print(f" ❌ {str(err)[:60]}", flush=True)

            with open(filepath, "w", encoding="utf-8") as f:
                f.write(content)

        print(f"  ✅ 完成: {len(content)} 字符, {img_count} 张图片", flush=True)
        summary.append({"title": title, "filename": filename, "status": "success", "char_count": len(content), "image_count": img_count})

    print(f"\n{'='*50}")
    success = [s for s in summary if s["status"] == "success"]
    print(f"成功: {len(success)}/{len(summary)}")
    print(f"{'='*50}")

if __name__ == "__main__":
    main()
