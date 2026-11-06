import base64
import math
import os
import subprocess

from PIL import Image, ImageDraw, ImageFont

ROOT = r"C:\Users\xiexi\cursor-byok-repair\design\icon-concepts"
SVG_DIR = os.path.join(ROOT, "svg")
PNG_DIR = os.path.join(ROOT, "png")
os.makedirs(SVG_DIR, exist_ok=True)
os.makedirs(PNG_DIR, exist_ok=True)

CHROME = r"C:\Program Files\Google\Chrome\Application\chrome.exe"
FONT_CN = r"C:\Windows\Fonts\msyh.ttc"
FONT_EN = r"C:\Windows\Fonts\segoeui.ttf"


def spark(cx, cy, s, fill, opacity=1.0):
    k = 0.22
    return (
        f'<path d="M {cx} {cy - s} '
        f'C {cx + k * s} {cy - k * s} {cx + k * s} {cy - k * s} {cx + s} {cy} '
        f'C {cx + k * s} {cy + k * s} {cx + k * s} {cy + k * s} {cx} {cy + s} '
        f'C {cx - k * s} {cy + k * s} {cx - k * s} {cy + k * s} {cx - s} {cy} '
        f'C {cx - k * s} {cy - k * s} {cx - k * s} {cy - k * s} {cx} {cy - s} Z" '
        f'fill="{fill}" opacity="{opacity}"/>'
    )


def arc_path(cx, cy, r, start_deg, end_deg):
    a0 = math.radians(start_deg)
    a1 = math.radians(end_deg)
    x0, y0 = cx + r * math.cos(a0), cy + r * math.sin(a0)
    x1, y1 = cx + r * math.cos(a1), cy + r * math.sin(a1)
    large = 1 if (end_deg - start_deg) % 360 > 180 else 0
    return f'M {x0:.2f} {y0:.2f} A {r} {r} 0 {large} 1 {x1:.2f} {y1:.2f}'


def svg_shell(inner, defs, title=""):
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 512 512" role="img" aria-label="{title}">
<defs>
{defs}
</defs>
{inner}
</svg>'''


BG_A = '''<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#070B14"/>
  <stop offset="1" stop-color="#111A2E"/>
</linearGradient>
<radialGradient id="glow" cx="0.42" cy="0.44" r="0.62">
  <stop offset="0" stop-color="#22D3EE" stop-opacity="0.38"/>
  <stop offset="0.55" stop-color="#6366F1" stop-opacity="0.16"/>
  <stop offset="1" stop-color="#6366F1" stop-opacity="0"/>
</radialGradient>
<linearGradient id="arrow" x1="0" y1="0" x2="0.4" y2="1">
  <stop offset="0" stop-color="#FFFFFF"/>
  <stop offset="1" stop-color="#CDD8FF"/>
</linearGradient>
<linearGradient id="spark" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#22D3EE"/>
  <stop offset="1" stop-color="#A855F7"/>
</linearGradient>
<filter id="soft" x="-40%" y="-40%" width="180%" height="180%">
  <feDropShadow dx="0" dy="10" stdDeviation="16" flood-color="#000000" flood-opacity="0.45"/>
</filter>
<filter id="blur" x="-60%" y="-60%" width="220%" height="220%">
  <feGaussianBlur stdDeviation="14"/>
</filter>'''

INNER_1 = '''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="238" cy="252" r="210" fill="url(#glow)"/>
<rect x="16" y="16" width="480" height="480" rx="112" fill="none" stroke="#FFFFFF" stroke-opacity="0.09" stroke-width="2"/>
<g filter="url(#soft)">
  <path d="M178 132 L178 344 L232 292 L268 372 L310 354 L274 276 L346 276 Z"
        fill="url(#arrow)" stroke="url(#arrow)" stroke-width="10" stroke-linejoin="round"/>
</g>
<g filter="url(#blur)" opacity="0.55">''' + spark(368, 172, 52, "#8B5CF6") + '''</g>
''' + spark(368, 172, 42, "url(#spark)") + '''
<circle cx="332" cy="132" r="7" fill="#22D3EE" opacity="0.9"/>
'''

BG_B = '''<radialGradient id="bg" cx="0.5" cy="0.45" r="0.75">
  <stop offset="0" stop-color="#18233F"/>
  <stop offset="1" stop-color="#060910"/>
