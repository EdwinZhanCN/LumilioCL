#ifndef VEINS_H_
#define VEINS_H_

/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Large ore veins (1.18+): the Overworld noise
 * router's vein toggle decides where copper (above) and iron (below) veins
 * run; ridge noises and per-block chances decide which stone inside a vein
 * becomes ore, granite/tuff filler or stays stone.
 */

#include "noise.h"

#ifdef __cplusplus
extern "C"
{
#endif

enum
{
    VN_VEININESS,   // minecraft:ore_veininess, read at 1.5x the block position
    VN_VEIN_A,      // minecraft:ore_vein_a, 4x
    VN_VEIN_B,      // minecraft:ore_vein_b, 4x
    VN_GAP,         // minecraft:ore_gap, 1x
    VN_MAX
};

enum { VEIN_NONE, VEIN_COPPER, VEIN_IRON };

/* What a vein makes of a stone block. */
enum
{
    VB_NONE,        // not part of a vein: the stone stays
    VB_GRANITE,     // copper vein filler
    VB_COPPER_ORE,
    VB_RAW_COPPER,  // raw copper block
    VB_TUFF,        // iron vein filler
    VB_IRON_ORE,    // deepslate iron ore
    VB_RAW_IRON,    // raw iron block
};

STRUCT(OreVeinNoise)
{
    int mc;
    uint64_t oreLo, oreHi; // the per-block random's positional factory
    DoublePerlinNoise noise[VN_MAX];
    PerlinNoise oct[VN_MAX * 2];
};

/* Seeds the vein noises for a world; returns 0 for releases without veins. */
int initOreVeinNoise(OreVeinNoise *vn, int mc, uint64_t seed);

/* One vein noise at the block position its density function reads. */
double sampleOreVeinNoise(const OreVeinNoise *vn, int noise, int x, int y, int z);

/* The vein toggle at a block, interpolated within its 4x8x4 noise cell as
 * the game does; positive for copper, negative for iron. */
double getVeinToggle(const OreVeinNoise *vn, int x, int y, int z);

/* The vein whose zone contains block (x, y, z), or VEIN_NONE. A zone is where
 * the toggle, with the edge roundoff near the vein's height limits, reaches
 * the threshold; `strength` (may be NULL) gets how far past it. 26.3 runs the
 * same rule in single precision, so a block right at a threshold may differ.
 */
int getOreVeinZone(const OreVeinNoise *vn, int x, int y, int z, double *strength);

/* What the vein rule makes of block (x, y, z) if it is stone (VB_*). Caves,
 * aquifers and later features can still remove or replace it. */
int getOreVeinBlock(const OreVeinNoise *vn, int x, int y, int z);

#ifdef __cplusplus
}
#endif

#endif /* VEINS_H_ */
