#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Generates the notenwart brand bundle: master SVGs, PNG exports, favicon.ico, web icons and social images.

Needs rsvg-convert (Homebrew: librsvg). The social image and the lockup PNGs use the Charter font (bundled with
macOS, free as Bitstream Charter elsewhere). Run from anywhere:
    python3 brand/generate.py
Edit this file, not the exported files.
"""
import os, shutil, struct, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
SVG, PNG, WEB, SOCIAL = (os.path.join(HERE, d) for d in ("svg", "png", "web", "social"))

NAVY = "#1D2B45"         # ink: outline, lines, dark backgrounds
CREAM = "#F5EFE3"        # paper: light backgrounds, ink on dark
BRASS = "#B8862E"        # the note on light backgrounds
BRASS_LIGHT = "#D9A441"  # the note on dark backgrounds
SERIF = "Charter, 'Bitstream Charter', 'Iowan Old Style', Georgia, serif"

# The shield: a gentle arch on top, so it reads as a crest rather than a security badge.
SHIELD = ("M512 150 C620 196 720 222 820 232 V500 C820 690 690 818 512 884 "
          "C334 818 204 690 204 500 V232 C304 222 404 196 512 150 Z")

# Regular mark: five staff lines; small mark (16-48 px): three heavier lines, heavier outline, larger note.
REGULAR = dict(lines=(350, 440, 530, 620, 710), line=20, outline=48, scale=1.06, x=466, y=664, gap=18)
SMALL = dict(lines=(420, 560, 700), line=40, outline=76, scale=1.12, x=470, y=700, gap=26)


def svg(body, view_box="0 0 1024 1024", size=None):
    dims = f' width="{size[0]}" height="{size[1]}"' if size else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}"{dims}>\n{body}\n</svg>\n'


def note(g, color, halo=0):
    """An eighth note, stem up. With `halo`, a widened silhouette used to cut the staff lines around the note."""
    s, x, y = g["scale"], g["x"], g["y"]
    hx, top, tip = x + 62 * s, y - 330 * s, y - 160 * s
    w = 36 * s + 2 * halo
    flag = f"M{hx:.0f} {top:.0f} C{hx + 70 * s:.0f} {top + 30 * s:.0f} {hx + 120 * s:.0f} {top + 80 * s:.0f} {hx + 92 * s:.0f} {tip:.0f}"
    if halo:  # also clear the pocket between flag and stem
        flag += f" L{hx:.0f} {tip:.0f} Z"
    return (f'<ellipse cx="{x}" cy="{y}" rx="{74 * s:.0f}" ry="{55 * s:.0f}" transform="rotate(-22 {x} {y})" '
            f'fill="{color}" stroke="{color}" stroke-width="{2 * halo}"/>'
            f'<line x1="{hx:.0f}" y1="{y - 14 * s:.0f}" x2="{hx:.0f}" y2="{top:.0f}" stroke="{color}" '
            f'stroke-width="{w:.0f}" stroke-linecap="round"/>'
            f'<path d="{flag}" fill="{color if halo else "none"}" stroke="{color}" stroke-width="{w:.0f}" '
            f'stroke-linecap="round" stroke-linejoin="round"/>')


def mark(ink, note_color, p, small=False):
    """Shield outline with staff lines and the note; transparent, works on any background (the gap is a mask)."""
    g = SMALL if small else REGULAR
    lines = "".join(f'<line x1="150" y1="{y}" x2="880" y2="{y}"/>' for y in g["lines"])
    return (f'<defs><clipPath id="{p}clip"><path d="{SHIELD}"/></clipPath>'
            f'<mask id="{p}gap" maskUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024">'
            f'<rect width="1024" height="1024" fill="#fff"/>{note(g, "#000", g["gap"])}</mask></defs>'
            f'<g clip-path="url(#{p}clip)"><g mask="url(#{p}gap)" stroke="{ink}" stroke-width="{g["line"]}">{lines}</g></g>'
            f'<path d="{SHIELD}" fill="none" stroke="{ink}" stroke-width="{g["outline"]}" stroke-linejoin="round"/>'
            + note(g, note_color))


def light(p, small=False):
    return mark(NAVY, BRASS, p, small)


def dark(p, small=False):
    return mark(CREAM, BRASS_LIGHT, p, small)


def tile(bg, body):
    """Rounded app tile (the rounding most platforms apply themselves is drawn in, for previews and the web)."""
    return f'<rect width="1024" height="1024" rx="224" fill="{bg}"/>{body}'


def square(bg, body, inset=1.0):
    """Edge to edge, for platforms that mask the icon (iOS, Android maskable): content in the safe zone."""
    t = f' transform="translate(512 512) scale({inset}) translate(-512 -512)"' if inset != 1 else ""
    return f'<rect width="1024" height="1024" fill="{bg}"/><g{t}>{body}</g>'


def favicon():
    """Small mark that follows the browser's light or dark mode."""
    return svg('<style>.d{display:none}@media (prefers-color-scheme: dark){.l{display:none}.d{display:inline}}</style>'
               f'<g class="l">{light("fl-", True)}</g><g class="d">{dark("fd-", True)}</g>')


