/* Uses cubiomes generator.h (MIT, Copyright (c) 2020 Cubitect).
 * The bridge is LumilioCL code; upstream structs stay on the C side. */
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

int lumilio_cubiomes_generate(int mc, uint64_t seed, int dim,
    int scale, int x, int z, int width, int height, int *out, size_t count)
{
    Generator g;
    setupGenerator(&g, mc, 0);
    applySeed(&g, dim, seed);
    Range r = {scale, x, z, width, height, scale == 1 ? 64 : 16, 1};
    if (count != (size_t)width * (size_t)height) return 1;
    int *cache = allocCache(&g, r);
    if (!cache) return 1;
    int result = genBiomes(&g, cache, r);
    if (result == 0) memcpy(out, cache, count * sizeof(int));
    free(cache);
    return result;
}

void lumilio_cubiomes_colors(unsigned char *out)
{
    unsigned char colors[256][3];
    initBiomeColors(colors);
    memcpy(out, colors, sizeof(colors));
}
