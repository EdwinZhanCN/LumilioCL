/* Independent fixture producer using cubiomes finders.h.
 * MIT, Copyright (c) 2020 Cubitect. See SOURCE.md for the exact invocation.
 * It walks regions itself rather than sharing the Rust bridge's loop. */
#include "finders.h"
#include <stdio.h>
#include <stdlib.h>

static const int KINDS[] = {
    Desert_Pyramid, Jungle_Pyramid, Swamp_Hut, Igloo, Village, Ocean_Ruin,
    Shipwreck, Monument, Mansion, Outpost, Ruined_Portal, Ancient_City,
    Trail_Ruins, Trial_Chambers, Fortress, Bastion, End_City,
};

int main(void)
{
    int versions[] = {MC_1_16_5, MC_1_18_2, MC_1_21_WD};
    int dims[] = {DIM_OVERWORLD, DIM_NETHER, DIM_END};
    uint64_t seeds[] = {262, 9876543210ULL};
    const int lo = -1536, hi = 1536;
    for (int s = 0; s < 2; s++)
    for (int v = 0; v < 3; v++)
    {
        int mc = versions[v];
        uint64_t seed = seeds[s];
        for (int d = 0; d < 3; d++)
        {
            Generator g;
            setupGenerator(&g, mc, 0);
            applySeed(&g, dims[d], seed);
            for (int k = 0; k < 17; k++)
            {
                int type = KINDS[k];
                if (type == Ruined_Portal && dims[d] == DIM_NETHER) type = Ruined_Portal_N;
                StructureConfig sc;
                if (!getStructureConfig(type, mc, &sc) || sc.dim != dims[d]) continue;
                int size = sc.regionSize * 16;
                printf("S %d %d %d %d", s, v, d, k);
                for (int rz = (lo < 0 ? -(-lo / size) - 1 : lo / size); rz <= (hi - 1) / size; rz++)
                    for (int rx = (lo < 0 ? -(-lo / size) - 1 : lo / size); rx <= (hi - 1) / size; rx++)
                    {
                        Pos p;
                        if (!getStructurePos(type, mc, seed, rx, rz, &p)) continue;
                        if (p.x < lo || p.x >= hi || p.z < lo || p.z >= hi) continue;
                        if (!isViableStructurePos(type, &g, p.x, p.z, 0)) continue;
                        printf(" %d,%d", p.x, p.z);
                    }
                printf("\n");
            }
        }
        Generator g;
        setupGenerator(&g, mc, 0);
        applySeed(&g, DIM_OVERWORLD, seed);
        StrongholdIter sh;
        initFirstStronghold(&sh, mc, seed);
        printf("H %d %d", s, v);
        for (int i = 0; i < 6 && nextStronghold(&sh, &g) > 0; i++)
            printf(" %d,%d", sh.pos.x, sh.pos.z);
        printf("\n");
        Pos sp = getSpawn(&g);
        printf("P %d %d %d,%d\n", s, v, sp.x, sp.z);
        printf("L %d %d ", s, v);
        for (int z = -4; z < 4; z++)
            for (int x = -4; x < 4; x++) putchar(isSlimeChunk(seed, x, z) ? '1' : '0');
        printf("\n");
    }
    return 0;
}
