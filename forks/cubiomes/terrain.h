#ifndef TERRAIN_H_
#define TERRAIN_H_

/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Terrain density: whether a block of the noise
 * terrain is solid, before surface rules, carvers and features. So far the
 * 1.18+ Nether, whose density is one blended noise between two slides.
 */

#include "noise.h"

#ifdef __cplusplus
extern "C"
{
#endif

/* The 1.18+ "old blended noise": a main noise choosing between two limit
 * noises, each with legacy octave initialisation. */
STRUCT(BlendedNoise)
{
    double xzMultiplier, yMultiplier, xzFactor, yFactor, smear;
    OctaveNoise minLimit, maxLimit, main;
    PerlinNoise oct[16 + 16 + 8];
};

void initBlendedNoise(BlendedNoise *bn, uint64_t *legacySeed, double xzScale,
        double yScale, double xzFactor, double yFactor, double smear);
double sampleBlendedNoise(const BlendedNoise *bn, int x, int y, int z);

STRUCT(NetherTerrain)
{
    int mc;
    BlendedNoise base;
};

/* Returns 0 before 1.18. */
int initNetherTerrain(NetherTerrain *nt, int mc, uint64_t seed);

/* The Nether's final density at a block (> 0 is netherrack), interpolated
 * within its 4x8x4 noise cell as the noise chunk does. */
double getNetherDensity(const NetherTerrain *nt, int x, int y, int z);

/* A nether fossil's height for its column (x, z), starting the downward
 * search at `startY` (drawn from 32..125 by the structure); 0 when it finds
 * no air above solid ground above the lava sea and the fossil fails. */
int getNetherFossilY(const NetherTerrain *nt, int x, int z, int startY);

#ifdef __cplusplus
}
#endif

#endif /* TERRAIN_H_ */
