/* Uses cubiomes generator.h (MIT, Copyright (c) 2020 Cubitect).
 * The bridge is LumilioCL code; upstream structs stay on the C side. */
#include "carvers.h"
#include "decorations.h"
#include "finders.h"
#include "generator.h"
#include "loot.h"
#include "terrain.h"
#include "util.h"
#include "veins.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int lumilio_cubiomes_probe(void)
{
    Generator g;
    setupGenerator(&g, MC_1_21_WD, 0);
    applySeed(&g, DIM_OVERWORLD, 262);
    return getBiomeAt(&g, 1, 0, 64, 0);
}

/* Independent original-game goldens cover several vertical levels, while
 * the product's 2D tiles continue to use block Y=64. Coordinates are quart. */
int lumilio_cubiomes_sample(int mc, uint64_t seed, int dim, int x, int y, int z)
{
    if (mc < MC_1_0 || mc > MC_NEWEST ||
        x < -7500000 || x > 7500000 || z < -7500000 || z > 7500000 ||
        y < -16 || y > 80 || (dim != -1 && dim != 0 && dim != 1)) return -1;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    return getBiomeAt(&g, 4, x, y, z);
}

/* Rows per genBiomes call. The generator is seeded once; the cancellation
 * hook is polled between bands so a stale tile releases its worker quickly. */
#define LUMILIO_BAND 16

int lumilio_cubiomes_generate(int mc, uint64_t seed, int dim,
    int scale, int x, int z, int width, int height, int *out, size_t count,
    int (*cancelled)(void *), void *user)
{
    if (count != (size_t)width * (size_t)height) return 1;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    int y = scale == 1 ? 64 : 16;
    int band = height < LUMILIO_BAND ? height : LUMILIO_BAND;
    Range first = {scale, x, z, width, band, y, 1};
    int *cache = allocCache(&g, first);
    if (!cache) return 1;
    int result = 0;
    for (int row = 0; row < height && result == 0; row += band)
    {
        if (cancelled && cancelled(user)) { result = 2; break; }
        int rows = height - row < band ? height - row : band;
        Range r = {scale, x, z + row, width, rows, y, 1};
        result = genBiomes(&g, cache, r);
        if (result == 0)
            memcpy(out + (size_t)row * (size_t)width, cache,
                (size_t)rows * (size_t)width * sizeof(int));
    }
    free(cache);
    return result;
}

/* Region-grid structures. Index = the Rust `Structure` discriminant; the
 * cubiomes enum itself never crosses the boundary. */
static const int KINDS[] = {
    Desert_Pyramid, Jungle_Pyramid, Swamp_Hut, Igloo, Village, Ocean_Ruin,
    Shipwreck, Monument, Mansion, Outpost, Ruined_Portal, Ancient_City,
    Trail_Ruins, Trial_Chambers, Fortress, Bastion, End_City, Abandoned_Camp,
    Treasure, Mineshaft, Desert_Well, Geode, End_Gateway, Nether_Fossil,
};
#define KIND_COUNT ((int)(sizeof(KINDS) / sizeof(KINDS[0])))

static int kind_config(int mc, int dim, int kind, int *type, StructureConfig *sc)
{
    if (kind < 0 || kind >= KIND_COUNT) return 0;
    int t = KINDS[kind];
    if (t == Ruined_Portal && dim == DIM_NETHER) t = Ruined_Portal_N;
    if (!getStructureConfig(t, mc, sc) || sc->dim != dim) return 0;
    *type = t;
    return 1;
}

int lumilio_cubiomes_structure_available(int mc, int dim, int kind)
{
    int type;
    StructureConfig sc;
    return kind_config(mc, dim, kind, &type, &sc);
}

/* The region grid's block edge for a kind, so the caller can size its output
 * buffer from the kind rather than the smallest upstream region. 0 when the
 * kind is unavailable in that version and dimension. */
int lumilio_cubiomes_region_blocks(int mc, int dim, int kind)
{
    int type;
    StructureConfig sc;
    if (!kind_config(mc, dim, kind, &type, &sc)) return 0;
    return sc.regionSize * 16;
}

/* Oracle tests compare the unfiltered regional placement, including candidates
 * outside an allowed biome. Product queries apply viability afterwards. */
int lumilio_cubiomes_placement(int mc, uint64_t seed, int kind, int rx, int rz, int *out)
{
    if (kind < 0 || kind >= KIND_COUNT || rx < -100 || rx > 100 || rz < -100 || rz > 100) return 1;
    StructureConfig sc;
    int type = KINDS[kind];
    if (!getStructureConfig(type, mc, &sc)) return 1;
    Pos p = (type == Monument || type == Mansion || type == End_City)
        ? getLargeStructurePos(sc, seed, rx, rz) : getFeaturePos(sc, seed, rx, rz);
    out[0] = p.x; out[1] = p.z;
    return 0;
}

