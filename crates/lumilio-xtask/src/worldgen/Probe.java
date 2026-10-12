/* LumilioCL development oracle. Runs the original Minecraft server classes;
 * no Minecraft implementation is copied into this file or the C fork.
 * The named jar, libraries and generated numeric data stay under target/.
 */
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

public class Probe {
    static Class<?> type(String name) throws Exception {
        return Class.forName("net.minecraft." + name);
    }
    static Object field(Object target, String name) throws Exception {
        Class<?> cls = target instanceof Class<?> c ? c : target.getClass();
        for (Class<?> c = cls; c != null; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(target instanceof Class<?> ? null : target);
            } catch (NoSuchFieldException ignored) {}
        }
        throw new NoSuchFieldException(cls.getName() + "." + name);
    }
    static Object call(Object target, String name, Object... args) throws Exception {
        Class<?> cls = target instanceof Class<?> c ? c : target.getClass();
        List<Method> methods = new ArrayList<>(Arrays.asList(cls.getMethods()));
        for (Class<?> c = cls; c != null; c = c.getSuperclass()) methods.addAll(Arrays.asList(c.getDeclaredMethods()));
            for (Method m : methods) {
                if (!m.getName().equals(name) || m.getParameterCount() != args.length) continue;
                Class<?>[] types = m.getParameterTypes();
                boolean matches = true;
                for (int i = 0; i < args.length; i++) {
                    if (!types[i].isPrimitive() && !types[i].isInstance(args[i])) matches = false;
                    if (types[i] == long.class && !(args[i] instanceof Long)) matches = false;
                    if (types[i] == int.class && !(args[i] instanceof Integer)) matches = false;
                }
                if (!matches) continue;
                m.setAccessible(true);
                return m.invoke(target instanceof Class<?> ? null : target, args);
            }
        throw new NoSuchMethodException(cls.getName() + "." + name);
    }
    static String biome(Object holder) throws Exception {
        Object key = ((Optional<?>)call(holder, "unwrapKey")).orElseThrow();
        try { return call(key, "location").toString(); }
        catch (NoSuchMethodException e) { return call(key, "identifier").toString(); }
    }
    static JsonObject tree(Object node) throws Exception {
        JsonObject out = new JsonObject();
        JsonArray bounds = new JsonArray();
        for (Object p : (Object[])field(node, "parameterSpace")) {
            JsonArray interval = new JsonArray();
            interval.add((Long)call(p, "min"));
            interval.add((Long)call(p, "max"));
            bounds.add(interval);
        }
        out.add("bounds", bounds);
        try {
            JsonArray children = new JsonArray();
            for (Object child : (Object[])field(node, "children")) children.add(tree(child));
            out.add("children", children);
        } catch (NoSuchFieldException leaf) {
            out.addProperty("biome", biome(field(node, "value")));
        }
        return out;
    }
    static Object construct(Class<?> cls, Object... args) throws Exception {
        for (Constructor<?> c : cls.getDeclaredConstructors()) {
            if (c.getParameterCount() != args.length) continue;
            Class<?>[] types = c.getParameterTypes();
            boolean matches = true;
            for (int i = 0; i < args.length; i++) {
                if (types[i] == long.class) matches &= args[i] instanceof Long;
                else if (types[i] == int.class) matches &= args[i] instanceof Integer;
                else if (types[i].isPrimitive()) matches = false;
                else matches &= types[i].isInstance(args[i]);
            }
            if (!matches) continue;
            c.setAccessible(true);
            return c.newInstance(args);
        }
        throw new NoSuchMethodException(cls.getName() + " constructor");
    }
    /* A world that only answers its height bounds: placement modifiers that
     * draw random numbers read nothing else. */
    static Object level(int minY, int height) throws Exception {
        Class<?> cls = type("world.level.WorldGenLevel");
        return Proxy.newProxyInstance(cls.getClassLoader(), new Class<?>[]{cls}, (proxy, method, args) -> {
            switch (method.getName()) {
                case "getMinY": case "getMinBuildHeight": return minY;
                case "getHeight": return height;
                case "getMaxY": return minY + height - 1;
                case "getMaxBuildHeight": return minY + height;
                case "getMinSectionY": case "getMinSection": return minY >> 4;
                default:
                    if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, args);
                    throw new UnsupportedOperationException(method.getName());
            }
        });
    }
    /* Filters that read the world and draw no random numbers pass through:
     * the sequence is the game's when every candidate fails its biome check. */
    static final List<String> PASSED = List.of("BiomeFilter", "HeightmapPlacement", "EnvironmentScanPlacement",
        "SurfaceRelativeThresholdFilter", "BlockPredicateFilter");
    static final List<String> TRACKED = List.of("minecraft:fossil_upper", "minecraft:fossil_lower",
        "minecraft:lake_lava_surface", "minecraft:lake_lava_underground", "minecraft:monster_room",
        "minecraft:monster_room_deep", "minecraft:rooted_sulfur_spring", "minecraft:amethyst_geode",
        "minecraft:desert_well", "minecraft:end_island_decorated", "minecraft:end_gateway_return");
    static List<?> modify(Object modifier, Object context, Object random, Object pos) throws Exception {
        if (PASSED.contains(modifier.getClass().getSimpleName())) return List.of(pos);
        List<Object> out = new ArrayList<>();
        try { call(modifier, "modify", context, random, pos, (java.util.function.Consumer<Object>)out::add); }
        catch (NoSuchMethodException older) {
            out.addAll(((java.util.stream.Stream<?>)call(modifier, "getPositions", context, random, pos)).toList());
        }
        return out;
    }
    static void candidates(JsonArray out, String dimension, Object source, Object settingHolder,
            List<?> steps, Map<Object, String> names) throws Exception {
        Object generator = construct(type("world.level.levelgen.NoiseBasedChunkGenerator"), source, settingHolder);
        int minY = dimension.equals("overworld") ? -64 : 0;
        int height = dimension.equals("overworld") ? 384 : 256;
        Object context = construct(type("world.level.levelgen.placement.PlacementContext"), level(minY, height), generator, Optional.empty());
        for (int step = 0; step < steps.size(); step++) {
            List<?> list = (List<?>)call(steps.get(step), "features");
            for (int index = 0; index < list.size(); index++) {
                String name = names.get(list.get(index));
                if (!TRACKED.contains(name)) continue;
                List<?> modifiers = (List<?>)call(list.get(index), "placement");
                // Counted features yield many candidates per chunk: fewer chunks.
                boolean counted = modifiers.get(0).getClass().getSimpleName().equals("CountPlacement");
                int edge = counted ? 4 : 16;
                for (long seed : new long[]{262L, 9876543210L, -123456789012345L}) {
                    for (int cz = -edge / 2; cz < edge / 2; cz++) {
                        for (int cx = -edge / 2; cx < edge / 2; cx++) {
                            Object random = construct(type("world.level.levelgen.WorldgenRandom"),
                                construct(type("world.level.levelgen.XoroshiroRandomSource"), 0L));
                            long decoration = (Long)call(random, "setDecorationSeed", seed, cx << 4, cz << 4);
                            call(random, "setFeatureSeed", decoration, index, step);
                            JsonArray found = new JsonArray();
                            // Depth first, like the game's feature placer.
                            Deque<Object[]> stack = new ArrayDeque<>();
                            stack.push(new Object[]{construct(type("core.BlockPos"), cx << 4, minY, cz << 4), 0});
                            while (!stack.isEmpty()) {
                                Object[] top = stack.pop();
                                int at = (Integer)top[1];
                                List<?> made = modify(modifiers.get(at), context, random, top[0]);
                                if (at + 1 < modifiers.size()) {
                                    for (int i = made.size() - 1; i >= 0; i--) stack.push(new Object[]{made.get(i), at + 1});
                                } else {
                                    for (Object pos : made) {
                                        JsonArray xyz = new JsonArray();
                                        for (String axis : List.of("getX", "getY", "getZ")) xyz.add((Integer)call(pos, axis));
                                        found.add(xyz);
                                    }
                                }
                            }
                            if (found.isEmpty()) continue;
                            JsonObject row = new JsonObject();
                            row.addProperty("feature", name);
                            row.addProperty("seed", seed);
                            row.addProperty("cx", cx);
                            row.addProperty("cz", cz);
                            row.add("positions", found);
                            out.add(row);
                        }
                    }
                }
            }
        }
    }
    /* The ore vein noises of a world, sampled as their density functions read
     * them (input scaled by the function's xz/y scale). */
    static void oreNoises(JsonArray out, Object setting, Object noises) throws Exception {
        String[] names = {"ORE_VEININESS", "ORE_VEIN_A", "ORE_VEIN_B", "ORE_GAP"};
        double[] scales = {1.5, 4.0, 4.0, 1.0};
        for (long seed : new long[]{262L, 9876543210L, -123456789012345L}) {
            Object state;
            try { state = call(type("world.level.levelgen.RandomState"), "create", setting, noises, seed); }
            catch (NoSuchMethodException e) { state = call(type("world.level.levelgen.RandomState"), "create", noises, seed, setting); }
            for (int n = 0; n < names.length; n++) {
                Object noise = call(state, "getOrCreateNoise", field(type("world.level.levelgen.Noises"), names[n]));
                for (int i = 0; i < 64; i++) {
                    int x = (i * 1543 % 2003) - 1000, y = (i * 37 % 121) - 64, z = (i * 7919 % 1999) - 1000;
                    double s = scales[n];
                    Object value;
                    try { value = call(noise, "getValue", x * s, y * s, z * s); }
                    catch (NoSuchMethodException e) { value = call(noise, "get", x * s, y * s, z * s); }
                    JsonObject row = new JsonObject();
                    row.addProperty("seed", seed);
                    row.addProperty("noise", n);
                    row.addProperty("x", x);
                    row.addProperty("y", y);
                    row.addProperty("z", z);
                    row.addProperty("value", ((Number)value).doubleValue());
                    out.add(row);
                }
            }
        }
    }
    /* Mth.sin and Mth.cos of float inputs, as carvers call them. */
    static void trig(JsonArray out) throws Exception {
        for (int i = 0; i < 400; i++) {
            float value = (i - 200) * 0.0371f + (i % 7) * 1.0e-3f;
            JsonObject row = new JsonObject();
            row.addProperty("value", value);
            row.addProperty("sin", ((Number)call(type("util.Mth"), "sin", value)).floatValue());
            row.addProperty("cos", ((Number)call(type("util.Mth"), "cos", value)).floatValue());
            out.add(row);
        }
    }
    static long carvedKey(int tx, int tz, int index) {
        long k = ((long)tx * 0x9E3779B1L) ^ ((long)tz * 0x85EBCA77L) ^ ((long)index * 0xC2B2AE3DL);
        k *= 0xFF51AFD7ED558CCDL;
        return k ^ (k >>> 33);
    }
    /* Every canyon starting near the origin, carved by the original carver
     * into a carving mask for each chunk it can reach (26.x carver API). */
    static void ravines(JsonArray out, Object registries, Object generator) throws Exception {
        Object carvers = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "CARVER"));
        Object canyon = null;
        for (Object holder : ((java.util.stream.Stream<?>)call(carvers, "listElements")).toList()) {
            if (biome(holder).equals("minecraft:canyon")) canyon = call(holder, "value");
        }
        Object context = construct(type("world.level.levelgen.WorldGenerationContext"), generator, level(-64, 384));
        for (long seed : new long[]{262L, 9876543210L, -123456789012345L}) {
            for (int sz = -12; sz < 12; sz++) {
                for (int sx = -12; sx < 12; sx++) {
                    Object random = construct(type("world.level.levelgen.WorldgenRandom"),
                        construct(type("world.level.levelgen.LegacyRandomSource"), 0L));
                    call(random, "setLargeFeatureSeed", seed + 2, sx, sz);
                    if (!(Boolean)call(canyon, "isStartChunk", random)) continue;
                    long count = 0, checksum = 0;
                    for (int tz = sz - 8; tz <= sz + 8; tz++) {
                        for (int tx = sx - 8; tx <= sx + 8; tx++) {
                            call(random, "setLargeFeatureSeed", seed + 2, sx, sz);
                            call(canyon, "isStartChunk", random);
                            Object mask = construct(type("world.level.chunk.CarvingMask"), -63, 312);
                            call(canyon, "carve", context, random, construct(type("world.level.ChunkPos"), tx, tz),
                                construct(type("world.level.ChunkPos"), sx, sz), mask);
                            BitSet bits = (BitSet)field(mask, "mask");
                            for (int i = bits.nextSetBit(0); i >= 0; i = bits.nextSetBit(i + 1)) {
                                count++;
                                checksum += carvedKey(tx, tz, i);
                            }
                        }
                    }
                    JsonObject row = new JsonObject();
                    row.addProperty("seed", seed);
                    row.addProperty("sx", sx);
                    row.addProperty("sz", sz);
                    row.addProperty("count", count);
                    row.addProperty("checksum", Long.toUnsignedString(checksum));
                    out.add(row);
                }
            }
        }
    }
    public static void main(String[] args) throws Exception {
        call(type("SharedConstants"), "tryDetectVersion");
        call(type("server.Bootstrap"), "bootStrap");
        Object registries;
        try { registries = call(type("data.registries.VanillaRegistries"), "createLookup"); }
        catch (NoSuchMethodException e) {
            registries = call(type("data.registries.VanillaRegistries"), "createWorldLookup");
        }
        Object biomes = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "BIOME"));
        Object presets = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST"));
        Object noises = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "NOISE"));
        Object settings = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "NOISE_SETTINGS"));
        JsonObject output = new JsonObject();
        JsonObject trees = new JsonObject();
        JsonObject features = new JsonObject();
        JsonArray decorations = new JsonArray();
        JsonArray veins = new JsonArray();
        JsonArray ravineRows = new JsonArray();
        JsonArray trigRows = new JsonArray();
        trig(trigRows);
        JsonArray samples = new JsonArray();
        Map<Object, String> placedNames = new IdentityHashMap<>();
        Object placed = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "PLACED_FEATURE"));
        for (Object holder : ((java.util.stream.Stream<?>)call(placed, "listElements")).toList()) {
            placedNames.put(call(holder, "value"), biome(holder));
        }
        for (String dimension : List.of("overworld", "nether", "end")) {
            Object source;
            if (dimension.equals("end")) source = call(type("world.level.biome.TheEndBiomeSource"), "create", biomes);
            else {
                Object preset = field(type("world.level.biome.MultiNoiseBiomeSourceParameterLists"), dimension.toUpperCase());
                Object holder = call(presets, "getOrThrow", preset);
                source = call(type("world.level.biome.MultiNoiseBiomeSource"), "createFromPreset", holder);
                Object parameters = call(call(holder, "value"), "parameters");
                trees.add(dimension, tree(field(field(parameters, "index"), "root")));
            }
            // The decoration order a chunk generator over this source uses:
            // a feature's seed index is its position within its step.
            List<Object> possible = new ArrayList<>((Collection<?>)call(source, "possibleBiomes"));
            java.util.function.Function<Object, Object> generation = holder -> {
                try { return call(call(call(holder, "value"), "getGenerationSettings"), "features"); }
                catch (Exception e) { throw new RuntimeException(e); }
            };
            JsonArray steps = new JsonArray();
            List<?> stepData = (List<?>)call(type("world.level.biome.FeatureSorter"), "buildFeaturesPerStep", possible, generation, true);
            for (Object step : stepData) {
                JsonArray names = new JsonArray();
                for (Object feature : (List<?>)call(step, "features")) names.add(placedNames.get(feature));
                steps.add(names);
            }
            features.add(dimension, steps);
            Object settingKey = field(type("world.level.levelgen.NoiseGeneratorSettings"), dimension.toUpperCase());
            Object settingHolder = call(settings, "getOrThrow", settingKey);
            Object setting = call(settingHolder, "value");
            candidates(decorations, dimension, source, settingHolder, stepData, placedNames);
            if (dimension.equals("overworld")) {
                oreNoises(veins, setting, noises);
                Object generator = construct(type("world.level.levelgen.NoiseBasedChunkGenerator"), source, settingHolder);
                // Before 26.1 a carver needs a whole chunk to carve into.
                try { ravines(ravineRows, registries, generator); }
                catch (NoSuchFieldException | NoSuchMethodException older) { ravineRows = new JsonArray(); }
            }
            for (long seed : new long[]{262L, 9876543210L, -123456789012345L}) {
                Object random;
                Object sampler;
                try {
                    random = call(type("world.level.levelgen.RandomState"), "create", setting, noises, seed);
                    sampler = call(random, "sampler");
                } catch (NoSuchMethodException e) {
                    random = call(type("world.level.levelgen.RandomState"), "create", noises, seed, setting);
                    sampler = call(random, "createClimateSampler", field(type("world.level.levelgen.densityfunction.SamplerContext"), "EMPTY_UNCACHED"));
                }
                Object resolver = null;
                try { resolver = call(source, "createResolver", sampler); }
                catch (NoSuchMethodException ignored) {}
                for (int y : new int[]{-48, 0, 64, 192}) {
                    for (int i = 0; i < 96; i++) {
                        int x = (i * 1543 % 50003) - 25000;
                        int z = (i * 7919 % 49999) - 25000;
                        Object holder = resolver == null
                            ? call(source, "getNoiseBiome", x >> 2, y >> 2, z >> 2, sampler)
                            : call(resolver, "getNoiseBiome", x >> 2, y >> 2, z >> 2);
                        JsonObject row = new JsonObject();
                        row.addProperty("seed", seed);
                        row.addProperty("dimension", dimension);
                        row.addProperty("x", x >> 2);
                        row.addProperty("y", y >> 2);
                        row.addProperty("z", z >> 2);
                        row.addProperty("biome", biome(holder));
                        if (!dimension.equals("end")) {
                            Object climate = call(sampler, "sample", x >> 2, y >> 2, z >> 2);
                            JsonArray values = new JsonArray();
                            for (String axis : List.of("temperature", "humidity", "continentalness", "erosion", "depth", "weirdness")) {
                                values.add((Long)call(climate, axis));
                            }
                            row.add("climate", values);
                        }
                        samples.add(row);
                    }
                }
            }
        }
        output.add("trees", trees);
        output.add("features", features);
        output.add("decorations", decorations);
        output.add("veins", veins);
        output.add("ravines", ravineRows);
        output.add("trig", trigRows);
        output.add("samples", samples);
        JsonArray placements = new JsonArray();
        Object sets = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "STRUCTURE_SET"));
        String[] setNames = {"DESERT_PYRAMIDS", "JUNGLE_TEMPLES", "SWAMP_HUTS", "IGLOOS", "VILLAGES", "OCEAN_RUINS", "SHIPWRECKS", "OCEAN_MONUMENTS", "WOODLAND_MANSIONS", "PILLAGER_OUTPOSTS", "RUINED_PORTALS", "ANCIENT_CITIES", "TRAIL_RUINS", "TRIAL_CHAMBERS", "NETHER_COMPLEXES", "NETHER_COMPLEXES", "END_CITIES", "ABANDONED_CAMP",
            // Rust kinds 18–22 are one-chunk decorators without a set here.
            "", "", "", "", "", "NETHER_FOSSILS"};
        for (int kind = 0; kind < setNames.length; kind++) {
            Object key;
            try { key = field(type("world.level.levelgen.structure.BuiltinStructureSets"), setNames[kind]); }
            catch (NoSuchFieldException absent) { continue; }
            // A set behind an experimental feature flag (1.20.5–1.20.6 trial
            // chambers) has a key but no vanilla registry entry.
            Optional<?> set = (Optional<?>)call(sets, "get", key);
            if (set.isEmpty()) continue;
            Object placement = call(call(set.get(), "value"), "placement");
            int spacing = (Integer)call(placement, "spacing");
            for (long seed : new long[]{262L, 9876543210L, -123456789012345L}) {
                for (int rx = -2; rx <= 2; rx++) {
                    for (int rz = -2; rz <= 2; rz++) {
                        Object pos = call(placement, "getPotentialStructureChunk", seed, rx * spacing, rz * spacing);
                        JsonObject row = new JsonObject();
                        row.addProperty("seed", seed);
                        row.addProperty("kind", kind);
                        row.addProperty("rx", rx);
                        row.addProperty("rz", rz);
                        row.addProperty("x", (Integer)field(pos, "x") * 16);
                        row.addProperty("z", (Integer)field(pos, "z") * 16);
                        placements.add(row);
                    }
                }
            }
        }
        output.add("placements", placements);
        Files.writeString(Path.of(args[0]), new Gson().toJson(output));
        System.exit(0);
    }
}
