// Entity geometry dumper (plan schematic-studio, T6/D4).
//
// Runs with a Minecraft client jar and its libraries on the class path, calls
// LayerDefinitions.createRoots() and writes every model layer's mesh as JSON.
// It touches only Mojang's classes and the JDK. For an obfuscated jar the
// caller passes a names file that maps Mojang's official names to the jar's
// names; the dumper itself never reads a mapping file. See README.md.
//
// Usage: java -cp <this folder>:<client jar>:<libraries> lumilio.entitydump.EntityDump OUTPUT [NAMES]
//
// The class has its own package: obfuscated jars are signed and keep their
// classes in the unnamed package, which an unsigned class may not join.
//
// NAMES lines (official names on the left, the jar's names on the right):
//   class <class> <runtime class>
//   field <class> <field> <runtime field>
//   method <class> <method> <runtime method>

package lumilio.entitydump;

import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

public final class EntityDump {
    private static final String GEOM = "net.minecraft.client.model.geom.";
    private static final String BUILDERS = GEOM + "builders.";
    private static final String LAYER_DEFINITIONS = GEOM + "LayerDefinitions";
    private static final String LAYER = BUILDERS + "LayerDefinition";
    private static final String MATERIAL = BUILDERS + "MaterialDefinition";
    private static final String MESH = BUILDERS + "MeshDefinition";
    private static final String PART = BUILDERS + "PartDefinition";
    private static final String CUBE = BUILDERS + "CubeDefinition";
    private static final String DEFORMATION = BUILDERS + "CubeDeformation";
    private static final String UV = BUILDERS + "UVPair";
    private static final String POSE = GEOM + "PartPose";
    /** The game's own vector before 1.19.3; later versions use JOML's. */
    private static final String MOJANG_VECTOR = "com.mojang.math.Vector3f";
    private static final String[] FACES = {"down", "up", "north", "south", "west", "east"};

    private final Map<String, String> names = new HashMap<>();
    /** The jar's class name → the official simple name (obfuscated jars). */
    private final Map<String, String> official = new HashMap<>();

    public static void main(String[] args) throws Exception {
        if (args.length < 1 || args.length > 2) {
            System.err.println("usage: EntityDump OUTPUT [NAMES]");
            System.exit(2);
        }
        EntityDump dump = new EntityDump();
        if (args.length == 2) {
            dump.readNames(Path.of(args[1]));
        }
        StringBuilder out = new StringBuilder();
        dump.write(out);
        Files.writeString(Path.of(args[0]), out, StandardCharsets.UTF_8);
    }

    private void readNames(Path file) throws IOException {
        for (String line : Files.readAllLines(file, StandardCharsets.UTF_8)) {
            String[] word = line.trim().split(" ");
            switch (word[0]) {
                case "class" -> {
                    names.put(word[1], word[2]);
                    String simple = word[1].substring(word[1].lastIndexOf('.') + 1);
                    official.put(word[2], simple.substring(simple.lastIndexOf('$') + 1));
                }
                case "field", "method" -> names.put(word[1] + "#" + word[2], word[3]);
                case "" -> {
                }
                default -> throw new IOException("unknown names line: " + line);
            }
        }
    }

    private Class<?> type(String official) throws ClassNotFoundException {
        return Class.forName(names.getOrDefault(official, official), false, getClass().getClassLoader());
    }

    /** The value of `official`'s field, or null when the class has no such field. */
    private Object get(Object owner, String official, String field, boolean optional) throws Exception {
        Field handle;
        try {
            handle = type(official).getDeclaredField(names.getOrDefault(official + "#" + field, field));
        } catch (NoSuchFieldException missing) {
            if (optional) {
                return null;
            }
            throw missing;
        }
        handle.setAccessible(true);
        return handle.get(owner);
    }

    private float number(Object owner, String official, String field) throws Exception {
        return (Float) get(owner, official, field, false);
    }