/* The raw candidate sequence of one decoration in a chunk, as x,y,z triples:
 * every candidate kept, as if each failed its biome check. Returns the count,
 * or -1 when the release table lacks the decoration. */
int lumilio_cubiomes_decoration_raw(int mc, int dim, int type, uint64_t seed,
    int cx, int cz, int *out, int cap)
{
    const DecoEntry *e = getDecorationEntry(mc, dim, type);
    DecoPos pos[256];
    if (!e || cap > 256) return -1;
    int n = getDecorations(e, NULL, seed, cx, cz, pos, cap);
    for (int i = 0; i < n; i++)
    {
        out[i * 3] = pos[i].x;
        out[i * 3 + 1] = pos[i].y;
        out[i * 3 + 2] = pos[i].z;
    }
    return n;
}

int lumilio_cubiomes_biome_rule(int mc, int rule, int biome)
{
    if (rule == -1) return isStrongholdBiome(mc, biome);
    if (rule < 0 || rule >= KIND_COUNT) return 0;
    return isViableFeatureBiome(mc, KINDS[rule], biome);
}

static int floor_div(int a, int b)
{
    int q = a / b;
    return (a % b != 0 && (a < 0) != (b < 0)) ? q - 1 : q;
}

/* Viable positions inside [x0, x1) x [z0, z1) blocks. `out` holds x,z pairs
 * and must have room for `cap` pairs; the number written goes to *count.
 * Returns 0 ok, 1 error, 2 cancelled, 3 more positions than `cap`. */
int lumilio_cubiomes_structures(int mc, uint64_t seed, int dim, int kind,
    int x0, int z0, int x1, int z1, int *out, size_t cap, size_t *count,
    int (*cancelled)(void *), void *user)
{
    int type;
    StructureConfig sc;
    *count = 0;
    if (!kind_config(mc, dim, kind, &type, &sc)) return 1;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    // A nether fossil also needs ground above the lava sea in its column.
    NetherTerrain nether;
    int fossil = type == Nether_Fossil && initNetherTerrain(&nether, mc, seed);
    int size = sc.regionSize * 16;
    int rx0 = floor_div(x0, size), rx1 = floor_div(x1 - 1, size);
    int rz0 = floor_div(z0, size), rz1 = floor_div(z1 - 1, size);
    for (int rz = rz0; rz <= rz1; rz++)
    {
        if (cancelled && cancelled(user)) return 2;
        for (int rx = rx0; rx <= rx1; rx++)
        {
            Pos p;
            if (!getStructurePos(type, mc, seed, rx, rz, &p)) continue;
            if (p.x < x0 || p.x >= x1 || p.z < z0 || p.z >= z1) continue;
            if (!isViableStructurePos(type, &g, p.x, p.z, 0)) continue;
            if (fossil)
            {
                uint64_t r = chunkGenerateRnd(seed, p.x >> 4, p.z >> 4);
                nextInt(&r, 16);
                nextInt(&r, 16);
                int start = nextInt(&r, 125 - 32 + 1) + 32;
                if (!getNetherFossilY(&nether, p.x, p.z, start)) continue;
            }
            if (*count >= cap) return 3;
            out[*count * 2] = p.x;
            out[*count * 2 + 1] = p.z;
            (*count)++;
        }
    }
    return 0;
}

/* What tells one placed structure from another of its kind, where its start
 * alone decides it (no terrain): the bastion's start piece 0..3 (housing
 * units, hoglin stable, treasure, bridge), 1 for an igloo with a basement,
 * 1 for an End City with a ship. -1 for kinds without such a variant. */
int lumilio_cubiomes_variant(int mc, uint64_t seed, int kind, int x, int z)
{
    if (kind < 0 || kind >= KIND_COUNT || mc < MC_1_0 || mc > MC_NEWEST) return -1;
    int type = KINDS[kind];
    StructureVariant sv;
    switch (type)
    {
    case Bastion:
        if (mc < MC_1_16_1 || !getVariant(&sv, Bastion, mc, seed, x, z, -1)) return -1;
        return sv.start;
    case Igloo:
        if (mc < MC_1_9 || !getVariant(&sv, Igloo, mc, seed, x, z, -1)) return -1;
        return sv.basement;
    case End_City:
    {
        if (mc < MC_1_9) return -1;
        Piece *pieces = malloc(sizeof(Piece) * END_CITY_PIECES_MAX);
        if (!pieces) return -1;
        int n = getEndCityPieces(pieces, seed, x >> 4, z >> 4);
        int ship = 0;
        for (int i = 0; i < n; i++)
            if (pieces[i].type == END_SHIP) ship = 1;
        free(pieces);
        return ship;
    }
    default:
        return -1;
    }
}

