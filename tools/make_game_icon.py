"""The game's own window icon, made from the launcher's.

The launcher's mark (docs/assets/icon.png) is a stone seal in a gold frame with blue circuit lines. The game's window
takes the same seal with the two colours exchanged: a blue frame and gold lines, so the two taskbar buttons are told
apart at a glance and still belong together. Nothing here comes from the game.

  py -I tools/make_game_icon.py                      docs/assets/icon.png -> docs/assets/icon-game.png
  py -I tools/make_game_icon.py --header IN.png OUT.h   a small RGBA image as a C array (the Linux window icon)

After the first form, in apps/launcher: `pnpm tauri icon ../../docs/assets/icon-game.png -o ../../work/icon-game`,
then copy work/icon-game/icon.ico to engine/runtime/ui/aod_game.ico and run the second form on its 64x64.png for
engine/runtime/ui/aod_game_icon.h.

Python standard library only: a PNG reader and writer for 8-bit RGBA, not interlaced.
"""
import argparse
import colorsys
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIGNATURE = b"\x89PNG\r\n\x1a\n"


def read_png(path):
    """(width, height, bytearray of RGBA rows) of an 8-bit RGBA or RGB PNG that is not interlaced."""
    data = Path(path).read_bytes()
    if data[:8] != SIGNATURE:
        raise ValueError(f"{path} is not a PNG")
    offset, idat, header = 8, [], None
    while offset + 8 <= len(data):
        size, kind = struct.unpack_from(">I4s", data, offset)
        body = data[offset + 8:offset + 8 + size]
        offset += 12 + size
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat.append(body)
        elif kind == b"IEND":
            break
    width, height, depth, colour, _, _, interlace = header
    if depth != 8 or colour not in (2, 6) or interlace:
        raise ValueError(f"{path}: only 8-bit RGB or RGBA without interlacing is read here")
    channels = 4 if colour == 6 else 3
    raw = zlib.decompress(b"".join(idat))
    stride = width * channels
    out = bytearray(width * height * 4)
    previous = bytearray(stride)
    position = 0
    for y in range(height):
        kind = raw[position]
        row = bytearray(raw[position + 1:position + 1 + stride])
        position += 1 + stride
        if kind == 1:
            for i in range(channels, stride):
                row[i] = (row[i] + row[i - channels]) & 255
        elif kind == 2:
            for i in range(stride):
                row[i] = (row[i] + previous[i]) & 255
        elif kind == 3:
            for i in range(stride):
                left = row[i - channels] if i >= channels else 0
                row[i] = (row[i] + ((left + previous[i]) >> 1)) & 255
        elif kind == 4:
            for i in range(stride):
                a = row[i - channels] if i >= channels else 0
                b = previous[i]
                c = previous[i - channels] if i >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                row[i] = (row[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        elif kind:
            raise ValueError(f"{path}: unknown row filter {kind}")
        previous = row
        if channels == 4:
            out[y * width * 4:(y + 1) * width * 4] = row
        else:
            for x in range(width):
                out[(y * width + x) * 4:(y * width + x) * 4 + 4] = bytes(row[x * 3:x * 3 + 3]) + b"\xff"
    return width, height, out


def write_png(path, width, height, rgba):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
    rows = b"".join(b"\x00" + bytes(rgba[y * width * 4:(y + 1) * width * 4]) for y in range(height))
    Path(path).write_bytes(SIGNATURE + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) +
                           chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def swap_hue(hue):
    """The frame's gold becomes the lines' blue and the other way round; other hues stay."""
    degrees = hue * 360.0
    if 15.0 <= degrees <= 75.0:        # gold frame (about 40) -> the blue of the lines (about 195)
        degrees += 155.0
    elif 160.0 <= degrees <= 215.0:    # blue lines (about 187) -> gold (about 40)
        degrees -= 147.0
    return (degrees % 360.0) / 360.0


def game_icon(source, target):
    width, height, rgba = read_png(source)
    cache = {}
    for i in range(0, len(rgba), 4):
        if not rgba[i + 3]:
            continue
        key = (rgba[i], rgba[i + 1], rgba[i + 2])
        new = cache.get(key)
        if new is None:
            h, s, v = colorsys.rgb_to_hsv(key[0] / 255.0, key[1] / 255.0, key[2] / 255.0)
            r, g, b = colorsys.hsv_to_rgb(swap_hue(h), s, v)
            new = cache[key] = (round(r * 255), round(g * 255), round(b * 255))
        rgba[i], rgba[i + 1], rgba[i + 2] = new
    write_png(target, width, height, rgba)
    print(f"{target}: {width}x{height}")


def header(source, target):
    width, height, rgba = read_png(source)
    name = Path(target).stem
    lines = [f"// The game's window icon as {width}x{height} RGBA, 8 bits a channel (Age of Decomp's own artwork, GPL-3.0-or-later).",
             "// Generated by tools/make_game_icon.py --header from docs/assets/icon-game.png; do not edit.",
             f"#define AOD_GAME_ICON_W {width}", f"#define AOD_GAME_ICON_H {height}",
             f"static const unsigned char {name}[{len(rgba)}] = {{"]
    for i in range(0, len(rgba), 24):
        lines.append("    " + ",".join(str(b) for b in rgba[i:i + 24]) + ",")
    lines.append("};")
    Path(target).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{target}: {width}x{height}, {len(rgba)} bytes")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--header", nargs=2, metavar=("PNG", "H"), type=Path)
    args = parser.parse_args(argv)
    if args.header:
        header(*args.header)
    else:
        game_icon(ROOT / "docs/assets/icon.png", ROOT / "docs/assets/icon-game.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
