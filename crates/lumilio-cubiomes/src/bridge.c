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

void lumilio_cubiomes_colors(unsigned char *out)
{
    unsigned char colors[256][3];
    initBiomeColors(colors);
    memcpy(out, colors, sizeof(colors));
}
