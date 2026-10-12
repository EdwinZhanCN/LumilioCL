#ifndef CARVERS_H_
#define CARVERS_H_

/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Ravines: the Overworld's canyon carver (1.18+).
 * Whether a chunk starts one and the path it walks depend on the seed alone;
 * which blocks it opens depends on the terrain it cuts through.
 */

#include "rng.h"

#ifdef __cplusplus
extern "C"
{
#endif

enum { RAVINE_MAX_STEPS = 112 };

STRUCT(RavineStep)
{
    double x, y, z;       // centre of this step's ellipsoid
    double hr, vr;        // horizontal and vertical radius
    int carved;           // 0: the step skips carving (one in four)
};

STRUCT(Ravine)
{
    int mc;
    int chunkX, chunkZ;   // the start chunk
    int x, y, z;          // the start block
    float thickness;
    int steps;
    RavineStep step[RAVINE_MAX_STEPS];
    float widthFactor[384]; // per height above the world's bottom
};

/* Whether the canyon carver starts in chunk (chunkX, chunkZ) of a 1.18+
 * Overworld; if so, walks its whole path into `rv`. */
int getRavine(Ravine *rv, int mc, uint64_t seed, int chunkX, int chunkZ);

/* The blocks the ravine carves in chunk (chunkX, chunkZ), as the game's
 * carving mask: `mask[(x * 16 + z) * 376 + (y + 63)]` for local x, z and
 * y from -63 to 312, set to 1. Returns the number of blocks set. */
int carveRavineChunk(const Ravine *rv, int chunkX, int chunkZ, unsigned char *mask);

/* Mth.sin/cos of the release, from the game's 65536-entry table. */
float mcSin(int mc, float value);
float mcCos(int mc, float value);

#ifdef __cplusplus
}
#endif

#endif /* CARVERS_H_ */