    /** Calls the static no-argument method if the version has it. */
    private void callIfPresent(String official, String method) throws Exception {
        Method handle;
        try {
            handle = type(official).getDeclaredMethod(names.getOrDefault(official + "#" + method, method));
        } catch (ClassNotFoundException | NoSuchMethodException absent) {
            return;
        }
        handle.setAccessible(true);
        try {
            handle.invoke(null);
        } catch (InvocationTargetException failed) {
            throw new IllegalStateException(official + "." + method + "() failed", failed.getCause());
        }
    }

    private void write(StringBuilder out) throws Exception {
        // Some layers read registries (boat and sign wood types, game events),
        // which refuse to be used before the game's bootstrap.
        callIfPresent("net.minecraft.SharedConstants", "tryDetectVersion");
        callIfPresent("net.minecraft.server.Bootstrap", "bootStrap");
        Method create = type(LAYER_DEFINITIONS)
                .getDeclaredMethod(names.getOrDefault(LAYER_DEFINITIONS + "#createRoots", "createRoots"));
        create.setAccessible(true);
        Map<?, ?> roots;
        try {
            roots = (Map<?, ?>) create.invoke(null);
        } catch (InvocationTargetException failed) {
            throw new IllegalStateException("createRoots() failed", failed.getCause());
        }
        // ModelLayerLocation.toString() is "namespace:path#layer" in every version.
        Map<String, Object> layers = new TreeMap<>();
        for (Map.Entry<?, ?> entry : roots.entrySet()) {
            layers.put(entry.getKey().toString(), entry.getValue());
        }
        out.append("{\"format\":2,\"layers\":{");
        boolean first = true;
        for (Map.Entry<String, Object> layer : layers.entrySet()) {
            if (!first) {
                out.append(',');
            }
            first = false;
            string(out, layer.getKey());
            out.append(":{\"texture_size\":[");
            Object material = get(layer.getValue(), LAYER, "material", false);
            out.append((int) (Integer) get(material, MATERIAL, "xTexSize", false)).append(',');
            out.append((int) (Integer) get(material, MATERIAL, "yTexSize", false)).append("],\"root\":");
            part(out, get(get(layer.getValue(), LAYER, "mesh", false), MESH, "root", false));
            out.append('}');
        }
        out.append('}');
        // The renderers are a bonus to the layers: a version whose renderers
        // cannot be built here still gives its geometry.
        StringBuilder renderers = new StringBuilder();
        try {
            renderers(renderers, roots);
            out.append(renderers);
        } catch (Exception | LinkageError failed) {
            out.append(",\"entities_error\":");
            string(out, failed.getClass().getSimpleName() + ": " + failed.getMessage());
        }
        out.append("}\n");
    }

    // ─── Renderers (plan schematic-studio T34, D32) ──────────────────────────
    //
    // Every entity type's renderer is built as the game builds it, from a
    // context that holds nothing but the model set, whose layer map notes each
    // layer the renderer asks for. A fresh render state then gives the default
    // texture (and with `isBaby` set, the baby's); renderers that keep their
    // texture in a field give it there. A renderer that needs more of the
    // context than the model set says so instead.

    /**
     * The model set's layer map: `roots`, noting in `asked` every layer looked
     * up. A proxy, so the dumper stays one class file.
     */
    private static Map<?, ?> recording(Map<?, ?> roots, List<String> asked) {
        return (Map<?, ?>) java.lang.reflect.Proxy.newProxyInstance(
                Map.class.getClassLoader(), new Class<?>[] {Map.class}, (proxy, call, arguments) -> {
                    if (call.getName().equals("get") && arguments != null && arguments.length == 1) {
                        asked.add(String.valueOf(arguments[0]));
                    }
                    try {
                        return call.invoke(roots, arguments);
                    } catch (InvocationTargetException failed) {
                        throw failed.getCause();
                    }
                });
    }