</radialGradient>
<linearGradient id="ring" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#22D3EE"/>
  <stop offset="0.55" stop-color="#818CF8"/>
  <stop offset="1" stop-color="#C084FC"/>
</linearGradient>
<linearGradient id="arrow" x1="0" y1="0" x2="0.3" y2="1">
  <stop offset="0" stop-color="#FFFFFF"/>
  <stop offset="1" stop-color="#D7DFFF"/>
</linearGradient>
<radialGradient id="core" cx="0.5" cy="0.5" r="0.5">
  <stop offset="0" stop-color="#818CF8" stop-opacity="0.45"/>
  <stop offset="1" stop-color="#818CF8" stop-opacity="0"/>
</radialGradient>
<filter id="soft" x="-40%" y="-40%" width="180%" height="180%">
  <feDropShadow dx="0" dy="8" stdDeviation="14" flood-color="#000000" flood-opacity="0.5"/>
</filter>
<filter id="blur" x="-60%" y="-60%" width="220%" height="220%">
  <feGaussianBlur stdDeviation="12"/>
</filter>'''

INNER_2 = '''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="256" cy="256" r="170" fill="url(#core)"/>
<g filter="url(#blur)" opacity="0.75">
  <circle cx="256" cy="256" r="128" fill="none" stroke="url(#ring)" stroke-width="34"
          stroke-dasharray="660 145" transform="rotate(112 256 256)"/>
</g>
<circle cx="256" cy="256" r="128" fill="none" stroke="url(#ring)" stroke-width="30"
        stroke-dasharray="660 145" transform="rotate(112 256 256)" stroke-linecap="round"/>
<circle cx="256" cy="256" r="82" fill="none" stroke="#FFFFFF" stroke-opacity="0.10" stroke-width="2"/>
<g filter="url(#soft)" transform="rotate(18 256 256)">
  <path d="M196 158 L196 348 L246 300 L280 372 L316 356 L282 284 L350 284 Z"
        fill="url(#arrow)" stroke="url(#arrow)" stroke-width="10" stroke-linejoin="round"/>
</g>
<circle cx="156" cy="158" r="8" fill="#22D3EE"/>
<circle cx="362" cy="352" r="6" fill="#C084FC" opacity="0.9"/>
'''

BG_C = '''<radialGradient id="bg" cx="0.7" cy="0.2" r="0.9">
  <stop offset="0" stop-color="#151A33"/>
  <stop offset="0.6" stop-color="#0A0C16"/>
  <stop offset="1" stop-color="#05060A"/>
</radialGradient>
<linearGradient id="c" x1="0.1" y1="0.9" x2="0.9" y2="0.1">
  <stop offset="0" stop-color="#34D399"/>
  <stop offset="0.5" stop-color="#22D3EE"/>
  <stop offset="1" stop-color="#6366F1"/>
</linearGradient>
<radialGradient id="cg" cx="0.5" cy="0.45" r="0.55">
  <stop offset="0" stop-color="#22D3EE" stop-opacity="0.22"/>
  <stop offset="1" stop-color="#22D3EE" stop-opacity="0"/>
</radialGradient>
<filter id="blur" x="-60%" y="-60%" width="220%" height="220%">
  <feGaussianBlur stdDeviation="18"/>
</filter>'''

INNER_3 = f'''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="236" cy="250" r="200" fill="url(#cg)"/>
<g filter="url(#blur)" opacity="0.6">
  <path d="{arc_path(246, 260, 140, 40, 320)}" fill="none" stroke="url(#c)" stroke-width="68" stroke-linecap="round"/>
</g>
<path d="{arc_path(246, 260, 140, 40, 320)}" fill="none" stroke="url(#c)" stroke-width="62" stroke-linecap="round"/>
{spark(388, 158, 34, "#FFFFFF")}
<circle cx="410" cy="212" r="6" fill="#22D3EE"/>
'''

BG_D = '''<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#121635"/>
  <stop offset="1" stop-color="#070912"/>
