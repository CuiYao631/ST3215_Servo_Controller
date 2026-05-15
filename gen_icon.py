import struct, zlib, math, sys

w = h = 128  # Small enough to be fast
color = (233, 69, 96)

def chunk(t, d):
    c = t + d
    crc = struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    return struct.pack('>I', len(d)) + c + crc

hdr = b'\x89PNG\r\n\x1a\n'
ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))  # RGBA

raw = b''
for y in range(h):
    raw += b'\x00'
    for x in range(w):
        cx, cy = w // 2, h // 2
        dx, dy = x - cx, y - cy
        dist = (dx * dx + dy * dy) ** 0.5
        r1 = w * 0.42
        r2 = w * 0.22
        r3 = w * 0.08
        a = math.atan2(dy, dx)
        notch = (math.sin(a * 8) > 0.2) and dist > r1 * 0.88

        if dist < r3:
            raw += bytes([26, 26, 46, 255])
        elif (dist < r1 and dist > r2) or (notch and dist < r1 * 1.18):
            raw += bytes(color) + b'\xff'
        else:
            raw += bytes([26, 26, 46, 255])

compressed = zlib.compress(raw)
idat = chunk(b'IDAT', compressed)
iend = chunk(b'IEND', b'')

with open(sys.argv[1], 'wb') as f:
    f.write(hdr + ihdr + idat + iend)
print(f"Created {sys.argv[1]}")
