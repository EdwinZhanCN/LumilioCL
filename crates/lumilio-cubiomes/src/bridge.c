/* Uses cubiomes generator.h (MIT, Copyright (c) 2020 Cubitect).
 * The bridge is LumilioCL code; upstream structs stay on the C side. */
#include "finders.h"
#include "generator.h"
#include "util.h"
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
            if (*count >= cap) return 3;
            out[*count * 2] = p.x;
            out[*count * 2 + 1] = p.z;
            (*count)++;
        }
    }
    return 0;
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
