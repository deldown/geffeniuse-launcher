#!/usr/bin/env python3
"""Generates the pixel landscapes in ui/art: four seasons x (day, dusk, night), Christmas and New Year.
Usage: python3 scripts/gen_scenes.py   (writes ui/art/hero-*.svg; the base and Halloween scenes are separate)."""
import os, random

W, H = 120, 40
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "ui", "art")

SKY = {
    "day": ['#3f86d8', '#4f95e0', '#62a5e8', '#78b6ee', '#8fc5f3', '#a8d3f6', '#c0e0f9', '#d6ecfb'],
    "dusk": ['#2d2b5e', '#4a3475', '#6e3f80', '#9a4a82', '#c9577a', '#e8795c', '#f39c62', '#f8c37a'],
    "night": ['#1b2a4a', '#223660', '#2a4577', '#34568a', '#4268a0', '#5a82b8', '#79a0cf', '#9bbfe0'],
}
SHADE = {"day": 1.0, "dusk": 0.74, "night": 0.5}


def shade(hex_, f):
    n = int(hex_[1:], 16)
    c = [max(0, min(255, round(v * f))) for v in (n >> 16, (n >> 8) & 255, n & 255)]
    return "#%02x%02x%02x" % tuple(c)


class Canvas:
    def __init__(self):
        self.g = [[None] * W for _ in range(H)]

    def put(self, x, y, c):
        if 0 <= x < W and 0 <= y < H:
            self.g[y][x] = c

    def rect(self, x, y, w, h, c):
        for j in range(h):
            for i in range(w):
                self.put(x + i, y + j, c)

    def disk(self, cx, cy, r, c):
        for y in range(cy - r, cy + r + 1):
            for x in range(cx - r, cx + r + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + r // 2:
                    self.put(x, y, c)

    def sky(self, colors, rows=27):
        for y in range(H):
            for x in range(W):
                self.g[y][x] = colors[min(len(colors) - 1, y * len(colors) // rows)]

    def hills(self, base, amp, step, color, edge, seed):
        rr = random.Random(seed)
        h = base
        xs = []
        for x in range(0, W, step):
            h = max(base - amp, min(base + amp, h + rr.choice([-2, -1, 0, 1, 2])))
            xs.extend([h] * min(step, W - x))
        for x, top in enumerate(xs):
            for y in range(top, H):
                self.g[y][x] = color
            self.put(x, top, edge)
        return xs

    def clouds(self, spots, c1, c2):
        for cx, cy, w in spots:
            self.rect(cx, cy, w, 2, c1)
            self.rect(cx + 2, cy - 1, max(2, w - 4), 1, c1)
            self.rect(cx + 1, cy + 2, max(2, w - 3), 1, c2)

    def svg(self, name):
        cells = {}
        for y in range(H):
            for x in range(W):
                cells.setdefault(self.g[y][x], []).append((x, y))
        paths = ""
        for c, pts in cells.items():
            rows = {}
            for x, y in pts:
                rows.setdefault(y, []).append(x)
            d = ""
            for y, xs in rows.items():
                xs.sort()
                start = prev = xs[0]
                for x in xs[1:] + [None]:
                    if x is None or x != prev + 1:
                        d += f"M{start} {y}h{prev - start + 1}v1h-{prev - start + 1}z"
                        if x is not None:
                            start = x
                    prev = x if x is not None else prev
            paths += f'<path fill="{c}" d="{d}"/>'
        with open(os.path.join(OUT, f"hero-{name}.svg"), "w") as f:
            f.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMax slice" shape-rendering="crispEdges">{paths}</svg>')


def ground(cv, near, dirt, stone, depth=3):
    for x in range(W):
        top = near[x]
        for y in range(top + 1, min(H, top + 1 + depth)):
            cv.g[y][x] = dirt
        for y in range(top + 1 + depth, H):
            cv.g[y][x] = stone


def round_tree(cv, near, tx, leaf, leaf_hi, trunk, blossoms=None, rr=None):
    top = near[tx]
    cv.rect(tx, top - 4, 1, 4, trunk)
    cv.rect(tx - 2, top - 8, 5, 4, leaf)
    cv.rect(tx - 1, top - 9, 3, 1, leaf_hi)
    cv.rect(tx - 2, top - 8, 5, 1, leaf_hi)
    if blossoms and rr:
        for _ in range(4):
            cv.put(tx - 2 + rr.randrange(5), top - 9 + rr.randrange(5), blossoms)


def spruce(cv, near, tx, green, snow=None, lights=None, star=None, h=11):
    """A stacked spruce: n tiers of three rows, narrowing upwards, with optional snow, lights and star."""
    top = near[tx]
    cv.rect(tx, top - 2, 1, 2, '#4a3322')
    n = max(3, h // 3)
    y0 = top - 3
    for t in range(n):
        for r in range(3):
            half = max(1, (n - t) + 1 - r)
            y = y0 - t * 3 - r
            cv.rect(tx - half, y, half * 2 + 1, 1, green)
            if snow and r == 2:
                cv.rect(tx - half + 1, y, max(1, half * 2 - 1), 1, snow)
    ytop = y0 - n * 3 + 1
    if star:
        for dx, dy in ((0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)):
            cv.put(tx + dx, ytop + dy - 1, star)
    elif snow:
        cv.put(tx, ytop, snow)
    if lights:
        rr = random.Random(tx)
        for k in range(n * 4):
            lx = tx + rr.randrange(-n, n + 1)
            ly = y0 - rr.randrange(n * 3 - 1)
            if 0 <= ly < H and 0 <= lx < W and cv.g[ly][lx] == green:
                cv.put(lx, ly, lights[k % len(lights)])


def sun_or_moon(cv, tod, cx, cy, r, sun_c, halo, moon_c='#e9eef8', moon_h='#cdd6e6'):
    if tod == "night":
        cv.disk(cx, cy, r, moon_c)
        cv.disk(cx, cy, max(1, r - 2), moon_h)
    elif tod == "dusk":
        cv.disk(cx, cy + 12, r, '#ffb347')
        cv.disk(cx, cy + 12, max(1, r - 2), '#ffd98a')
    else:
        cv.disk(cx, cy, r, sun_c)
        cv.disk(cx, cy, max(1, r - 2), halo)


def season(name, tod):
    cv = Canvas()
    cv.sky(SKY[tod])
    rr = random.Random(sum(map(ord, name)) + 7)
    if tod == "night":
        for _ in range(16):
            cv.put(rr.randrange(W), rr.randrange(0, 8), '#dbe8ff')
    f = SHADE[tod]
    cloud1, cloud2 = ('#f1f6ff', '#dbe6f5') if tod == "day" else (shade('#c8d2e8', f + .15), shade('#a8b4cc', f + .15))
    cv.clouds([(14, 9, 10), (50, 6, 10), (100, 18, 10), (66, 15, 8)], cloud1, cloud2)
    P = {
        "spring": dict(far=('#8fb0c8', '#a5c2d6'), mid=('#5fae6a', '#79c682'), near=('#6fcf4f', '#92e86a'), dirt='#7a5636', stone='#6d717c', leaf='#6dc45a', hi='#8ee07a', trunk='#6b4a2e', sun='#ffe36a', halo='#fff3b0'),
        "summer": dict(far=('#6f95c4', '#88aad2'), mid=('#2f9a55', '#43b56a'), near=('#3fb23a', '#5fd54a'), dirt='#7a5636', stone='#6d717c', leaf='#2f8f33', hi='#49b24a', trunk='#6b4a28', sun='#ffd84a', halo='#fff3b0'),
        "autumn": dict(far=('#9a7a68', '#b08c76'), mid=('#c2762c', '#d98f3a'), near=('#b8742a', '#d9912e'), dirt='#6b4a2e', stone='#6d6258', leaf='#e8741a', hi='#f2a33a', trunk='#5b3d22', sun='#ffc24a', halo='#ffe29a'),
        "winter": dict(far=('#c8d8ec', '#e2ecf8'), mid=('#9fb5cf', '#c3d3e6'), near=('#eef4fb', '#ffffff'), dirt='#cdd9ea', stone='#8a96ab', leaf='#1f5a3a', hi='#2f7a52', trunk='#4a3322', sun='#fff0c0', halo='#ffffff'),
    }[name]
    sun_or_moon(cv, tod, 86, 8, 8, P['sun'], P['halo'])
    g = min(1.0, f + (0.32 if name == "winter" else 0))  # snow stays bright at night
    cv.hills(24, 4, 5, shade(P['far'][0], g), shade(P['far'][1], g), 1)
    cv.hills(28, 3, 4, shade(P['mid'][0], g), shade(P['mid'][1], g), 2)
    near = cv.hills(32, 2, 3, shade(P['near'][0], g), shade(P['near'][1], g), 3)
    ground(cv, near, shade(P['dirt'], g), shade(P['stone'], g))
    trees = (10, 31, 57, 79, 108)
    if name == "winter":
        for i, tx in enumerate(trees):
            spruce(cv, near, tx, shade(P['leaf'], f + .05), snow=shade('#ffffff', min(1, f + .2)), h=9 + (i % 2) * 3)
    else:
        leaf, hi = P['leaf'], P['hi']
        for tx in trees:
            lc = leaf
            if name == "autumn":
                lc = rr.choice([leaf, '#c0392b', '#d9912e', '#e8b21a'])
                hi = shade(lc, 1.18)
            round_tree(cv, near, tx, shade(lc, f + .1), shade(hi, f + .1), shade(P['trunk'], f), blossoms='#f9c4da' if name == "spring" else None, rr=rr)
    # ground detail
    if name == "spring":
        for x in range(0, W, 2):
            if rr.random() < .35:
                cv.put(x, near[x] - 1, shade(rr.choice(['#ffffff', '#ffd84a', '#ff7aa8']), f + .1))
    if name == "summer":
        for sx in (16, 44, 70, 96):
            top = near[sx]
            cv.rect(sx, top - 4, 1, 4, shade('#3b8a2a', f))
            cv.rect(sx - 1, top - 7, 3, 3, shade('#ffd51a', f + .1))
            cv.put(sx, top - 6, shade('#6b3a10', f))
    if name == "autumn":
        for x in range(0, W, 2):
            if rr.random() < .4:
                cv.put(x, near[x] - 1 + rr.randrange(2), shade(rr.choice(['#c0392b', '#e8741a', '#e8b21a']), f + .1))
        for _ in range(9):
            cv.put(rr.randrange(W), rr.randrange(10, 28), shade(rr.choice(['#c0392b', '#e8741a', '#e8b21a']), min(1, f + .2)))
    if name == "winter":
        for _ in range(26):
            cv.put(rr.randrange(W), rr.randrange(0, 30), shade('#ffffff', min(1, f + .3)))
    cv.svg(f"{name}-{tod}")


def christmas():
    cv = Canvas()
    cv.sky(['#0a1230', '#101c46', '#18285c', '#223670', '#2e4686', '#3d5a9c', '#5a78b4', '#7e98c8'])
    rr = random.Random(24)
    for _ in range(22):
        cv.put(rr.randrange(W), rr.randrange(0, 12), '#e6f0ff')
    cv.disk(22, 9, 7, '#f4f1e2'); cv.disk(22, 9, 5, '#e2deca')
    cv.clouds([(48, 6, 12), (88, 14, 10)], '#2b3a6a', '#222f58')
    cv.hills(25, 4, 5, '#5a6f9c', '#7a8fb8', 1)
    cv.hills(28, 3, 4, '#8497bd', '#a5b5d4', 2)
    near = cv.hills(31, 2, 3, '#dfe8f6', '#ffffff', 3)
    ground(cv, near, '#c4d2e8', '#8794ad', 3)
    lights = ['#ff4a3d', '#ffd84a', '#4aa8ff', '#5be070']
    for i, tx in enumerate((12, 40, 66, 92, 112)):
        spruce(cv, near, tx, '#1c5a3c', snow='#ffffff', lights=lights, star='#ffe27a' if i == 2 else None, h=12 if i == 2 else 9)
    # snowman
    sx = 52
    top = near[sx]
    cv.disk(sx, top - 2, 3, '#ffffff'); cv.disk(sx, top - 7, 2, '#ffffff'); cv.disk(sx, top - 10, 1, '#ffffff')
    cv.put(sx - 1, top - 11, '#1b1b22'); cv.put(sx + 1, top - 11, '#1b1b22'); cv.put(sx, top - 10, '#ff8a1f'); cv.put(sx + 1, top - 10, '#ff8a1f')
    cv.rect(sx - 2, top - 8, 5, 1, '#d63a2e'); cv.put(sx + 2, top - 7, '#d63a2e')
    cv.rect(sx - 1, top - 13, 3, 2, '#1b1b22'); cv.rect(sx - 2, top - 12, 5, 1, '#1b1b22')
    cv.put(sx, top - 6, '#1b1b22'); cv.put(sx, top - 4, '#1b1b22')
    # presents
    for px, c1, c2 in ((78, '#d63a2e', '#ffd84a'), (82, '#2f7de0', '#ffffff'), (86, '#2fa84a', '#ff4a3d')):
        t = near[px]
        cv.rect(px, t - 3, 3, 3, c1); cv.rect(px + 1, t - 3, 1, 3, c2); cv.rect(px, t - 2, 3, 1, c2)
        cv.put(px, t - 4, c2); cv.put(px + 2, t - 4, c2)
    # warm window glow of a little cabin
    cx = 100
    t = near[cx]
    cv.rect(cx - 5, t - 6, 11, 6, '#6b3f22'); cv.rect(cx - 6, t - 8, 13, 2, '#f2f6ff'); cv.rect(cx - 4, t - 10, 9, 2, '#f2f6ff')
    cv.rect(cx - 3, t - 5, 2, 2, '#ffd84a'); cv.rect(cx + 2, t - 5, 2, 2, '#ffd84a'); cv.rect(cx, t - 4, 1, 4, '#3a2412')
    cv.svg("christmas")


def newyear():
    cv = Canvas()
    cv.sky(['#0a0a1f', '#12123a', '#1b1b52', '#29206a', '#3a2a80', '#52308f', '#733a98', '#9a4aa0'])
    rr = random.Random(1)
    for _ in range(14):
        cv.put(rr.randrange(W), rr.randrange(0, 10), '#e6e6ff')
    # skyline
    x = 0
    heights = []
    while x < W:
        bw = rr.randrange(5, 10)
        bh = rr.randrange(8, 17)
        base = 34
        cv.rect(x, base - bh, bw, bh + 6, '#120f26')
        for wy in range(base - bh + 2, base - 1, 3):
            for wx in range(x + 1, x + bw - 1, 2):
                if rr.random() < .45:
                    cv.put(wx, wy, '#ffd86b')
        heights.append((x, bw, bh))
        x += bw
    # clock tower in the middle
    tx = 56
    cv.rect(tx, 8, 8, 28, '#1b1638'); cv.rect(tx + 1, 5, 6, 3, '#1b1638'); cv.rect(tx + 3, 2, 2, 3, '#1b1638')
    cv.disk(tx + 4, 14, 3, '#f4f1e2'); cv.rect(tx + 4, 11, 1, 3, '#14102a'); cv.rect(tx + 4, 11, 1, 3, '#14102a')
    cv.put(tx + 4, 14, '#14102a')
    cv.rect(0, 35, W, 5, '#0d0a1c')
    cv.svg("newyear")


if __name__ == "__main__":
    for s in ("spring", "summer", "autumn", "winter"):
        for t in ("day", "dusk", "night"):
            season(s, t)
    christmas()
    newyear()
    print("ok")
