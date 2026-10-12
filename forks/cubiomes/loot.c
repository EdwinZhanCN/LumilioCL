/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). */
#include "loot.h"
#include "biomes.h"
#include "finders.h"

#include "tables/loot.h"

// The chunk's decoration seed; defined in finders.c without a header entry.
uint64_t getPopulationSeed(int mc, uint64_t ws, int x, int z);

const PyramidLoot *getPyramidLoot(int mc)
{
    size_t i;
    for (i = 0; i < sizeof(PYRAMID_LOOT) / sizeof(PYRAMID_LOOT[0]); i++)
        if (PYRAMID_LOOT[i].mc == mc)
            return &PYRAMID_LOOT[i];
    return NULL;
}

int getDesertPyramidChests(StructureChest out[4], int mc, uint64_t seed,
        int chunkX, int chunkZ)
{
    const PyramidLoot *loot = getPyramidLoot(mc);
    if (!loot)
        return 0;
    // The piece faces a random horizontal direction (north, east, south,
    // west), the first draw of the start chunk's structure random.
    uint64_t r = chunkGenerateRnd(seed, chunkX, chunkZ);
    int facing = nextInt(&r, 4);
    int minX = chunkX * 16, minZ = chunkZ * 16, maxX = minX + 20, maxZ = minZ + 20;

    // All four chests lie in the start chunk. Its decoration random draws the
    // height offset, then one loot seed per chest in direction order.
    Xoroshiro xr;
    xSetSeed(&xr, getPopulationSeed(mc, seed, minX, minZ) + loot->index + 10000 * loot->step);
    xNextIntJ(&xr, 3);
    static const int step[4][2] = { {0, -1}, {1, 0}, {0, 1}, {-1, 0} };
    for (int d = 0; d < 4; d++)
    {
        int lx = 10 + step[d][0] * 2, lz = 10 + step[d][1] * 2;
        switch (facing)
        {
        case 0: out[d].x = minX + lx; out[d].z = maxZ - lz; break; // north
        case 1: out[d].x = minX + lz; out[d].z = minZ + lx; break; // east
        case 2: out[d].x = minX + lx; out[d].z = minZ + lz; break; // south
        default: out[d].x = maxX - lz; out[d].z = minZ + lx; break; // west
        }
        out[d].lootSeed = xNextLongJ(&xr);
    }
    return 4;
}

/* Mth.nextInt: min itself when the range is empty, else one draw. */
static int between(uint64_t *r, int min, int max)
{
    return min >= max ? min : nextInt(r, max - min + 1) + min;
}

int getPyramidLootDraws(const PyramidLoot *loot, uint64_t lootSeed, LootDraw *out, int cap)
{
    uint64_t r;
    int total = 0, i, roll, rolls;

    if (!loot)
        return 0;
    // The loot context's random: a Java random seeded with the loot seed.
    setSeed(&r, lootSeed);
    for (i = 0; i < loot->entryCount; i++)
        total += loot->entries[i].weight;
    rolls = between(&r, loot->rollsMin, loot->rollsMax);
    for (roll = 0; roll < rolls && roll < cap; roll++)
    {
        LootDraw *d = &out[roll];
        d->entry = 0;
        if (loot->entryCount > 1)
        {
            int pick = nextInt(&r, total);
            for (i = 0; i < loot->entryCount; i++)
            {
                pick -= loot->entries[i].weight;
                if (pick < 0)
                    break;
            }
            d->entry = i;
        }
        const LootEntry *e = &loot->entries[d->entry];
        d->count = e->item ? 1 : 0;
        d->enchantment = -1;
        d->level = 0;
        if (e->kind == LE_COUNT)
            d->count = between(&r, e->a, e->b);
        else if (e->kind == LE_ENCHANT && e->levelCount > 0)
        {
            d->enchantment = nextInt(&r, e->levelCount);
            d->level = between(&r, 1, e->levels[d->enchantment]);
        }
    }
    return roll;
}

int getPyramidEnchantedApples(const PyramidLoot *loot, uint64_t lootSeed)
{
    LootDraw draw[16];
    int n = getPyramidLootDraws(loot, lootSeed, draw, 16), found = 0;
    for (int i = 0; i < n; i++)
        if (loot->entries[draw[i].entry].kind == LE_TARGET)
            found++;
    return found;
}
