/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). */
#include "terrain.h"
#include "biomes.h"
#include "rng.h"

#include <math.h>

void initBlendedNoise(BlendedNoise *bn, uint64_t *legacySeed, double xzScale,
        double yScale, double xzFactor, double yFactor, double smear)
{
    // The limits first, then the main noise, from one legacy random.
    octaveInit(&bn->minLimit, legacySeed, bn->oct + 0, -15, 16);
    octaveInit(&bn->maxLimit, legacySeed, bn->oct + 16, -15, 16);
    octaveInit(&bn->main, legacySeed, bn->oct + 32, -7, 8);
    bn->xzMultiplier = 684.412 * xzScale;
    bn->yMultiplier = 684.412 * yScale;
    bn->xzFactor = xzFactor;
    bn->yFactor = yFactor;
    bn->smear = smear;
}

double sampleBlendedNoise(const BlendedNoise *bn, int x, int y, int z)
{
    double d = x * bn->xzMultiplier, e = y * bn->yMultiplier, f = z * bn->xzMultiplier;
    double g = d / bn->xzFactor, h = e / bn->yFactor, i = f / bn->xzFactor;
    double j = bn->yMultiplier * bn->smear;
    double k = j / bn->yFactor;
    double low = 0, high = 0, main = 0, o = 1.0;
    int p;

    for (p = 0; p < 8; p++)
    {
        main += samplePerlin(&bn->main.octaves[p], maintainPrecision(g * o),
            maintainPrecision(h * o), maintainPrecision(i * o), k * o, h * o) / o;
        o /= 2.0;
    }
    double q = (main / 10.0 + 1.0) / 2.0;
    int above = q >= 1.0, below = q <= 0.0;
    o = 1.0;
    for (p = 0; p < 16; p++)
    {
        double s = maintainPrecision(d * o), t = maintainPrecision(e * o);
        double u = maintainPrecision(f * o), v = j * o;
        if (!above)
            low += samplePerlin(&bn->minLimit.octaves[p], s, t, u, v, e * o) / o;
        if (!below)
            high += samplePerlin(&bn->maxLimit.octaves[p], s, t, u, v, e * o) / o;
        o /= 2.0;
    }
    // Mth.clampedLerp(q, low / 512, high / 512) / 128
    double a = low / 512.0, b = high / 512.0;
    double v = q < 0 ? a : q > 1 ? b : a + q * (b - a);
    return v / 128.0;
}

int initNetherTerrain(NetherTerrain *nt, int mc, uint64_t seed)
{
    nt->mc = mc;
    if (mc < MC_1_18)
        return 0;
    // The Nether keeps the legacy random source: the terrain noise is seeded
    // straight from the world seed.
    uint64_t s;
    setSeed(&s, seed);
    initBlendedNoise(&nt->base, &s, 0.25, 0.375, 80.0, 60.0, 8.0);
    return 1;
}

static double clampedMap(double v, double inMin, double inMax, double outMin, double outMax)
{
    double t = (v - inMin) / (inMax - inMin);
    return t < 0 ? outMin : t > 1 ? outMax : outMin + t * (outMax - outMin);
}

/* The density inside the interpolation: the base noise between a ceiling
 * slide (y 104 to 128 toward 0.9375) and a floor slide (24 down to -8
 * toward 2.5). */
static double netherCorner(const NetherTerrain *nt, int x, int y, int z)
{
    double base = sampleBlendedNoise(&nt->base, x, y, z);
    double top = clampedMap(y, 104, 128, 1.0, 0.0);
    double bottom = clampedMap(y, -8, 24, 0.0, 1.0);
    double v = 0.9375 + top * (-0.9375 + base);
    return 2.5 + bottom * (-2.5 + v);
}

double getNetherDensity(const NetherTerrain *nt, int x, int y, int z)
{
    int x0 = x & ~3, z0 = z & ~3, y0 = y & ~7;
    double dx = (x - x0) / 4.0, dy = (y - y0) / 8.0, dz = (z - z0) / 4.0;
    double v000 = netherCorner(nt, x0,     y0,     z0);
    double v100 = netherCorner(nt, x0 + 4, y0,     z0);
    double v010 = netherCorner(nt, x0,     y0 + 8, z0);
    double v110 = netherCorner(nt, x0 + 4, y0 + 8, z0);
    double v001 = netherCorner(nt, x0,     y0,     z0 + 4);
    double v101 = netherCorner(nt, x0 + 4, y0,     z0 + 4);
    double v011 = netherCorner(nt, x0,     y0 + 8, z0 + 4);
    double v111 = netherCorner(nt, x0 + 4, y0 + 8, z0 + 4);
    double d = lerp(dz,
        lerp(dx, lerp(dy, v000, v010), lerp(dy, v100, v110)),
        lerp(dx, lerp(dy, v001, v011), lerp(dy, v101, v111)));
    // squeeze(0.64 * d)
    d *= 0.64;
    d = d < -1 ? -1 : d > 1 ? 1 : d;
    return d / 2.0 - d * d * d / 24.0;
}

int getNetherFossilY(const NetherTerrain *nt, int x, int z, int startY)
{
    // Walk down from the drawn height to the first air block standing on
    // solid ground; the lava sea (y 32 and below) ends the search.
    int y = startY;
    while (y > 32)
    {
        int air = !(getNetherDensity(nt, x, y, z) > 0);
        y--;
        if (air && getNetherDensity(nt, x, y, z) > 0)
            return y;
    }
    return 0;
}