int lumilio_cubiomes_decoration_available(int mc, int dim, int type)
{
    return getDecorationEntry(mc, dim, type) != NULL;
}

/* Biome-filtered candidates of one decoration whose column lies inside
 * [x0, x1) x [z0, z1), as x,y,z,flags quads (flags: DecoPos flags).
 * Returns 0 ok, 1 error, 2 cancelled, 3 more candidates than `cap`. */
int lumilio_cubiomes_decorations(int mc, uint64_t seed, int dim, int type,
    int x0, int z0, int x1, int z1, int *out, size_t cap, size_t *count,
    int (*cancelled)(void *), void *user)
{
    *count = 0;
    const DecoEntry *e = getDecorationEntry(mc, dim, type);
    if (!e) return 1;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    int cx0 = floor_div(x0, 16), cx1 = floor_div(x1 - 1, 16);
    int cz0 = floor_div(z0, 16), cz1 = floor_div(z1 - 1, 16);
    DecoPos found[256];
    for (int cz = cz0; cz <= cz1; cz++)
    {
        if (cancelled && cancelled(user)) return 2;
        for (int cx = cx0; cx <= cx1; cx++)
        {
            int n = getDecorations(e, &g, seed, cx, cz, found, 256);
            if (n < 0) return 1;
            for (int i = 0; i < n; i++)
            {
                DecoPos p = found[i];
                if (p.x < x0 || p.x >= x1 || p.z < z0 || p.z >= z1) continue;
                if (*count >= cap) return 3;
                int *o = out + *count * 4;
                o[0] = p.x; o[1] = p.y; o[2] = p.z; o[3] = p.flags;
                (*count)++;
            }
        }
    }
    return 0;
}

/* One raw vein noise at a block (VN_*), for the oracle tests. */
double lumilio_cubiomes_vein_noise(int mc, uint64_t seed, int noise, int x, int y, int z)
{
    OreVeinNoise vn;
    if (noise < 0 || noise >= VN_MAX || !initOreVeinNoise(&vn, mc, seed)) return 0;
    return sampleOreVeinNoise(&vn, noise, x, y, z);
}

/* What the vein rule makes of each block of a column, y from y0 up to y1
 * inclusive (VB_* codes), for the saved-world tests. */
int lumilio_cubiomes_vein_column(int mc, uint64_t seed, int x, int z, int y0, int y1,
    unsigned char *out)
{
    OreVeinNoise vn;
    if (y1 < y0 || y1 - y0 > 512 || !initOreVeinNoise(&vn, mc, seed)) return 1;
    for (int y = y0; y <= y1; y++)
        out[y - y0] = (unsigned char)getOreVeinBlock(&vn, x, y, z);
    return 0;
}

/* Zones of one vein type (VEIN_COPPER or VEIN_IRON) on a grid of `cell`-block
 * squares from (x0, z0), both multiples of 4. The game interpolates the
 * toggle between noise corners 4 blocks apart (8 in height), so a square's
 * strongest block lies near one of the corner columns on or around it; each
 * corner column is read at every corner height inside the vein's range.
 * `out` gets how far past the threshold the square's strongest corner is,
 * or -1 outside any zone. Returns 0 ok, 1 error, 2 cancelled. */
int lumilio_cubiomes_vein_zones(int mc, uint64_t seed, int vein, int x0, int z0,
    int cell, int w, int h, float *out, int (*cancelled)(void *), void *user)
{
    OreVeinNoise vn;
    if ((vein != VEIN_COPPER && vein != VEIN_IRON) || cell < 4 || cell % 4 ||
        (x0 & 3) || (z0 & 3) || w < 1 || h < 1)
        return 1;
    if (!initOreVeinNoise(&vn, mc, seed)) return 1;
    int ylo = vein == VEIN_COPPER ? 0 : -56, yhi = vein == VEIN_COPPER ? 48 : -8;
    int step = cell / 4, cw = w * step + 1, ch = h * step + 1;
    float *corner = malloc(sizeof(float) * (size_t)cw * (size_t)ch);
    if (!corner) return 1;
    for (int j = 0; j < ch; j++)
    {
        if (cancelled && cancelled(user)) { free(corner); return 2; }
        for (int i = 0; i < cw; i++)
        {
            double best = -1;
            for (int y = ylo; y <= yhi; y += 8)
            {
                double strength;
                if (getOreVeinZone(&vn, x0 + 4 * i, y, z0 + 4 * j, &strength) == vein
                    && strength > best)
                    best = strength;
            }
            corner[(size_t)j * cw + i] = (float)best;
        }
    }
    for (int j = 0; j < h; j++)
    {
        for (int i = 0; i < w; i++)
        {
            float best = -1;
            for (int b = 0; b <= step; b++)
                for (int a = 0; a <= step; a++)
                {
                    float v = corner[(size_t)(j * step + b) * cw + (size_t)(i * step + a)];
                    if (v > best) best = v;
                }
            out[(size_t)j * w + i] = best;
        }
    }
    free(corner);
    return 0;
}

