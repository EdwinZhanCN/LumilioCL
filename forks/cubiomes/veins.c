/* LumilioCL addition to this cubiomes snapshot, under the directory's MIT
 * license (see LUMILIO.md). Ore veins of the 1.18+ Overworld, as the noise
 * router (1.18–26.2) and the 26.3 ore vein material rules place them. */
#include "veins.h"
#include "biomes.h"
#include "rng.h"

int initOreVeinNoise(OreVeinNoise *vn, int mc, uint64_t seed)
{
    static const double amp[] = {1};
    // The world's positional random hashes each name (md5, as RandomState).
    static const uint64_t md5[VN_MAX][2] = {
        {0x6b86c7820a307171, 0xd87fb0fefd9c1624}, // minecraft:ore_veininess
        {0x4cd8d69b9a841649, 0xcdd63f17bfe8f5ed}, // minecraft:ore_vein_a
        {0x6b26220b31f7c6c9, 0xae077edebf6aaec1}, // minecraft:ore_vein_b
        {0x9c4cc6b2fb0be4bb, 0xbd5964705573bb5e}, // minecraft:ore_gap
    };
    static const int firstOctave[VN_MAX] = {-8, -7, -7, -5};
    Xoroshiro pxr;
    uint64_t xlo, xhi;
    int i;

    vn->mc = mc;
    if (mc < MC_1_18)
        return 0;
    xSetSeed(&pxr, seed);
    xlo = xNextLong(&pxr);
    xhi = xNextLong(&pxr);
    for (i = 0; i < VN_MAX; i++)
    {
        pxr.lo = xlo ^ md5[i][0];
        pxr.hi = xhi ^ md5[i][1];
        xDoublePerlinInit(&vn->noise[i], &pxr, vn->oct + 2*i, amp, firstOctave[i], 1, -1);
    }
    // The per-block random: the factory of "minecraft:ore", forked.
    pxr.lo = xlo ^ 0x9b88124de600116d;
    pxr.hi = xhi ^ 0x2ae68055aa4a7761;
    vn->oreLo = xNextLong(&pxr);
    vn->oreHi = xNextLong(&pxr);
    return 1;
}

double sampleOreVeinNoise(const OreVeinNoise *vn, int noise, int x, int y, int z)
{
    static const double scale[VN_MAX] = {1.5, 4.0, 4.0, 1.0};
    double s = scale[noise];
    return sampleDoublePerlin(&vn->noise[noise], x * s, y * s, z * s);
}

/* The toggle and ridge noises are limited to the veins' heights (outside:
 * 0, or 1 for the ridges since 26.3) and read at the corners of 4x8x4 cells. */
static double cornerValue(const OreVeinNoise *vn, int noise, int x, int y, int z)
{
    int newer = vn->mc >= MC_26_3;
    if (y < (newer ? -64 : -60) || y >= (newer ? 57 : 51))
        return (newer && noise != VN_VEININESS) ? 1.0 : 0.0;
    return sampleOreVeinNoise(vn, noise, x, y, z);
}

/* Trilinear between the cell corners, in the order the noise chunk
 * interpolates: y, then x, then z. */
static double interpolated(const OreVeinNoise *vn, int noise, int x, int y, int z)
{
    int x0 = x & ~3, z0 = z & ~3;
    int y0 = ((y + 64) & ~7) - 64;
    double dx = (x - x0) / 4.0, dy = (y - y0) / 8.0, dz = (z - z0) / 4.0;
    double v000 = cornerValue(vn, noise, x0,   y0,   z0);
    double v100 = cornerValue(vn, noise, x0+4, y0,   z0);
    double v010 = cornerValue(vn, noise, x0,   y0+8, z0);
    double v110 = cornerValue(vn, noise, x0+4, y0+8, z0);
    double v001 = cornerValue(vn, noise, x0,   y0,   z0+4);
    double v101 = cornerValue(vn, noise, x0+4, y0,   z0+4);
    double v011 = cornerValue(vn, noise, x0,   y0+8, z0+4);
    double v111 = cornerValue(vn, noise, x0+4, y0+8, z0+4);
    double xz00 = lerp(dy, v000, v010);
    double xz10 = lerp(dy, v100, v110);
    double xz01 = lerp(dy, v001, v011);
    double xz11 = lerp(dy, v101, v111);
    return lerp(dz, lerp(dx, xz00, xz10), lerp(dx, xz01, xz11));
}

