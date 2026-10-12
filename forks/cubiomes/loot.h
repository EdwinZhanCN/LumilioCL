#ifndef LOOT_H_
#define LOOT_H_

/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Structure chests whose loot seed the seed alone
 * decides, and the loot that seed fills them with. The per-release data is
 * tables/loot.h, generated from the original game data by
 * `cargo xtask cubiomes-tables`.
 */

#include "rng.h"

#ifdef __cplusplus
extern "C"
{
#endif

/* What drawing one loot entry costs and gives. */
enum
{
    LE_PLAIN,   // an item or nothing, no further draws
    LE_COUNT,   // a set_count from a to b: one draw when b > a
    LE_ENCHANT, // an enchanted book: one enchantment, then a level if > 1
    LE_TARGET,  // the enchanted golden apple
};

STRUCT(LootEntry)
{
    int weight, kind, a, b;
    const int *levels;  // LE_ENCHANT: each option's maximum level
    int levelCount;
    const char *item;   // NULL for an empty entry
    const char *const *enchantments; // LE_ENCHANT: each option's id
};

/* One roll of the pool: the entry drawn, its count and, for a book, the
 * enchantment option and level. */
STRUCT(LootDraw)
{
    int entry, count, enchantment, level;
};

STRUCT(PyramidLoot)
{
    int mc, step, index;    // the pyramid's decoration step and place in it
    int rollsMin, rollsMax; // the first pool's rolls
    int entryCount;
    const LootEntry *entries;
};

STRUCT(StructureChest)
{
    int x, z;           // the chest's column; its height follows the ground
    uint64_t lootSeed;  // the seed the chest's loot table is filled with
};

/* The release's desert pyramid loot, or NULL before 1.21. */
const PyramidLoot *getPyramidLoot(int mc);

/* The four chests of a desert pyramid starting in chunk (chunkX, chunkZ), in
 * the order the game creates them. Returns 4, or 0 without release data. */
int getDesertPyramidChests(StructureChest out[4], int mc, uint64_t seed,
        int chunkX, int chunkZ);

/* Every roll of a desert pyramid chest's first pool, the one holding the
 * apple; returns the number of rolls (at most `cap`). */
int getPyramidLootDraws(const PyramidLoot *loot, uint64_t lootSeed, LootDraw *out, int cap);

/* How many enchanted golden apples a desert pyramid chest with this loot
 * seed holds. */
int getPyramidEnchantedApples(const PyramidLoot *loot, uint64_t lootSeed);

#ifdef __cplusplus
}
#endif

#endif /* LOOT_H_ */
