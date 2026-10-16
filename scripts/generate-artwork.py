"""Generate original release artwork. Requires Pillow; no game assets are used."""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "crates/player-ui/assets"


def font(size, bold=False):
    return ImageFont.truetype(
        str(ASSETS / ("PlexSans-SemiBold.ttf" if bold else "PlexSans-Regular.ttf")), size
    )


def label(draw, xy, text, size, color, bold=False):
    draw.text(xy, text, font=font(size, bold), fill=color)


GAMES = [
    ("contra", ["CONTRA"], "#d76040"),
    ("snowbros", ["SNOW BROS."], "#539bba"),
    ("bubblebobble", ["BUBBLE", "BOBBLE"], "#83ad65"),
    ("iceclimber", ["ICE CLIMBER"], "#8d94d1"),
    ("doubledragon2", ["DOUBLE", "DRAGON II"], "#daab59"),
    ("tmnt2", ["TMNT II"], "#76b394"),
]
for index, (name, lines, accent) in enumerate(GAMES):
    im = Image.new("RGB", (960, 540), "#202723")
    d = ImageDraw.Draw(im)
    for x in range(500, 1000, 44):
        d.line((x, 0, x - 220, 540), fill="#2c3630", width=2)
    d.rounded_rectangle((700, 100, 1110, 510), radius=100, outline=accent, width=5)
    d.rounded_rectangle((760, 170, 1170, 580), radius=100, outline=accent, width=5)
    d.rectangle((48, 148, 112, 154), fill=accent)
    for j, line in enumerate(lines):
        label(d, (44, 185 + j * 76), line, 67, "#f5f2e7", True)
    im.save(ASSETS / "gallery" / f"{name}.png")
