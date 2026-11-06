import math
import os
import struct
import subprocess

from PIL import Image, ImageDraw, ImageFont

ROOT = r"C:\Users\xiexi\cursor-byok-repair"
CONCEPTS = os.path.join(ROOT, "design", "icon-concepts")
OUT = os.path.join(CONCEPTS, "app-icon")
os.makedirs(OUT, exist_ok=True)

CHROME = r"C:\Program Files\Google\Chrome\Application\chrome.exe"
FONT_CN = r"C:\Windows\Fonts\msyh.ttc"
FONT_EN = r"C:\Windows\Fonts\segoeui.ttf"

SVG_PATH = os.path.join(CONCEPTS, "svg", "01-neon-cursor.svg")
with open(SVG_PATH, "r", encoding="utf-8") as f:
    SVG = f.read()

SIZES = [1024, 512, 256, 128, 64, 48, 32, 24, 16]


def render_master():
    html = f'''<!doctype html><html><head><meta charset="utf-8">
<style>html,body{{margin:0;padding:0;background:#FFFFFF;overflow:hidden}}svg{{display:block}}</style>
</head><body>{SVG}</body></html>'''
    html_path = os.path.join(OUT, "_render.html")
    raw_path = os.path.join(OUT, "_master.raw.png")
    with open(html_path, "w", encoding="utf-8") as f:
        f.write(html)
    subprocess.run([
        CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
        "--force-device-scale-factor=1", "--window-size=1024,1024",
        "--screenshot=" + raw_path, "file:///" + html_path.replace("\\", "/"),
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.remove(html_path)

    ss = 4
    big = Image.new("L", (1024 * ss, 1024 * ss), 0)
    d = ImageDraw.Draw(big)
    d.rounded_rectangle([32 * ss, 32 * ss, 992 * ss, 992 * ss], radius=224 * ss, fill=255)
    mask = big.resize((1024, 1024), Image.LANCZOS)
    im = Image.open(raw_path).convert("RGBA")
    im.putalpha(mask)
    os.remove(raw_path)
    return im


master = render_master()
master.save(os.path.join(OUT, "icon-1024.png"))

frames = {}
for size in SIZES:
    im = master.resize((size, size), Image.LANCZOS)
    frames[size] = im
    im.save(os.path.join(OUT, f"icon-{size}.png"))

# ---------- multi-size ICO (PNG/BMP handled by Pillow) ----------
ico_sizes = [16, 24, 32, 48, 64, 128, 256]
master.save(os.path.join(OUT, "icon.ico"), format="ICO", sizes=[(s, s) for s in ico_sizes])

# ---------- ICNS container ----------
icns_types = [
    (b"icp4", 16), (b"icp5", 32), (b"icp6", 64),
    (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024),
    (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512),
]
chunks = []
for tag, size in icns_types:
    png = os.path.join(OUT, f"_icns_{tag.decode()}_{size}.png")
    frames[size].save(png)
    with open(png, "rb") as f:
        data = f.read()
    os.remove(png)
    chunks.append(tag + struct.pack(">I", len(data) + 8) + data)
body = b"".join(chunks)
icns = b"icns" + struct.pack(">I", len(body) + 8) + body
with open(os.path.join(OUT, "icon.icns"), "wb") as f:
    f.write(icns)

# ---------- preview: all sizes on dark / light, small ones enlarged x2 ----------
prev_w, prev_h = 1100, 800
prev = Image.new("RGB", (prev_w, prev_h), "#0C0F14")
d = ImageDraw.Draw(prev)
d.text((prev_w // 2, 34), "Cursor 助手 · 新图标 01 霓虹光标 · 全尺寸", font=ImageFont.truetype(FONT_CN, 30),
       fill="#F3F5F9", anchor="mm")

rows = [("#171B23", 66, "#8A93A3"), ("#F5F6F8", 400, "#5A6473")]
show = [256, 128, 64, 48, 32, 24, 16]
for bgc, y, fill in rows:
    d.rounded_rectangle([16, y, prev_w - 16, y + 320], radius=16, fill=bgc)
    x = 40
    for size in show:
        im = frames[size]
        draw_size = size if size >= 64 else size * 2
        s = im.resize((draw_size, draw_size), Image.NEAREST if size < 64 else Image.LANCZOS)
        prev.paste(s, (x, y + 16 + (256 - draw_size) // 2), s)
        d.text((x + draw_size // 2, y + 288), f"{size}px", font=ImageFont.truetype(FONT_EN, 18), fill=fill, anchor="mm")
        if size < 64:
            d.text((x + draw_size // 2, y + 308), "(x2)", font=ImageFont.truetype(FONT_EN, 14), fill=fill, anchor="mm")
        x += draw_size + 26
prev.save(os.path.join(OUT, "icon-preview.png"))

print("sizes:", ", ".join(str(s) for s in SIZES))
print("ico bytes:", os.path.getsize(os.path.join(OUT, "icon.ico")))
print("icns bytes:", os.path.getsize(os.path.join(OUT, "icon.icns")))
