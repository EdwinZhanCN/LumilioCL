#ifndef DECORATIONS_H_
#define DECORATIONS_H_

/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Decoration features: in each chunk the decoration
 * random runs a placed feature's placement modifiers, then the feature itself.
 * The per-release data (decoration step and index, modifiers, the biomes that
 * list a feature) is tables/decorations.h, generated from the original game
 * data by `cargo xtask cubiomes-tables`.
 */

#include "generator.h"

#ifdef __cplusplus
extern "C"
{
#endif

enum DecorationType
{
    Deco_Fossil_Upper,          // fossil with coal (fossil_upper)
    Deco_Fossil_Lower,          // deep fossil with diamonds (fossil_lower)
    Deco_Lava_Lake_Surface,     // lake_lava_surface
    Deco_Lava_Lake_Underground, // lake_lava_underground
    Deco_Monster_Room,          // monster_room
    Deco_Monster_Room_Deep,     // monster_room_deep
    Deco_Sulfur_Spring,         // rooted_sulfur_spring (26.2+)
    Deco_Geode,                 // amethyst_geode
    Deco_Desert_Well,           // desert_well
    Deco_End_Island,            // end_island_decorated
    Deco_End_Gateway,           // end_gateway_return
    DECORATION_NUM
};

/* Placement modifiers, as the generator encodes them. */
enum
{
    DP_END,
    DP_RARITY,          // a: chance (1 in a)
    DP_IN_SQUARE,
    DP_COUNT,           // a..b, c: drawn (else the constant a)
    DP_COUNT_WEIGHTED,  // value a with weight b, value c with weight d
    DP_HEIGHT_UNIFORM,  // anchors (a, b) to (c, d); kinds 0 absolute,
                        // 1 above bottom, 2 below top
    DP_OFFSET,          // axis a (0 x, 1 y, 2 z) by b..c, d: drawn
    DP_HEIGHTMAP,       // y becomes a surface height
    DP_BIOME,           // the biome at the position lists the feature
    DP_TERRAIN,         // a block or height filter; draws nothing
};

STRUCT(DecoOp)
{
    int op, a, b, c, d;
};

STRUCT(DecoEntry)
{
    int mc, dim, type;
    int salt;           // 10000 * step + index
    int minY, height;   // the placement context's generator bounds
    const DecoOp *ops;
    const uint64_t *biomes; // 256-bit set of biome ids listing the feature
};

enum
{
    DECO_SURFACE = 0x01, // y follows a heightmap, which is not computed
    DECO_TERRAIN = 0x02, // a block or height filter decides placement
    DECO_AFTER   = 0x04, // an earlier candidate of this chunk was placed first,
                         // and how many random numbers it drew is unknown
};

STRUCT(DecoPos)
{
    int x, y, z;
    int flags;
};

/* The release table entry, or NULL when the release or dimension lacks it. */
const DecoEntry *getDecorationEntry(int mc, int dim, int type);

/* The candidate positions of one decoration in chunk (chunkX, chunkZ), in the
 * order the game places them. A generator `g` with the world seed applied
 * (same mc and dimension as the entry) applies the biome filter; with NULL,
 * every candidate is kept as if each one failed it, which is the raw random
 * sequence. Returns the number of positions written (at most `cap`).
 */
int getDecorations(const DecoEntry *entry, const Generator *g, uint64_t seed,
        int chunkX, int chunkZ, DecoPos *out, int cap);

#ifdef __cplusplus
}
#endif

#endif /* DECORATIONS_H_ */