</linearGradient>
<linearGradient id="bubble" x1="0.15" y1="0" x2="0.85" y2="1">
  <stop offset="0" stop-color="#8B5CF6"/>
  <stop offset="0.55" stop-color="#6366F1"/>
  <stop offset="1" stop-color="#3B82F6"/>
</linearGradient>
<linearGradient id="arrow" x1="0" y1="0" x2="0.3" y2="1">
  <stop offset="0" stop-color="#FFFFFF"/>
  <stop offset="1" stop-color="#E0E7FF"/>
</linearGradient>
<radialGradient id="glow" cx="0.5" cy="0.45" r="0.6">
  <stop offset="0" stop-color="#8B5CF6" stop-opacity="0.35"/>
  <stop offset="1" stop-color="#8B5CF6" stop-opacity="0"/>
</radialGradient>
<filter id="soft" x="-40%" y="-40%" width="180%" height="180%">
  <feDropShadow dx="0" dy="12" stdDeviation="18" flood-color="#000000" flood-opacity="0.45"/>
</filter>'''

INNER_D = '''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="256" cy="262" r="200" fill="url(#glow)"/>
<g filter="url(#soft)">
  <rect x="86" y="122" width="340" height="248" rx="70" fill="url(#bubble)"/>
  <path d="M156 358 L156 424 L222 366 Z" fill="#5B5BF0"/>
</g>
<rect x="86" y="122" width="340" height="248" rx="70" fill="none" stroke="#FFFFFF" stroke-opacity="0.22" stroke-width="3"/>
<path d="M206 180 L206 340 L248 298 L278 364 L312 348 L282 284 L342 284 Z"
      fill="url(#arrow)" stroke="url(#arrow)" stroke-width="9" stroke-linejoin="round"/>
''' + spark(356, 196, 28, "#FFFFFF") + '''
<circle cx="140" cy="188" r="5" fill="#FFFFFF" opacity="0.55"/>
'''

BG_E = '''<radialGradient id="bg" cx="0.4" cy="0.4" r="0.8">
  <stop offset="0" stop-color="#0E2030"/>
  <stop offset="0.6" stop-color="#070C14"/>
  <stop offset="1" stop-color="#04060A"/>
</radialGradient>
<linearGradient id="line" x1="0" y1="0" x2="1" y2="1">
  <stop offset="0" stop-color="#22D3EE"/>
  <stop offset="1" stop-color="#818CF8"/>
</linearGradient>
<radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">
  <stop offset="0" stop-color="#22D3EE" stop-opacity="0.3"/>
  <stop offset="1" stop-color="#22D3EE" stop-opacity="0"/>
</radialGradient>
<filter id="blur" x="-60%" y="-60%" width="220%" height="220%">
  <feGaussianBlur stdDeviation="10"/>
</filter>'''

INNER_E = '''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="250" cy="250" r="190" fill="url(#glow)"/>
<path d="M168 148 L168 336 L214 292 L250 372 L292 354 L256 278 L330 278 Z"
      fill="#FFFFFF" fill-opacity="0.07" stroke="url(#line)" stroke-width="7" stroke-linejoin="round"/>
<g stroke="url(#line)" stroke-width="6" stroke-linecap="round">
  <line x1="168" y1="148" x2="330" y2="278"/>
  <line x1="168" y1="336" x2="250" y2="372"/>
  <line x1="214" y1="292" x2="168" y2="148" stroke-opacity="0.5"/>
</g>
<g fill="#0A1522" stroke="url(#line)" stroke-width="7">
  <circle cx="168" cy="148" r="13"/>
  <circle cx="168" cy="336" r="13"/>
  <circle cx="214" cy="292" r="13"/>
  <circle cx="250" cy="372" r="13"/>
  <circle cx="292" cy="354" r="13"/>
  <circle cx="256" cy="278" r="13"/>
  <circle cx="330" cy="278" r="13"/>
