#!/usr/bin/env python3
"""Pack PNG files into a single ICO (PNG-compressed frames). Stdlib only.

Usage: pack_ico.py <in.png>... <out.ico>

ICO frames may be PNG blobs (supported since Windows Vista and by all modern
browsers). Used here to build a lean favicon.ico (16+32) without upscaling —
unlike png-to-ico, which embeds a 256x256 frame and bloats the file.
"""
import struct
import sys


def png_size(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG file")
    # IHDR width/height are the two big-endian u32s at byte offset 16.
    return struct.unpack(">II", data[16:24])


def main() -> None:
    *pngs, out = sys.argv[1:]
    if not pngs:
        sys.exit("usage: pack_ico.py <in.png>... <out.ico>")

    frames = []
    for path in pngs:
        with open(path, "rb") as f:
            data = f.read()
        w, h = png_size(data)
        frames.append((w, h, data))

    header = struct.pack("<HHH", 0, 1, len(frames))  # reserved, type=1, count
    offset = 6 + 16 * len(frames)
    entries = b""
    blobs = b""
    for w, h, data in frames:
        # width/height byte is 0 for 256; fine here since we only pack <=32.
        entries += struct.pack(
            "<BBBBHHII", w & 0xFF, h & 0xFF, 0, 0, 1, 32, len(data), offset
        )
        blobs += data
        offset += len(data)

    with open(out, "wb") as f:
        f.write(header + entries + blobs)


if __name__ == "__main__":
    main()
