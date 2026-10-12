/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). The 1.18+ canyon carver: a start chance per
 * chunk, then a random walk of ellipsoids. Constants are the vanilla
 * `canyon` carver's, unchanged from 1.18 to 26.3. */
#include "carvers.h"
#include "biomes.h"
#include "finders.h"

#include <math.h>
#include <string.h>

#define PI 3.141592653589793

/* Mth's 65536-entry sine table, entry by entry. Releases before 1.21.11
 * index it with a float product; later ones with a double. */
static float sinEntry(int mc, int i)
{
    if (mc >= MC_1_21_11)
        return (float)sin(i / 10430.378350470453);
    return (float)sin((double)i * PI * 2.0 / 65536.0);
}

float mcSin(int mc, float value)
{
    if (mc >= MC_1_21_11)
        return sinEntry(mc, (int)((int64_t)((double)value * 10430.378350470453) & 65535));
    return sinEntry(mc, (int)(value * 10430.378f) & 65535);
}

float mcCos(int mc, float value)
{
    if (mc >= MC_1_21_11)
        return sinEntry(mc, (int)((int64_t)((double)value * 10430.378350470453 + 16384.0) & 65535));
    return sinEntry(mc, (int)(value * 10430.378f + 16384.0f) & 65535);
}

/* UniformFloat: nextFloat() * (max - min) + min. */
static float between(uint64_t *r, float min, float max)
{
    return nextFloat(r) * (max - min) + min;
}

int getRavine(Ravine *rv, int mc, uint64_t seed, int chunkX, int chunkZ)
{
    // The canyon is the third carver of every carving Overworld biome.
    uint64_t r = chunkGenerateRnd(seed + 2, chunkX, chunkZ);
    int i, step;

    if (mc < MC_1_18 || !(nextFloat(&r) <= 0.01f))
        return 0;
    memset(rv, 0, sizeof(*rv));
    rv->mc = mc;
    rv->chunkX = chunkX;
    rv->chunkZ = chunkZ;

    // carve(): range 4 chunks, so at most (4 * 2 - 1) * 16 steps.
    rv->x = chunkX * 16 + nextInt(&r, 16);
    rv->y = nextInt(&r, 67 - 10 + 1) + 10;
    rv->z = chunkZ * 16 + nextInt(&r, 16);
    float hRot = nextFloat(&r) * (float)(PI * 2);
    float vRot = between(&r, -0.125f, 0.125f);
    double yScale = 3.0f;
    // TrapezoidFloat(0, 6, plateau 2): 0 + u * 4 + v * 2.
    float a = nextFloat(&r) * 4.0f;
    rv->thickness = 0.0f + a + nextFloat(&r) * 2.0f;
    int distance = (int)(112 * between(&r, 0.75f, 1.0f));
    uint64_t tunnel = nextLong(&r);

    // doCarve(): a fresh random from the tunnel seed.
    setSeed(&r, tunnel);
    float width = 1.0f;
    for (i = 0; i < 384; i++)
    {
        if (i == 0 || nextInt(&r, 3) == 0)
            width = 1.0f + nextFloat(&r) * nextFloat(&r);
        rv->widthFactor[i] = width * width;
    }
    double x = rv->x, y = rv->y, z = rv->z;
    float yRota = 0.0f, xRota = 0.0f;
    if (distance > RAVINE_MAX_STEPS) distance = RAVINE_MAX_STEPS;
    for (step = 0; step < distance; step++)
    {
        RavineStep *s = &rv->step[step];
        double hr = 1.5 + mcSin(mc, step * (float)PI / distance) * rv->thickness;
        double vr = hr * yScale;
        hr *= between(&r, 0.75f, 1.0f);
        // updateVerticalRadius: the default factor 1 and centre factor 0.
        float vm = 1.0f - fabsf(0.5f - (float)step / (float)distance) * 2.0f;
        float factor = 1.0f + 0.0f * vm;
        vr = factor * vr * between(&r, 0.75f, 1.0f);
        float xc = mcCos(mc, vRot);
        float xs = mcSin(mc, vRot);
        x += mcCos(mc, hRot) * xc;
        y += xs;
        z += mcSin(mc, hRot) * xc;
        vRot *= 0.7f;
        vRot += xRota * 0.05f;
        hRot += yRota * 0.05f;
        xRota *= 0.8f;
        yRota *= 0.5f;
        float f1 = nextFloat(&r), f2 = nextFloat(&r), f3 = nextFloat(&r);
        xRota += (f1 - f2) * f3 * 2.0f;
        f1 = nextFloat(&r); f2 = nextFloat(&r); f3 = nextFloat(&r);
        yRota += (f1 - f2) * f3 * 4.0f;
        s->x = x; s->y = y; s->z = z;
        s->hr = hr; s->vr = vr;
        s->carved = nextInt(&r, 4) != 0;
    }
    rv->steps = distance;
    return 1;
}

static int floorD(double v)
{
    int i = (int)v;
    return v < i ? i - 1 : i;
}

int carveRavineChunk(const Ravine *rv, int chunkX, int chunkZ, unsigned char *mask)
{
    // The carving mask spans one above the bottom to 7 below the top.
    const int minY = -63, maxY = 312, height = maxY - minY + 1;
    int minX = chunkX * 16, minZ = chunkZ * 16, n = 0, step;
    double midX = minX + 8, midZ = minZ + 8;

    memset(mask, 0, 256 * (size_t)height);
    for (step = 0; step < rv->steps; step++)
    {
        const RavineStep *s = &rv->step[step];
        if (!s->carved)
            continue;
        // canReach: past here the rest of the walk cannot come back.
        double xd = s->x - midX, zd = s->z - midZ;
        double remaining = rv->steps - step;
        double rr = rv->thickness + 2.0f + 16.0f;
        if (!(xd * xd + zd * zd - remaining * remaining <= rr * rr))
            return n;
        double maxDelta = 16.0 + s->hr * 2.0;
        if (fabs(s->x - midX) > maxDelta || fabs(s->z - midZ) > maxDelta)
            continue;
        int x0 = floorD(s->x - s->hr) - minX - 1, x1 = floorD(s->x + s->hr) - minX;
        int z0 = floorD(s->z - s->hr) - minZ - 1, z1 = floorD(s->z + s->hr) - minZ;
        int y0 = floorD(s->y - s->vr) - 1, y1 = floorD(s->y + s->vr) + 1;
        if (x0 < 0) x0 = 0;
        if (z0 < 0) z0 = 0;
        if (x1 > 15) x1 = 15;
        if (z1 > 15) z1 = 15;
        if (y0 < minY) y0 = minY;
        if (y1 > maxY) y1 = maxY;
        for (int xi = x0; xi <= x1; xi++)
        {
            double ex = (minX + xi + 0.5 - s->x) / s->hr;
            for (int zi = z0; zi <= z1; zi++)
            {
                double ez = (minZ + zi + 0.5 - s->z) / s->hr;
                if (ex * ex + ez * ez >= 1.0)
                    continue;
                for (int y = y1; y > y0; y--)
                {
                    double ey = (y - 0.5 - s->y) / s->vr;
                    // shouldSkip: the width factor one below this height.
                    if ((ex * ex + ez * ez) * rv->widthFactor[y + 64 - 1] + ey * ey / 6.0 >= 1.0)
                        continue;
                    unsigned char *bit = &mask[(xi * 16 + zi) * height + (y - minY)];
                    if (!*bit)
                    {
                        *bit = 1;
                        n++;
                    }
                }
            }
        }
    }
    return n;
}