def lockup(ink, note_color, p):
    """Mark and wordmark side by side (README, docs). The wordmark is text: it needs Charter or a fallback serif."""
    return svg(f'<g transform="translate(0 0) scale(0.25)">{mark(ink, note_color, p)}</g>'
               f'<text x="268" y="166" font-family="{SERIF}" font-size="148" font-weight="600" fill="{ink}" '
               f'letter-spacing="1">notenwart</text>', "0 0 1080 256", (1080, 256))


def social():
    return svg(f'<rect width="1280" height="640" fill="{NAVY}"/>'
               f'<g transform="translate(80 140) scale(0.35)">{dark("s-")}</g>'
               f'<text x="480" y="318" font-family="{SERIF}" font-size="132" font-weight="600" fill="{CREAM}">notenwart</text>'
               f'<text x="484" y="390" font-family="{SERIF}" font-size="31" fill="{CREAM}" opacity="0.8">'
               f'Sheet music for schools, orchestras and bands –</text>'
               f'<text x="484" y="434" font-family="{SERIF}" font-size="31" fill="{CREAM}" opacity="0.8">'
               f'every player sees their own part.</text>'
               f'<rect x="484" y="472" width="120" height="8" rx="4" fill="{BRASS_LIGHT}"/>',
               "0 0 1280 640", (1280, 640))


# --- export ----------------------------------------------------------------------------------


def write(path, content):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(content)


def render(src, dst, w, h=None):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    subprocess.run(["rsvg-convert", "-w", str(w), "-h", str(h or w), src, "-o", dst], check=True)


def ico(dst, pngs):
    """favicon.ico with embedded PNGs."""
    data = [open(p, "rb").read() for _, p in pngs]
    out = struct.pack("<HHH", 0, 1, len(pngs))
    offset = 6 + 16 * len(pngs)
    for (size, _), d in zip(pngs, data):
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(d), offset)
        offset += len(d)
    with open(dst, "wb") as f:
        f.write(out + b"".join(data))


def main():
    files = {
        "notenwart-icon-light.svg": svg(tile(CREAM, light("il-"))),
        "notenwart-icon-dark.svg": svg(tile(NAVY, dark("id-"))),
        "notenwart-icon-small-light.svg": svg(tile(CREAM, light("isl-", True))),
        "notenwart-icon-small-dark.svg": svg(tile(NAVY, dark("isd-", True))),
        "notenwart-icon-square-light.svg": svg(square(CREAM, light("ql-"), 0.9)),
        "notenwart-icon-square-dark.svg": svg(square(NAVY, dark("qd-"), 0.9)),
        "notenwart-icon-maskable.svg": svg(square(CREAM, light("m-"), 0.78)),
        "notenwart-mark-on-light.svg": svg(light("ml-")),
        "notenwart-mark-on-dark.svg": svg(dark("md-")),
        "notenwart-mark-small-on-light.svg": svg(light("msl-", True)),
        "notenwart-mark-small-on-dark.svg": svg(dark("msd-", True)),
        "notenwart-glyph-black.svg": svg(mark("#000000", "#000000", "gb-")),
        "notenwart-glyph-white.svg": svg(mark("#FFFFFF", "#FFFFFF", "gw-")),
        "notenwart-lockup-on-light.svg": lockup(NAVY, BRASS, "ll-"),
        "notenwart-lockup-on-dark.svg": lockup(CREAM, BRASS_LIGHT, "ld-"),
        "favicon.svg": favicon(),
        "notenwart-social.svg": social(),
    }
    for name, content in files.items():
        write(os.path.join(SVG, name), content)
    s = lambda name: os.path.join(SVG, name)

    for theme in ("light", "dark"):
        for n in (16, 32, 48, 64, 128, 256, 512, 1024):
            src = f"notenwart-icon-small-{theme}.svg" if n <= 48 else f"notenwart-icon-{theme}.svg"
            render(s(src), f"{PNG}/icon/notenwart-icon-{theme}-{n}.png", n)
        for n in (128, 256, 512):
            render(s(f"notenwart-mark-on-{theme}.svg"), f"{PNG}/mark/notenwart-mark-on-{theme}-{n}.png", n)
        render(s(f"notenwart-lockup-on-{theme}.svg"), f"{PNG}/lockup/notenwart-lockup-on-{theme}.png", 1080, 256)

    for n in (16, 32, 48):
        render(s("favicon.svg"), f"{WEB}/favicon-{n}.png", n)
    ico(f"{WEB}/favicon.ico", [(n, f"{WEB}/favicon-{n}.png") for n in (16, 32, 48)])
    for n in (16, 32, 48):
        os.remove(f"{WEB}/favicon-{n}.png")
    shutil.copy(s("favicon.svg"), f"{WEB}/favicon.svg")
    render(s("notenwart-icon-square-light.svg"), f"{WEB}/apple-touch-icon.png", 180)
    for n in (192, 512):
        render(s("notenwart-icon-square-light.svg"), f"{WEB}/icon-{n}.png", n)
    render(s("notenwart-icon-maskable.svg"), f"{WEB}/icon-maskable-512.png", 512)

    render(s("notenwart-social.svg"), f"{SOCIAL}/notenwart-social-1280x640.png", 1280, 640)
    render(s("notenwart-icon-square-dark.svg"), f"{SOCIAL}/github-avatar-500.png", 500)


if __name__ == "__main__":
    main()