static double clampedMap(double v, double inMin, double inMax, double outMin, double outMax)
{
    double t = (v - inMin) / (inMax - inMin);
    return t < 0 ? outMin : t > 1 ? outMax : lerp(t, outMin, outMax);
}

static int zoneOf(const OreVeinNoise *vn, double toggle, int y, double *strength)
{
    int newer = vn->mc >= MC_26_3;
    int copper = toggle > 0.0;
    int lo = copper ? 0 : -60;
    int hi = copper ? 50 : -8;
    // 26.3's material rules exclude the top height; earlier veins include it.
    if (y < lo || y > (newer ? hi - 1 : hi))
        return VEIN_NONE;
    int edge = hi - y < y - lo ? hi - y : y - lo;
    double t = toggle < 0 ? -toggle : toggle;
    double value, threshold;
    if (newer)
    {   // (toggle - 0.4) + (clamp(edge, 0, 20) * 0.01 - 0.2) >= 0
        value = t + ((edge > 20 ? 20 : edge) * 0.01 - 0.2);
        threshold = 0.4;
    }
    else
    {   // |toggle| + clampedMap(edge, 0, 20, -0.2, 0) < 0.4F: none
        value = t + clampedMap(edge, 0.0, 20.0, -0.2, 0.0);
        threshold = (double)0.4f;
    }
    if (value < threshold)
        return VEIN_NONE;
    if (strength)
        *strength = value - threshold;
    return copper ? VEIN_COPPER : VEIN_IRON;
}

double getVeinToggle(const OreVeinNoise *vn, int x, int y, int z)
{
    return interpolated(vn, VN_VEININESS, x, y, z);
}

int getOreVeinZone(const OreVeinNoise *vn, int x, int y, int z, double *strength)
{
    if (vn->mc < MC_1_18)
        return VEIN_NONE;
    return zoneOf(vn, interpolated(vn, VN_VEININESS, x, y, z), y, strength);
}

/* Mth.getSeed: a position's hash for positional randoms. */
static uint64_t positionSeed(int x, int y, int z)
{
    int64_t s = (int64_t)(int32_t)((uint32_t)x * 3129871u)
        ^ (int64_t)z * 116129781LL ^ (int64_t)y;
    uint64_t u = (uint64_t)s;
    u = u * u * 42317861ULL + u * 11ULL;
    return (uint64_t)((int64_t)u >> 16);
}

int getOreVeinBlock(const OreVeinNoise *vn, int x, int y, int z)
{
    double toggle, a, b, ridge, richness, t, gap;
    int vein;
    Xoroshiro r;

    if (vn->mc < MC_1_18)
        return VB_NONE;
    toggle = interpolated(vn, VN_VEININESS, x, y, z);
    vein = zoneOf(vn, toggle, y, NULL);
    if (vein == VEIN_NONE)
        return VB_NONE;
    a = interpolated(vn, VN_VEIN_A, x, y, z);
    b = interpolated(vn, VN_VEIN_B, x, y, z);
    if (a < 0) a = -a;
    if (b < 0) b = -b;
    // The ridge must be under 0.08 (a float constant before 26.3).
    ridge = (a > b ? a : b) - (vn->mc >= MC_26_3 ? 0.08 : (double)0.08f);
    r.lo = positionSeed(x, y, z) ^ vn->oreLo;
    r.hi = vn->oreHi;
    if (xNextFloat(&r) > 0.7f || ridge >= 0)
        return VB_NONE;
    t = toggle < 0 ? -toggle : toggle;
    if (vn->mc >= MC_26_3)
        richness = (t < 0.4 ? 0.4 : t > 0.6 ? 0.6 : t) - 0.3;
    else
        richness = clampedMap(t, 0.4f, 0.6f, 0.1f, 0.3f);
    gap = vn->mc >= MC_26_3 ? -0.3 : (double)-0.3f;
    if (xNextFloat(&r) < richness && sampleOreVeinNoise(vn, VN_GAP, x, y, z) > gap)
    {
        int raw = xNextFloat(&r) < 0.02f;
        if (vein == VEIN_COPPER)
            return raw ? VB_RAW_COPPER : VB_COPPER_ORE;
        return raw ? VB_RAW_IRON : VB_IRON_ORE;
    }
    return vein == VEIN_COPPER ? VB_GRANITE : VB_TUFF;
}