/* A pyramid chest's first-pool items for the tests, one line each:
 * "id count", or "minecraft:enchanted_book enchantment level" for a book.
 * Returns the bytes written (excluding the terminator), -1 if too small. */
int lumilio_cubiomes_pyramid_items(int mc, uint64_t lootSeed, char *out, size_t cap)
{
    const PyramidLoot *loot = getPyramidLoot(mc);
    LootDraw draw[16];
    int n = getPyramidLootDraws(loot, lootSeed, draw, 16);
    size_t used = 0;
    if (cap) out[0] = 0;
    for (int i = 0; i < n; i++)
    {
        const LootEntry *e = &loot->entries[draw[i].entry];
        int w;
        if (!e->item)
            continue;
        if (e->kind == LE_ENCHANT && draw[i].enchantment >= 0)
            w = snprintf(out + used, cap - used, "minecraft:enchanted_book %s %d\n",
                e->enchantments[draw[i].enchantment], draw[i].level);
        else
            w = snprintf(out + used, cap - used, "%s %d\n", e->item, draw[i].count);
        if (w < 0 || (size_t)w >= cap - used) return -1;
        used += (size_t)w;
    }
    return (int)used;
}

int lumilio_cubiomes_pyramid_loot_available(int mc)
{
    return getPyramidLoot(mc) != NULL;
}

/* A desert pyramid starting at block (x, z): each chest's x, z, loot seed and
 * enchanted golden apples. Returns the chest count, 0 without release data. */
int lumilio_cubiomes_pyramid_chests(int mc, uint64_t seed, int x, int z,
    int *xz, uint64_t *seeds, int *apples)
{
    StructureChest chest[4];
    const PyramidLoot *loot = getPyramidLoot(mc);
    int n = getDesertPyramidChests(chest, mc, seed, x >> 4, z >> 4);
    for (int i = 0; i < n; i++)
    {
        xz[2 * i] = chest[i].x;
        xz[2 * i + 1] = chest[i].z;
        seeds[i] = chest[i].lootSeed;
        apples[i] = getPyramidEnchantedApples(loot, chest[i].lootSeed);
    }
    return n;
}

/* The height of the nether fossil whose column is (x, z) (a position the
 * Nether_Fossil query returned): its start chunk's structure random draws the
 * column, then the search start in 32..125. 0 when it fails, -1 before 1.18. */
int lumilio_cubiomes_nether_fossil_y(int mc, uint64_t seed, int x, int z)
{
    NetherTerrain nt;
    if (!initNetherTerrain(&nt, mc, seed)) return -1;
    // The start chunk's structure random: the column, then the search start.
    uint64_t r = chunkGenerateRnd(seed, x >> 4, z >> 4);
    nextInt(&r, 16);
    nextInt(&r, 16);
    int start = nextInt(&r, 125 - 32 + 1) + 32;
    return getNetherFossilY(&nt, x, z, start);
}

/* Mth.sin (0) or Mth.cos (1) of a release, for the oracle tests. */
float lumilio_cubiomes_trig(int mc, int cos, float value)
{
    return cos ? mcCos(mc, value) : mcSin(mc, value);
}

/* Order-free checksum of one carved block, as the oracle probe sums them:
 * target chunk and the carving mask's bit index. */
static uint64_t carvedKey(int tx, int tz, int index)
{
    uint64_t k = ((uint64_t)(int64_t)tx * 0x9E3779B1ULL)
        ^ ((uint64_t)(int64_t)tz * 0x85EBCA77ULL)
        ^ ((uint64_t)(int64_t)index * 0xC2B2AE3DULL);
    k *= 0xFF51AFD7ED558CCDULL;
    return k ^ (k >> 33);
}

