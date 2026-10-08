/* Uses cubiomes generator.h (MIT, Copyright (c) 2020 Cubitect).
 * The bridge is LumilioCL code; upstream structs stay on the C side. */
#include "generator.h"

int lumilio_cubiomes_probe(void)
{
    Generator g;
    setupGenerator(&g, MC_1_21_WD, 0);
    applySeed(&g, DIM_OVERWORLD, 262);
    return getBiomeAt(&g, 1, 0, 64, 0);
}
