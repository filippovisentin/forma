"""Draws Forma's application icon (original artwork): a golden free-form NURBS
surface with its control polygon on a dark graphite tile. Writes forma.ico
(16-256 px, for forma.exe) and forma-256.png (window icon).

    python3 packaging/icon/make_icon.py   (needs Pillow)
"""
import math
import os
from PIL import Image, ImageDraw, ImageFilter

S = 1024  # drawing size; smaller sizes are downsampled
HERE = os.path.dirname(os.path.abspath(__file__))


def lerp(a, b, t):
    return a + (b - a) * t


def mix(c0, c1, t):
    return tuple(int(round(lerp(a, b, t))) for a, b in zip(c0, c1))


def tile():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        gd.line([(0, y), (S, y)], fill=mix((44, 58, 76), (17, 22, 29), y / S) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([40, 40, S - 40, S - 40], radius=210, fill=255)
    img.paste(grad, (0, 0), mask)
    # Thin light rim at the top.
    rim = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(rim).rounded_rectangle(
        [44, 44, S - 44, S - 44], radius=206, outline=(120, 140, 165, 90), width=6
    )
    img.alpha_composite(rim)
    return img


def height(u, v):
    """The free-form surface: a double wave, z over the unit square."""
    return 0.22 * math.sin(math.tau * (0.85 * u + 0.15)) * (0.55 + 0.45 * v) + 0.12 * math.sin(math.pi * v)


def project(u, v):
    """Oblique view from the front, slightly above."""
    z = height(u, v)
    return (165 + 690 * u + 70 * v, 790 - 340 * v - 330 * z)


def surface(img):
    """The surface shaded by a light from the upper left, with isocurves."""
    rows, cols = 48, 96
    d = ImageDraw.Draw(img)
    light = (-0.45, 0.35, 0.82)
    ln = math.sqrt(sum(c * c for c in light))
    light = tuple(c / ln for c in light)
    gold = [(120, 70, 16), (196, 128, 38), (236, 176, 74), (255, 222, 140), (255, 244, 205)]
    for i in range(rows):
        for j in range(cols):
            corners = [(j, i), (j + 1, i), (j + 1, i + 1), (j, i + 1)]
            quad = [project(a / cols, b / rows) for a, b in corners]
            u, v = (j + 0.5) / cols, (i + 0.5) / rows
            e = 1e-3
            du = (height(u + e, v) - height(u - e, v)) / (2 * e)
            dv = (height(u, v + e) - height(u, v - e)) / (2 * e)
            n = (-du, -dv, 1.0)
            nn = math.sqrt(sum(c * c for c in n))
            lit = max(0.0, sum(a * b / nn for a, b in zip(n, light)))
            t = min(0.999, max(0.0, lit * 1.05 - 0.05)) * (len(gold) - 1)
            k = int(t)
            col = mix(gold[k], gold[k + 1], t - k)
            d.polygon(quad, fill=col + (255,))
    iso = (110, 62, 14, 170)
    for k in range(1, 5):
        u = k / 5
        d.line([project(u, t / 40) for t in range(41)], fill=iso, width=6, joint="curve")
    for k in range(1, 3):
        v = k / 3
        d.line([project(t / 80, v) for t in range(81)], fill=iso, width=6, joint="curve")
    rim = (255, 240, 200, 255)
    for v in (0.0, 1.0):
        d.line([project(t / 100, v) for t in range(101)], fill=rim, width=11, joint="curve")
    for u in (0.0, 1.0):
        d.line([project(u, t / 40) for t in range(41)], fill=rim, width=11, joint="curve")
    # The far edge, for the control polygon above it.
    return [project(u, 1.0) for u in (0.0, 1 / 3, 2 / 3, 1.0)]


def control_polygon(img, back):
    d = ImageDraw.Draw(img)
    lifted = [(x, y - 170) for x, y in back]
    lifted[1] = (lifted[1][0], lifted[1][1] - 70)
    lifted[2] = (lifted[2][0], lifted[2][1] + 70)
    blue = (90, 170, 255, 255)
    for a, b in zip(lifted, lifted[1:]):
        d.line([a, b], fill=blue, width=10)
    for a, b in zip(lifted, back):
        for t in range(0, 10, 2):
            p = (lerp(a[0], b[0], t / 10), lerp(a[1], b[1], t / 10))
            q = (lerp(a[0], b[0], (t + 1) / 10), lerp(a[1], b[1], (t + 1) / 10))
            d.line([p, q], fill=(90, 170, 255, 150), width=6)
    for x, y in lifted:
        d.ellipse([x - 30, y - 30, x + 30, y + 30], fill=(255, 255, 255, 255), outline=blue, width=12)


def main():
    img = tile()
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    back = surface(layer)
    sh = layer.split()[3].point(lambda a: int(a * 0.45))
    shadow.paste((0, 0, 0, 255), (18, 30), sh)
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(22)))
    img.alpha_composite(layer)
    control_polygon(img, back)
    big = img.resize((256, 256), Image.LANCZOS)
    big.save(os.path.join(HERE, "forma-256.png"))
    sizes = [16, 24, 32, 48, 64, 128, 256]
    big.save(os.path.join(HERE, "forma.ico"), sizes=[(s, s) for s in sizes])
    # Preview strip (not shipped).
    strip = Image.new("RGBA", (sum(sizes) + 10 * len(sizes), 256), (128, 128, 128, 255))
    x = 0
    for s in sizes:
        strip.alpha_composite(img.resize((s, s), Image.LANCZOS), (x, 256 - s))
        x += s + 10
    strip.save(os.path.join(os.environ.get("PREVIEW_DIR", HERE), "preview.png"))


if __name__ == "__main__":
    main()
