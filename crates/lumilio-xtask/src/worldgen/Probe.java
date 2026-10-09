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
        JsonArray samples = new JsonArray();
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
            Object settingKey = field(type("world.level.levelgen.NoiseGeneratorSettings"), dimension.toUpperCase());
            Object setting = call(call(settings, "getOrThrow", settingKey), "value");
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
        output.add("samples", samples);
        JsonArray placements = new JsonArray();
        Object sets = call(registries, "lookupOrThrow", field(type("core.registries.Registries"), "STRUCTURE_SET"));
        String[] setNames = {"DESERT_PYRAMIDS", "JUNGLE_TEMPLES", "SWAMP_HUTS", "IGLOOS", "VILLAGES", "OCEAN_RUINS", "SHIPWRECKS", "OCEAN_MONUMENTS", "WOODLAND_MANSIONS", "PILLAGER_OUTPOSTS", "RUINED_PORTALS", "ANCIENT_CITIES", "TRAIL_RUINS", "TRIAL_CHAMBERS", "NETHER_COMPLEXES", "NETHER_COMPLEXES", "END_CITIES", "ABANDONED_CAMP"};
        for (int kind = 0; kind < setNames.length; kind++) {
            Object key;
            try { key = field(type("world.level.levelgen.structure.BuiltinStructureSets"), setNames[kind]); }
            catch (NoSuchFieldException absent) { continue; }
            Object placement = call(call(call(sets, "getOrThrow", key), "value"), "placement");
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