/* The blocks a ravine starting in chunk (sx, sz) carves in every chunk it can
 * reach (8 chunks around): their number and checksum. Returns 0 when the
 * chunk starts none, 1 otherwise, -1 on error. */
int lumilio_cubiomes_ravine_check(int mc, uint64_t seed, int sx, int sz,
    int64_t *count, uint64_t *checksum)
{
    Ravine *rv = malloc(sizeof(Ravine));
    unsigned char *mask = malloc(256 * 376);
    int found = -1;
    *count = 0;
    *checksum = 0;
    if (rv && mask)
    {
        found = getRavine(rv, mc, seed, sx, sz);
        for (int tz = sz - 8; found == 1 && tz <= sz + 8; tz++)
            for (int tx = sx - 8; tx <= sx + 8; tx++)
            {
                *count += carveRavineChunk(rv, tx, tz, mask);
                for (int i = 0; i < 256 * 376; i++)
                    if (mask[i]) *checksum += carvedKey(tx, tz, i);
            }
    }
    free(rv);
    free(mask);
    return found;
}

/* Ravines whose start chunk lies in [x0, x1) x [z0, z1) (block bounds),
 * packed into `out` as: start x, y, z, underwater (0/1), steps n, then n
 * carved-or-not step centres as x, z (rounded down). `cap` counts ints.
 * Returns 0 ok, 1 error, 2 cancelled, 3 more than `cap`. */
int lumilio_cubiomes_ravines(int mc, uint64_t seed, int x0, int z0, int x1, int z1,
    int *out, size_t cap, size_t *used, int (*cancelled)(void *), void *user)
{
    *used = 0;
    if (mc < MC_1_18) return 1;
    Ravine *rv = malloc(sizeof(Ravine));
    if (!rv) return 1;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, DIM_OVERWORLD, seed);
    int result = 0;
    int cx0 = floor_div(x0, 16), cx1 = floor_div(x1 - 1, 16);
    int cz0 = floor_div(z0, 16), cz1 = floor_div(z1 - 1, 16);
    for (int cz = cz0; cz <= cz1 && result == 0; cz++)
    {
        if (cancelled && cancelled(user)) { result = 2; break; }
        for (int cx = cx0; cx <= cx1; cx++)
        {
            if (!getRavine(rv, mc, seed, cx, cz)) continue;
            size_t need = 5 + 2 * (size_t)rv->steps;
            if (*used + need > cap) { result = 3; break; }
            int *o = out + *used;
            // Under an ocean the ravine fills with water: the start column's
            // biome at sea level decides how the map draws it.
            int biome = getBiomeAt(&g, 4, rv->x >> 2, 63 >> 2, rv->z >> 2);
            o[0] = rv->x; o[1] = rv->y; o[2] = rv->z;
            o[3] = isOceanic(biome);
            o[4] = rv->steps;
            for (int i = 0; i < rv->steps; i++)
            {
                o[5 + 2 * i] = (int)floor(rv->step[i].x);
                o[6 + 2 * i] = (int)floor(rv->step[i].z);
            }
            *used += need;
        }
    }
    free(rv);
    return result;
}

/* Up to `cap` strongholds in generation order (x,z pairs). */
int lumilio_cubiomes_strongholds(int mc, uint64_t seed, int *out, size_t cap,
    size_t *count, int (*cancelled)(void *), void *user)
{
    *count = 0;
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, DIM_OVERWORLD, seed);
    StrongholdIter sh;
    initFirstStronghold(&sh, mc, seed);
    while (*count < cap)
    {
        if (cancelled && cancelled(user)) return 2;
        /* The result counts strongholds from this one on; 0 means none left. */
        if (nextStronghold(&sh, &g) <= 0) break;
        out[*count * 2] = sh.pos.x;
        out[*count * 2 + 1] = sh.pos.z;
        (*count)++;
    }
    return 0;
}

int lumilio_cubiomes_spawn(int mc, uint64_t seed, int dim, int *out)
{
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    Pos p = getSpawn(&g);
    out[0] = p.x;
    out[1] = p.z;
    return 0;
}

/* One byte per chunk, row-major: 1 for a slime chunk. */
void lumilio_cubiomes_slime(uint64_t seed, int cx, int cz, int width, int height,
    unsigned char *out)
{
    for (int z = 0; z < height; z++)
        for (int x = 0; x < width; x++)
            out[(size_t)z * (size_t)width + (size_t)x] = (unsigned char)isSlimeChunk(seed, cx + x, cz + z);
}

void lumilio_cubiomes_colors(unsigned char *out)
{
    unsigned char colors[256][3];
    initBiomeColors(colors);
    memcpy(out, colors, sizeof(colors));
}