</g>
<g filter="url(#blur)" opacity="0.8">
  <circle cx="330" cy="278" r="22" fill="#22D3EE"/>
</g>
<circle cx="330" cy="278" r="7" fill="#E0F7FF"/>
<line x1="330" y1="278" x2="392" y2="196" stroke="#22D3EE" stroke-width="4" stroke-dasharray="3 10" stroke-linecap="round" opacity="0.85"/>
<circle cx="392" cy="196" r="9" fill="#0A1522" stroke="#22D3EE" stroke-width="5"/>
<circle cx="392" cy="196" r="3" fill="#E0F7FF"/>
'''

BG_F = '''<radialGradient id="bg" cx="0.5" cy="0.38" r="0.8">
  <stop offset="0" stop-color="#131A18"/>
  <stop offset="1" stop-color="#070909"/>
</radialGradient>
<linearGradient id="green" x1="0.1" y1="0.1" x2="0.9" y2="0.9">
  <stop offset="0" stop-color="#6EE7B7"/>
  <stop offset="0.55" stop-color="#34D399"/>
  <stop offset="1" stop-color="#059669"/>
</linearGradient>
<radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">
  <stop offset="0" stop-color="#34D399" stop-opacity="0.25"/>
  <stop offset="1" stop-color="#34D399" stop-opacity="0"/>
</radialGradient>
<filter id="soft" x="-40%" y="-40%" width="180%" height="180%">
  <feDropShadow dx="0" dy="10" stdDeviation="16" flood-color="#000000" flood-opacity="0.5"/>
</filter>'''

INNER_F = '''
<rect x="16" y="16" width="480" height="480" rx="112" fill="url(#bg)"/>
<circle cx="252" cy="256" r="200" fill="url(#glow)"/>
<g filter="url(#soft)" transform="rotate(-6 256 256)">
  <path d="M184 144 L184 352 L238 300 L274 374 L314 356 L280 284 L350 284 Z"
        fill="url(#green)" stroke="url(#green)" stroke-width="16" stroke-linejoin="round"/>
</g>
''' + spark(372, 160, 30, "#FFFFFF") + '''
<circle cx="340" cy="140" r="6" fill="#6EE7B7"/>
'''

ICONS = [
    ("01-neon-cursor", "霓虹光标", "Neon Cursor", BG_A, INNER_1),
    ("02-gateway-ring", "网关之门", "Gateway Ring", BG_B, INNER_2),
    ("03-monogram-c", "字母 C", "Monogram C", BG_C, INNER_3),
    ("04-chat-assistant", "对话助手", "Chat Assistant", BG_D, INNER_D),
    ("05-node-cursor", "节点箭头", "Node Cursor", BG_E, INNER_E),
    ("06-minimal-green", "极简绿箭", "Minimal Green", BG_F, INNER_F),
]


def render(svg_text, out_png):
    html = f'''<!doctype html><html><head><meta charset="utf-8">
