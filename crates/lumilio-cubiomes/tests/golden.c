/* Independent fixture producer using cubiomes generator.h.
 * MIT, Copyright (c) 2020 Cubitect. See SOURCE.md for the exact invocation. */
#include "generator.h"
#include <stdio.h>
#include <stdlib.h>
int main(void)
{
    int versions[] = {MC_1_16_5, MC_1_18_2, MC_1_21_WD};
    int dims[] = {DIM_OVERWORLD, DIM_NETHER, DIM_END};
    int scales[] = {1, 4, 16, 64, 256};
    for (int v = 0; v < 3; v++) for (int d = 0; d < 3; d++)
        for (int s = 0; s < 5; s++) {
            Generator g;
            setupGenerator(&g, versions[v], 0);
            applySeed(&g, dims[d], 262);
            Range r = {scales[s], -2, -2, 4, 4, scales[s] == 1 ? 64 : 16, 1};
            int *cache = allocCache(&g, r);
            if (!cache || genBiomes(&g, cache, r)) return 1;
            for (int i = 0; i < 16; i++) printf("%d%c", cache[i], i == 15 ? '\n' : ' ');
            free(cache);
        }
    return 0;
}
