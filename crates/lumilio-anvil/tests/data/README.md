# Region samples

Small regions written by the vanilla dedicated server, for the tests in
`src/tests/samples.rs`. Each directory holds one region trimmed to 4×4 chunks
(the chunks' compressed bytes and timestamps are copied unchanged, only the
sector layout is rewritten) and the world's `level.dat`. No third-party worlds;
nothing here was edited by hand.

| Directory | Server jar sha1 (Mojang version metadata) | Java | Region | Chunks kept | Compression |
|---|---|---|---|---|---|
| `1.16.5/` | `1b557e7b033b583cd9f66746b7a9ab1ec1673ced` | 21.0.7 | `world/region/r.0.-1.mca` | x 8–11, z 16–19 | zlib (2) |
| `1.18.2/` | `c8f83c5655308435b3dcf03c06d9fe8740a77469` | 21.0.7 | `world/region/r.0.0.mca` | x 0–3, z 0–3 | zlib (2) |
| `26.3/` | `33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c` | 25.0.1 | `world/dimensions/minecraft/overworld/region/r.0.0.mca` | x 5–8, z 0–3 | LZ4 (4) |

Generated 2026-10-09. Java came from the launcher's managed runtimes
(`java-runtime-delta`, `java-runtime-epsilon`); 1.16.5 asks for Java 8 but its
server runs on 21. Jars and full worlds stayed in the git-ignored
`target/anvil-samples/<version>/`.

## Commands

For each version, in `target/anvil-samples/<version>/` with `server.jar`
downloaded from the URL in Mojang's version metadata and its sha1 checked:

```sh
echo eula=true > eula.txt
cat > server.properties <<EOF
level-seed=20261009
online-mode=false
server-port=0
spawn-protection=0
view-distance=2
simulation-distance=2
EOF
# 26.3 only: one more line, region-file-compression=lz4
( until grep -q 'Done (' logs/latest.log 2>/dev/null; do sleep 2; done; sleep 3
  # 26.3 only (it keeps no spawn chunks loaded): echo 'forceload add 0 0 63 63'; sleep 20
  echo 'save-all flush'; sleep 3; echo stop ) | java -Xmx2G -jar server.jar nogui
```

Then trim with this script (`trim.py SRC DST X0 Z0 4`):

```python
import sys, struct
src, dst, x0, z0, n = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:])
b = open(src, 'rb').read()
head, stamps, body = bytearray(4096), bytearray(4096), bytearray()
for z in range(z0, z0 + n):
    for x in range(x0, x0 + n):
        i = (z * 32 + x) * 4
        e = struct.unpack('>I', b[i:i + 4])[0]
        off, sec = e >> 8, e & 255
        if off < 2 or sec == 0:
            continue
        at = 2 + len(body) // 4096
        body += b[off * 4096:(off + sec) * 4096]
        head[i:i + 4] = struct.pack('>I', at << 8 | sec)
        stamps[i:i + 4] = b[4096 + i:4096 + i + 4]
open(dst, 'wb').write(head + stamps + body)
```

## What the samples show

- 1.16.5: the `Level` layout, numeric 4×4×4 biomes, 37-long heightmaps; the
  server also left zero-length `r.*.mca` files beside the real ones.
- 1.18.2: sections at the root with `yPos` -4 and per-section biome palettes.
- 26.3: dimensions moved under `dimensions/minecraft/<name>/`; palette entries
  are plain strings, or `{"": name}` and `{id, properties}` in a mixed list;
  `level.dat` keeps the spawn as `Data.spawn{pos:[I;x,y,z],dimension,…}`.
  Chunk column x = 8 stopped at `minecraft:biomes`, x = 7 at
  `minecraft:terrain` and x = 6 at `minecraft:initialize_light` (features
  placed), so the sample also has unfinished chunks.