<style>html,body{{margin:0;padding:0;background:#FFFFFF;overflow:hidden}}svg{{display:block}}</style>
</head><body>{svg_text}</body></html>'''
    html_path = out_png + ".html"
    with open(html_path, "w", encoding="utf-8") as f:
        f.write(html)
    subprocess.run([
        CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
        "--force-device-scale-factor=1", "--window-size=1024,1024",
        "--screenshot=" + out_png, "file:///" + html_path.replace("\\", "/"),
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.remove(html_path)


def rounded_mask(size=1024, margin=32, radius=224, ss=4):
    big = Image.new("L", (size * ss, size * ss), 0)
    d = ImageDraw.Draw(big)
    d.rounded_rectangle([margin * ss, margin * ss, (size - margin) * ss, (size - margin) * ss],
                        radius=radius * ss, fill=255)
    return big.resize((size, size), Image.LANCZOS)


MASK = rounded_mask()

print("rendering icons...")
for slug, cn, en, defs, inner in ICONS:
    svg = svg_shell(inner, defs, cn)
    svg_path = os.path.join(SVG_DIR, slug + ".svg")
    with open(svg_path, "w", encoding="utf-8") as f:
        f.write(svg)
    raw = os.path.join(PNG_DIR, slug + ".raw.png")
    render(svg, raw)
    im = Image.open(raw).convert("RGBA")
    im.putalpha(MASK)
    im.save(os.path.join(PNG_DIR, slug + ".png"))
    os.remove(raw)
    print("  ok", slug)

# ---------- contact sheet ----------
sheet_w, sheet_h = 1280, 1160
sheet = Image.new("RGB", (sheet_w, sheet_h), "#0C0F14")
d = ImageDraw.Draw(sheet)
title_font = ImageFont.truetype(FONT_CN, 40)
sub_font = ImageFont.truetype(FONT_EN, 20)
label_font = ImageFont.truetype(FONT_CN, 26)
small_font = ImageFont.truetype(FONT_CN, 17)
d.text((sheet_w // 2, 44), "Cursor 助手 · 桌面图标方案", font=title_font, fill="#F3F5F9", anchor="mm")
d.text((sheet_w // 2, 84), "6 concepts · 512×512 · dark tile", font=sub_font, fill="#7C8799", anchor="mm")

cell_w, cell_h = 400, 430
x0, y0 = 40, 120
for idx, (slug, cn, en, defs, inner) in enumerate(ICONS):
    r, c = divmod(idx, 3)
    cx = x0 + c * cell_w
    cy = y0 + r * cell_h
    im = Image.open(os.path.join(PNG_DIR, slug + ".png")).convert("RGBA").resize((256, 256), Image.LANCZOS)
    tile = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
    tile.alpha_composite(im)
    sheet.paste(tile, (cx + 72, cy), tile)
    d.text((cx + 200, cy + 286), f"{idx + 1:02d}  {cn}", font=label_font, fill="#EDF1F7", anchor="mm")
    d.text((cx + 200, cy + 318), en, font=sub_font, fill="#6E7B8F", anchor="mm")
    # small size preview
    for j, size in enumerate((32, 48)):
        s = im.resize((size, size), Image.LANCZOS)
        sheet.paste(s, (cx + 150 + j * 60, cy + 340), s)
    d.text((cx + 200, cy + 404), "32px / 48px", font=small_font, fill="#59667A", anchor="mm")

sheet.save(os.path.join(ROOT, "contact-sheet.png"))
print("saved contact sheet")

# ---------- light/dark size sheet ----------
col_w = 168
sheet2_w = 6 * col_w + 40
sheet2 = Image.new("RGB", (sheet2_w, 760), "#0C0F14")
d2 = ImageDraw.Draw(sheet2)
d2.text((sheet2_w // 2, 36), "不同尺寸 · 深色 / 浅色背景", font=ImageFont.truetype(FONT_CN, 28),
        fill="#F3F5F9", anchor="mm")
d2.text((sheet2_w // 2, 68), "128 / 64 / 48 / 32 px", font=ImageFont.truetype(FONT_EN, 18),
        fill="#7C8799", anchor="mm")

blocks = [("#161A22", 92, "#8A93A3"), ("#F5F6F8", 420, "#5A6473")]
for bgc, y, label_fill in blocks:
    d2.rounded_rectangle([14, y, sheet2_w - 14, y + 300], radius=18, fill=bgc)
    for ci, (slug, cn, en, defs, inner) in enumerate(ICONS):
        im = Image.open(os.path.join(PNG_DIR, slug + ".png")).convert("RGBA")
        px = 20 + ci * col_w
        big = im.resize((128, 128), Image.LANCZOS)
        sheet2.paste(big, (px + (col_w - 128) // 2, y + 16), big)
        sx = px + 10
        for size in (64, 48, 32):
            s = im.resize((size, size), Image.LANCZOS)
            sheet2.paste(s, (sx, y + 170 + (64 - size) // 2), s)
            sx += size + 12
        d2.text((px + col_w // 2, y + 266), f"{ci + 1:02d}  {cn}",
                font=ImageFont.truetype(FONT_CN, 20), fill=label_fill, anchor="mm")
sheet2.save(os.path.join(ROOT, "sizes-preview.png"))
print("saved sizes preview")
