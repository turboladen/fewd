#!/usr/bin/env bun
// Pack PNG files into a single ICO (PNG-compressed frames). No deps; run with bun.
// ICO frames may be PNG blobs (supported since Windows Vista and by all modern
// browsers), so we build a lean favicon.ico (16+32) without upscaling — unlike
// png-to-ico, which embeds a 256x256 frame and bloats the file.
//
// Usage: bun scripts/pack-ico.mjs <in.png>... <out.ico>
import { readFileSync, writeFileSync } from "node:fs";

function pngSize(buf) {
  // PNG signature is 0x89 'P' 'N' 'G'; IHDR width/height are big-endian u32s
  // at byte offsets 16 and 20 (8-byte sig + 4 len + 4 "IHDR").
  if (buf.readUInt32BE(0) !== 0x89504e47) {
    throw new Error("not a PNG file");
  }
  return [buf.readUInt32BE(16), buf.readUInt32BE(20)];
}

const args = process.argv.slice(2);
const out = args.pop();
if (args.length === 0 || !out) {
  console.error("usage: bun scripts/pack-ico.mjs <in.png>... <out.ico>");
  process.exit(1);
}

const frames = args.map((path) => {
  const data = readFileSync(path);
  const [w, h] = pngSize(data);
  return { w, h, data };
});

const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0); // reserved
header.writeUInt16LE(1, 2); // type = 1 (icon)
header.writeUInt16LE(frames.length, 4); // image count

let offset = 6 + 16 * frames.length;
const entries = [];
const blobs = [];
for (const { w, h, data } of frames) {
  const e = Buffer.alloc(16);
  e.writeUInt8(w & 0xff, 0); // width (0 means 256; fine, we only pack <=32)
  e.writeUInt8(h & 0xff, 1); // height
  e.writeUInt8(0, 2); // palette color count
  e.writeUInt8(0, 3); // reserved
  e.writeUInt16LE(1, 4); // color planes
  e.writeUInt16LE(32, 6); // bits per pixel
  e.writeUInt32LE(data.length, 8); // image data size
  e.writeUInt32LE(offset, 12); // image data offset
  entries.push(e);
  blobs.push(data);
  offset += data.length;
}

writeFileSync(out, Buffer.concat([header, ...entries, ...blobs]));
