/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Evaluates a placed feature's placement modifiers
 * the way the original game's feature placer runs them: depth first, each
 * modifier on one position at a time, the feature placed at every position
 * that passes the last one. */
#include "decorations.h"
#include "finders.h"
#include "rng.h"

#include "tables/decorations.h"

// The chunk's decoration seed; defined in finders.c without a header entry.
uint64_t getPopulationSeed(int mc, uint64_t ws, int x, int z);

const DecoEntry *getDecorationEntry(int mc, int dim, int type)
{
    size_t i;
    for (i = 0; i < sizeof(DECORATIONS) / sizeof(DECORATIONS[0]); i++)
    {
        const DecoEntry *e = &DECORATIONS[i];
        if (e->mc == mc && e->dim == dim && e->type == type)
            return e;
    }
    return NULL;
}

static int anchorY(const DecoEntry *e, int kind, int value)
{
    switch (kind)
    {
    case 1: return e->minY + value;                 // above bottom
    case 2: return e->minY + e->height - 1 - value; // below top
    default: return value;                          // absolute
    }
}

static int between(Xoroshiro *xr, int min, int max)
{
    return xNextIntJ(xr, (uint32_t)(max - min + 1)) + min;
}

static int listed(const DecoEntry *e, int id)
{
    return id >= 0 && id < 256 && ((e->biomes[id >> 6] >> (id & 63)) & 1);
}

typedef struct { int x, y, z, op, flags; } Frame;

#define DECO_STACK 256

int getDecorations(const DecoEntry *e, const Generator *g, uint64_t seed,
        int chunkX, int chunkZ, DecoPos *out, int cap)
{
    if (!e)
        return 0;
    Xoroshiro xr;
    xSetSeed(&xr, getPopulationSeed(e->mc, seed, chunkX << 4, chunkZ << 4) + e->salt);

    Frame stack[DECO_STACK];
    Frame made[DECO_STACK];
    int top = 0, n = 0, placed = 0;
    // The decoration origin is the chunk's lowest section.
    stack[top++] = (Frame) { chunkX << 4, e->minY, chunkZ << 4, 0, 0 };

    while (top > 0)
    {
        Frame f = stack[--top];
        const DecoOp *op = &e->ops[f.op];
        int k = 0, i, count, sampleY, id;

        switch (op->op)
        {
        case DP_RARITY:
            if (xNextFloat(&xr) < 1.0f / (float)op->a)
                made[k++] = f;
            break;
        case DP_IN_SQUARE:
            f.x += xNextIntJ(&xr, 16);
            f.z += xNextIntJ(&xr, 16);
            made[k++] = f;
            break;
        case DP_COUNT:
        case DP_COUNT_WEIGHTED:
            if (op->op == DP_COUNT)
                count = op->c ? between(&xr, op->a, op->b) : op->a;
            else
                count = (int)xNextIntJ(&xr, (uint32_t)(op->b + op->d)) < op->b ? op->a : op->c;
            if (count < 0 || count > DECO_STACK)
                return -1;
            for (i = 0; i < count; i++)
                made[k++] = f;
            break;
        case DP_HEIGHT_UNIFORM:
        {
            int min = anchorY(e, op->a, op->b);
            int max = anchorY(e, op->c, op->d);
            f.y = min > max ? min : between(&xr, min, max);
            made[k++] = f;
            break;
        }
        case DP_OFFSET:
        {
            int v = op->d ? between(&xr, op->b, op->c) : op->b;
            if (op->a == 0) f.x += v;
            else if (op->a == 1) f.y += v;
            else f.z += v;
            made[k++] = f;
            break;
        }
        case DP_HEIGHTMAP:
            f.flags |= DECO_SURFACE;
            made[k++] = f;
            break;
        case DP_BIOME:
            if (g)
            {
                // A surface height is not computed: sample at sea level.
                sampleY = (f.flags & DECO_SURFACE) ? 63 : f.y;
                id = getBiomeAt(g, 1, f.x, sampleY, f.z);
                if (!listed(e, id))
                    break;
            }
            made[k++] = f;
            break;
        case DP_TERRAIN:
            f.flags |= DECO_TERRAIN;
            made[k++] = f;
            break;
        default:
            return -1;
        }

        if (e->ops[f.op + 1].op != DP_END)
        {
            if (top + k > DECO_STACK)
                return -1;
            for (i = k - 1; i >= 0; i--)
            {
                made[i].op = f.op + 1;
                stack[top++] = made[i];
            }
            continue;
        }
        for (i = 0; i < k; i++)
        {
            if (n >= cap)
                return n;
            out[n].x = made[i].x;
            out[n].y = made[i].y;
            out[n].z = made[i].z;
            out[n].flags = made[i].flags | (placed ? DECO_AFTER : 0);
            n++;
            // The game places the feature here, drawing an unknown number of
            // random numbers before the next candidate, unless no biome
            // filter is applied (the raw sequence assumes none placed).
            if (g)
                placed = 1;
        }
    }
    return n;
}