    private static final String RENDERERS = "net.minecraft.client.renderer.entity.EntityRenderers";
    private static final String PROVIDER = "net.minecraft.client.renderer.entity.EntityRendererProvider";
    private static final String CONTEXT = PROVIDER + "$Context";
    private static final String MODEL_SET = GEOM + "EntityModelSet";
    private static final String ENTITY_RENDERER = "net.minecraft.client.renderer.entity.EntityRenderer";
    private static final String BUILT_IN = "net.minecraft.core.registries.BuiltInRegistries";
    private static final String REGISTRY = "net.minecraft.core.Registry";
    private static final String LIVING_STATE = "net.minecraft.client.renderer.entity.state.LivingEntityRenderState";

    private Method method(String official, String name, Class<?>... parameters) throws Exception {
        Method handle = type(official).getMethod(names.getOrDefault(official + "#" + name, name), parameters);
        handle.setAccessible(true);
        return handle;
    }

    /**
     * The first method named `name` with `arity` parameters up `type`'s
     * classes that `fits`: obfuscated jars give many methods one short name.
     */
    private Method declared(Class<?> type, String official, String name, int arity,
            java.util.function.Predicate<Method> fits) {
        String runtime = names.getOrDefault(official + "#" + name, name);
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            for (Method m : c.getDeclaredMethods()) {
                if (m.getName().equals(runtime) && m.getParameterCount() == arity && !m.isBridge()
                        && fits.test(m)) {
                    m.setAccessible(true);
                    return m;
                }
            }
        }
        return null;
    }

    private static Object allocate(Class<?> type) throws Exception {
        Field theUnsafe = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        Object unsafe = theUnsafe.get(null);
        return unsafe.getClass().getMethod("allocateInstance", Class.class).invoke(unsafe, type);
    }

    private void renderers(StringBuilder out, Map<?, ?> roots) throws Exception {
        List<String> asked = new java.util.ArrayList<>();
        Object models = modelSet(recording(roots, asked));
        Class<?> contextType = type(CONTEXT);
        Object context = allocate(contextType);
        Field modelSet = contextType.getDeclaredField(names.getOrDefault(CONTEXT + "#modelSet", "modelSet"));
        modelSet.setAccessible(true);
        modelSet.set(context, models);
        Map<?, ?> providers = (Map<?, ?>) get(null, RENDERERS, "PROVIDERS", false);
        Method create = method(PROVIDER, "create", contextType);
        Object registry;
        try {
            registry = get(null, BUILT_IN, "ENTITY_TYPE", false);
        } catch (ClassNotFoundException before) {
            // Before 1.19.3 the built-in registries are Registry's fields.
            registry = get(null, REGISTRY, "ENTITY_TYPE", false);
        }
        Method key = method(REGISTRY, "getKey", Object.class);
        Class<?> entityRenderer = type(ENTITY_RENDERER);
        // Named ResourceLocation before 26.1.
        Class<?> identifier;
        try {
            identifier = type("net.minecraft.resources.Identifier");
        } catch (ClassNotFoundException older) {
            identifier = type("net.minecraft.resources.ResourceLocation");
        }

        Map<String, String> entries = new TreeMap<>();
        for (Object entityType : (Iterable<?>) registry) {
            String id = String.valueOf(key.invoke(registry, entityType));
            StringBuilder entry = new StringBuilder("{");
            Object provider = providers.get(entityType);
            if (provider == null) {
                entry.append("\"error\":\"no renderer\"}");
                entries.put(id, entry.toString());
                continue;
            }
            asked.clear();
            Object renderer;
            try {
                renderer = create.invoke(provider, context);
            } catch (InvocationTargetException failed) {
                Throwable cause = failed.getCause();
                entry.append("\"error\":");
                string(entry, cause.getClass().getSimpleName() + " building the renderer");
                entries.put(id, entry.append('}').toString());
                continue;
            }
            entry.append("\"renderer\":");
            names(entry, chain(renderer.getClass(), entityRenderer));
            Class<?> entity = entityClass(renderer.getClass(), entityRenderer);
            if (entity != null) {
                entry.append(",\"entity\":");
                names(entry, chain(entity, null));
            }
            entry.append(",\"layers\":");
            names(entry, asked);
            texture(entry, renderer, identifier);
            List<String> fields = textureFields(renderer, identifier);
            if (!fields.isEmpty()) {
                entry.append(",\"texture_fields\":");
                names(entry, fields);
            }
            entries.put(id, entry.append('}').toString());
        }
        out.append(",\"entities\":{");
        boolean first = true;
        for (Map.Entry<String, String> entry : entries.entrySet()) {
            if (!first) {
                out.append(',');
            }
            first = false;
            string(out, entry.getKey());
            out.append(':').append(entry.getValue());
        }
        out.append('}');
    }

    /** A model set over `layers`: built from them, or before 1.21 given them. */
    private Object modelSet(Map<?, ?> layers) throws Exception {
        Class<?> type = type(MODEL_SET);
        try {
            return type.getConstructor(Map.class).newInstance(layers);
        } catch (NoSuchMethodException older) {
            Object models = allocate(type);
            Field roots = type.getDeclaredField(names.getOrDefault(MODEL_SET + "#roots", "roots"));
            roots.setAccessible(true);
            roots.set(models, layers);
            return models;
        }
    }

    /**
     * `type` and its superclasses up to (and without) `stop`, by their
     * official simple names.
     */
    private List<String> chain(Class<?> type, Class<?> stop) {
        List<String> chain = new java.util.ArrayList<>();
        for (Class<?> c = type; c != null && c != Object.class && c != stop; c = c.getSuperclass()) {
            chain.add(official.getOrDefault(c.getName(), c.getSimpleName()));
        }
        return chain;
    }

    /**
     * The entity class a renderer draws: the first type argument of
     * EntityRenderer, followed through the renderer's generic superclasses.
     */
    private static Class<?> entityClass(Class<?> renderer, Class<?> entityRenderer) {
        Map<java.lang.reflect.TypeVariable<?>, java.lang.reflect.Type> bound = new HashMap<>();
        for (Class<?> c = renderer; c != null && c != Object.class; c = c.getSuperclass()) {
            java.lang.reflect.Type parent = c.getGenericSuperclass();
            if (!(parent instanceof java.lang.reflect.ParameterizedType generic)) {
                continue;
            }
            Class<?> raw = (Class<?>) generic.getRawType();
            java.lang.reflect.Type[] arguments = generic.getActualTypeArguments();
            java.lang.reflect.TypeVariable<?>[] variables = raw.getTypeParameters();
            for (int i = 0; i < variables.length && i < arguments.length; i++) {
                java.lang.reflect.Type argument = arguments[i];
                while (argument instanceof java.lang.reflect.TypeVariable<?> variable && bound.containsKey(variable)) {
                    argument = bound.get(variable);
                }
                bound.put(variables[i], argument);
            }
            if (raw == entityRenderer && arguments.length > 0) {
                java.lang.reflect.Type entity = bound.get(variables[0]);
                if (entity instanceof java.lang.reflect.ParameterizedType parameterized) {
                    entity = parameterized.getRawType();
                }
                if (entity instanceof java.lang.reflect.TypeVariable<?> variable) {
                    entity = variable.getBounds()[0];
                }
                return entity instanceof Class<?> found ? found : null;
            }
        }
        return null;
    }

    /**
     * The texture of a fresh render state, and the baby's where it differs;
     * nothing before 1.21.2, which has no render states.
     */
    private void texture(StringBuilder entry, Object renderer, Class<?> identifier) {
        Class<?> stateType;
        try {
            stateType = type("net.minecraft.client.renderer.entity.state.EntityRenderState");
        } catch (ClassNotFoundException older) {
            return;
        }
        Method create = declared(renderer.getClass(), ENTITY_RENDERER, "createRenderState", 0,
                m -> stateType.isAssignableFrom(m.getReturnType()));
        if (create == null) {
            return;
        }
        try {
            Object state = create.invoke(renderer);
            Method texture = declared(renderer.getClass(),
                    "net.minecraft.client.renderer.entity.LivingEntityRenderer", "getTextureLocation", 1,
                    m -> m.getReturnType() == identifier && m.getParameterTypes()[0].isInstance(state));
            if (texture == null) {
                return;
            }
            String adult = String.valueOf(texture.invoke(renderer, state));
            entry.append(",\"texture\":");
            string(entry, adult);
            Field baby = null;
            String isBaby = names.getOrDefault(LIVING_STATE + "#isBaby", "isBaby");
            for (Class<?> c = state.getClass(); c != null && baby == null; c = c.getSuperclass()) {
                for (Field f : c.getDeclaredFields()) {
                    if (f.getName().equals(isBaby) && f.getType() == boolean.class) {
                        baby = f;
                    }
                }
            }
            if (baby != null) {
                baby.setAccessible(true);
                baby.setBoolean(state, true);
                String young = String.valueOf(texture.invoke(renderer, state));
                if (!young.equals(adult)) {
                    entry.append(",\"baby_texture\":");
                    string(entry, young);
                }
            }
        } catch (Exception failed) {
            // A texture that needs more than a fresh state: none is said.
        }
    }

    /** Textures the renderer keeps in fields of its own or its class's. */
    private static List<String> textureFields(Object renderer, Class<?> identifier) {
        java.util.Set<String> found = new java.util.TreeSet<>();
        for (Class<?> c = renderer.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (f.getType() != identifier) {
                    continue;
                }
                try {
                    f.setAccessible(true);
                    Object value = f.get(java.lang.reflect.Modifier.isStatic(f.getModifiers()) ? null : renderer);
                    if (value != null && value.toString().contains("textures/")) {
                        found.add(value.toString());
                    }
                } catch (Exception | LinkageError unreadable) {
                    // Not a texture we can read; leave it out.
                }
            }
        }
        return new java.util.ArrayList<>(found);
    }

    private static void names(StringBuilder out, List<String> names) {
        out.append('[');
        for (int i = 0; i < names.size(); i++) {
            if (i > 0) {
                out.append(',');
            }
            string(out, names.get(i));
        }
        out.append(']');
    }

    private void part(StringBuilder out, Object part) throws Exception {
        Object pose = get(part, PART, "partPose", false);
        out.append("{\"pivot\":");
        floats(out, number(pose, POSE, "x"), number(pose, POSE, "y"), number(pose, POSE, "z"));
        out.append(",\"rotation\":");
        floats(out, number(pose, POSE, "xRot"), number(pose, POSE, "yRot"), number(pose, POSE, "zRot"));
        Object xScale = get(pose, POSE, "xScale", true);
        if (xScale != null) {
            float x = (Float) xScale;
            float y = number(pose, POSE, "yScale");
            float z = number(pose, POSE, "zScale");
            if (x != 1f || y != 1f || z != 1f) {
                out.append(",\"scale\":");
                floats(out, x, y, z);
            }
        }
        List<?> cubes = (List<?>) get(part, PART, "cubes", false);
        if (!cubes.isEmpty()) {
            out.append(",\"cubes\":[");
            for (int i = 0; i < cubes.size(); i++) {
                if (i > 0) {
                    out.append(',');
                }
                cube(out, cubes.get(i));
            }
            out.append(']');
        }
        Map<String, Object> children = new TreeMap<>();
        for (Map.Entry<?, ?> child : ((Map<?, ?>) get(part, PART, "children", false)).entrySet()) {
            children.put((String) child.getKey(), child.getValue());
        }
        if (!children.isEmpty()) {
            out.append(",\"children\":{");
            boolean first = true;
            for (Map.Entry<String, Object> child : children.entrySet()) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                string(out, child.getKey());
                out.append(':');
                part(out, child.getValue());
            }
            out.append('}');
        }
        out.append('}');
    }

    private void cube(StringBuilder out, Object cube) throws Exception {
        out.append("{\"origin\":");
        vector(out, get(cube, CUBE, "origin", false));
        out.append(",\"size\":");
        vector(out, get(cube, CUBE, "dimensions", false));
        Object uv = get(cube, CUBE, "texCoord", false);
        out.append(",\"uv\":[");
        number(out, number(uv, UV, "u"));
        out.append(',');
        number(out, number(uv, UV, "v"));
        out.append(']');
        Object scale = get(cube, CUBE, "texScale", true);
        if (scale != null) {
            float u = number(scale, UV, "u");
            float v = number(scale, UV, "v");
            if (u != 1f || v != 1f) {
                out.append(",\"uv_scale\":[");
                number(out, u);
                out.append(',');
                number(out, v);
                out.append(']');
            }
        }
        Object grow = get(cube, CUBE, "grow", false);
        float gx = number(grow, DEFORMATION, "growX");
        float gy = number(grow, DEFORMATION, "growY");
        float gz = number(grow, DEFORMATION, "growZ");
        if (gx != 0f || gy != 0f || gz != 0f) {
            out.append(",\"inflate\":");
            floats(out, gx, gy, gz);
        }
        if ((Boolean) get(cube, CUBE, "mirror", false)) {
            out.append(",\"mirror\":true");
        }
        Object faces = get(cube, CUBE, "visibleFaces", true);
        if (faces != null && ((java.util.Set<?>) faces).size() < FACES.length) {
            // Direction's constants keep their order (down, up, north, south,
            // west, east) in every version, and ordinals survive obfuscation.
            boolean[] visible = new boolean[FACES.length];
            for (Object face : (java.util.Set<?>) faces) {
                visible[((Enum<?>) face).ordinal()] = true;
            }
            out.append(",\"faces\":[");
            boolean first = true;
            for (int i = 0; i < FACES.length; i++) {
                if (visible[i]) {
                    if (!first) {
                        out.append(',');
                    }
                    first = false;
                    string(out, FACES[i]);
                }
            }
            out.append(']');
        }
        out.append('}');
    }

    private void vector(StringBuilder out, Object vector) throws Exception {
        Class<?> mojang;
        try {
            mojang = type(MOJANG_VECTOR);
        } catch (ClassNotFoundException absent) {
            mojang = null;
        }
        if (vector.getClass() == mojang) {
            floats(out, number(vector, MOJANG_VECTOR, "x"), number(vector, MOJANG_VECTOR, "y"),
                    number(vector, MOJANG_VECTOR, "z"));
            return;
        }
        // JOML's Vector3fc (1.19.3 and later), never obfuscated.
        Class<?> joml = Class.forName("org.joml.Vector3fc", false, getClass().getClassLoader());
        floats(out, (Float) joml.getMethod("x").invoke(vector), (Float) joml.getMethod("y").invoke(vector),
                (Float) joml.getMethod("z").invoke(vector));
    }

    private static void floats(StringBuilder out, float x, float y, float z) {
        out.append('[');
        number(out, x);
        out.append(',');
        number(out, y);
        out.append(',');
        number(out, z);
        out.append(']');
    }

    /** The shortest decimal that reads back as the same float. */
    private static void number(StringBuilder out, float value) {
        if (Float.isNaN(value) || Float.isInfinite(value)) {
            throw new IllegalStateException("not a finite number: " + value);
        }
        if (value == Math.rint(value) && Math.abs(value) < 1e7f) {
            out.append((long) value);
            if (value == 0f && 1f / value < 0f) {
                out.setLength(out.length() - 1);
                out.append("-0");
            }
            return;
        }
        out.append(Float.toString(value));
    }

    private static void string(StringBuilder out, String text) {
        out.append('"');
        for (char c : text.toCharArray()) {
            if (c == '"' || c == '\\') {
                out.append('\\').append(c);
            } else if (c < 0x20) {
                out.append(String.format("\\u%04x", (int) c));
            } else {
                out.append(c);
            }
        }
        out.append('"');
    }
}
