package me.marin.lockout.server;

import com.google.gson.*;
import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.client.LockoutBoard;
import me.marin.lockout.lockout.Goal;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.storage.LevelResource;
import oshi.util.tuples.Pair;

import java.io.IOException;
import java.lang.reflect.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.*;

/**
 * Saves the running match to the world folder, so it survives a server restart.
 * <p>
 * Besides the board, teams and timer, every instance field of {@link Lockout} is saved through reflection.
 * That covers the progress trackers (unique foods eaten, mobs killed, ...) without listing them here,
 * so new trackers are saved too. A field of an unsupported type makes saving fail loudly instead of being skipped.
 */
public class MatchPersistence {

    private static final String FILE_NAME = "lockout-match.json";
    private static final int FORMAT_VERSION = 1;
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    /** Saved explicitly (board, teams, timer) or derived; not part of the tracker fields. */
    private static final Set<String> EXPLICIT_FIELDS = Set.of("board", "teams", "hasStarted", "isRunning", "ticks");

    private static boolean hasSave = false;

    private static Path path(MinecraftServer server) {
        return server.getWorldPath(LevelResource.ROOT).resolve(FILE_NAME);
    }

    public static void save(MinecraftServer server, Lockout lockout) {
        try {
            JsonObject json = toJson(lockout);
            Path path = path(server);
            Path tmp = path.resolveSibling(FILE_NAME + ".tmp");
            Files.writeString(tmp, GSON.toJson(json));
            Files.move(tmp, path, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            hasSave = true;
        } catch (Exception e) {
            Lockout.log("Could not save the Lockout match.");
            Lockout.error(e);
        }
    }

    public static void delete(MinecraftServer server) {
        if (!hasSave) return;
        try {
            Files.deleteIfExists(path(server));
        } catch (IOException e) {
            Lockout.error(e);
        }
        hasSave = false;
    }

    /**
     * @return the saved match, or null if there's none (or it couldn't be read)
     */
    public static Lockout load(MinecraftServer server) {
        Path path = path(server);
        if (!Files.exists(path)) return null;
        hasSave = true;
        try {
            JsonObject json = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
            return fromJson(json, server);
        } catch (Exception e) {
            Lockout.log("Could not restore the saved Lockout match.");
            Lockout.error(e);
            return null;
        }
    }

    public static JsonObject toJson(Lockout lockout) throws IllegalAccessException {
        JsonObject json = new JsonObject();
        json.addProperty("version", FORMAT_VERSION);
        json.addProperty("ticks", lockout.getTicks());
        json.addProperty("started", lockout.hasStarted());

        List<? extends LockoutTeam> teams = lockout.getTeams();
        JsonArray teamsJson = new JsonArray();
        for (LockoutTeam team : teams) {
            LockoutTeamServer serverTeam = (LockoutTeamServer) team;
            JsonObject teamJson = new JsonObject();
            teamJson.addProperty("color", LockoutTeam.colorName(team.getColor()));
            teamJson.addProperty("points", team.getPoints());
            JsonArray players = new JsonArray();
            for (UUID playerId : serverTeam.getPlayers()) {
                JsonObject player = new JsonObject();
                player.addProperty("uuid", playerId.toString());
                player.addProperty("name", serverTeam.getPlayerName(playerId));
                players.add(player);
            }
            teamJson.add("players", players);
            teamsJson.add(teamJson);
        }
        json.add("teams", teamsJson);

        JsonArray goalsJson = new JsonArray();
        for (Goal goal : lockout.getBoard().getGoals()) {
            JsonObject goalJson = new JsonObject();
            goalJson.addProperty("id", goal.getId());
            goalJson.addProperty("data", goal.getData());
            goalJson.addProperty("completedBy", goal.isCompleted() ? teams.indexOf(goal.getCompletedTeam()) : -1);
            goalsJson.add(goalJson);
        }
        json.add("goals", goalsJson);

        JsonObject trackers = new JsonObject();
        for (Field field : trackerFields()) {
            trackers.add(field.getName(), encode(field.get(lockout), field.getGenericType(), teams, field.getName()));
        }
        json.add("trackers", trackers);
        return json;
    }

    public static Lockout fromJson(JsonObject json, MinecraftServer server) throws IllegalAccessException {
        int version = json.get("version").getAsInt();
        if (version != FORMAT_VERSION) {
            throw new IllegalStateException("Unknown save format version " + version);
        }

        List<LockoutTeamServer> teams = new ArrayList<>();
        List<Integer> points = new ArrayList<>();
        for (JsonElement element : json.getAsJsonArray("teams")) {
            JsonObject teamJson = element.getAsJsonObject();
            List<String> names = new ArrayList<>();
            List<UUID> ids = new ArrayList<>();
            for (JsonElement playerElement : teamJson.getAsJsonArray("players")) {
                JsonObject player = playerElement.getAsJsonObject();
                ids.add(UUID.fromString(player.get("uuid").getAsString()));
                names.add(player.get("name").getAsString());
            }
            teams.add(new LockoutTeamServer(names, ids, LockoutTeam.colorByName(teamJson.get("color").getAsString()), server));
            points.add(teamJson.get("points").getAsInt());
        }

        List<Pair<String, String>> goalIds = new ArrayList<>();
        List<Integer> completedBy = new ArrayList<>();
        for (JsonElement element : json.getAsJsonArray("goals")) {
            JsonObject goalJson = element.getAsJsonObject();
            goalIds.add(new Pair<>(goalJson.get("id").getAsString(), goalJson.get("data").getAsString()));
            completedBy.add(goalJson.get("completedBy").getAsInt());
        }

        Lockout lockout = new Lockout(new LockoutBoard(goalIds), teams);
        lockout.setTicks(json.get("ticks").getAsLong());
        for (int i = 0; i < goalIds.size(); i++) {
            if (completedBy.get(i) >= 0) {
                lockout.getBoard().getGoals().get(i).setCompleted(true, teams.get(completedBy.get(i)));
            }
        }
        // Points are saved as-is: opponent goals can give points differently from completions.
        for (int i = 0; i < teams.size(); i++) {
            for (int p = 0; p < points.get(i); p++) {
                teams.get(i).addPoint();
            }
        }

        JsonObject trackers = json.getAsJsonObject("trackers");
        for (Field field : trackerFields()) {
            if (!trackers.has(field.getName())) continue; // saved by an older version
            Object value = decode(trackers.get(field.getName()), field.getGenericType(), teams, field.getName());
            if (Modifier.isFinal(field.getModifiers())) {
                // Final collections: fill the existing instance.
                Object existing = field.get(lockout);
                if (existing instanceof Map map) {
                    map.clear();
                    map.putAll((Map) value);
                } else {
                    throw new IllegalStateException("Can't restore final field Lockout." + field.getName());
                }
            } else {
                field.set(lockout, value);
            }
        }
        return lockout;
    }

    static List<Field> trackerFields() {
        List<Field> fields = new ArrayList<>();
        for (Field field : Lockout.class.getDeclaredFields()) {
            if (Modifier.isStatic(field.getModifiers()) || EXPLICIT_FIELDS.contains(field.getName())) continue;
            field.setAccessible(true);
            fields.add(field);
        }
        return fields;
    }

    private static JsonElement encode(Object value, Type type, List<? extends LockoutTeam> teams, String fieldName) {
        if (value == null) return JsonNull.INSTANCE;
        Class<?> raw = rawClass(type);

        if (raw == UUID.class) return new JsonPrimitive(value.toString());
        if (value instanceof Number number) return new JsonPrimitive(number);
        if (value instanceof Boolean bool) return new JsonPrimitive(bool);
        if (raw == Item.class) return new JsonPrimitive(BuiltInRegistries.ITEM.getKey((Item) value).toString());
        if (raw == Block.class) return new JsonPrimitive(BuiltInRegistries.BLOCK.getKey((Block) value).toString());
        if (raw == EntityType.class) return new JsonPrimitive(BuiltInRegistries.ENTITY_TYPE.getKey((EntityType<?>) value).toString());
        if (raw == Identifier.class) return new JsonPrimitive(value.toString());
        if (LockoutTeam.class.isAssignableFrom(raw)) return new JsonPrimitive(teams.indexOf(value));

        if (Map.class.isAssignableFrom(raw)) {
            Type[] args = ((ParameterizedType) type).getActualTypeArguments();
            JsonArray entries = new JsonArray();
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
                JsonArray pair = new JsonArray();
                pair.add(encode(entry.getKey(), args[0], teams, fieldName));
                pair.add(encode(entry.getValue(), args[1], teams, fieldName));
                entries.add(pair);
            }
            return entries;
        }
        if (Collection.class.isAssignableFrom(raw)) {
            Type elementType = ((ParameterizedType) type).getActualTypeArguments()[0];
            JsonArray elements = new JsonArray();
            for (Object element : (Collection<?>) value) {
                elements.add(encode(element, elementType, teams, fieldName));
            }
            return elements;
        }
        throw new IllegalStateException("Don't know how to save Lockout." + fieldName + " (" + type + ")");
    }

    private static Object decode(JsonElement json, Type type, List<LockoutTeamServer> teams, String fieldName) {
        if (json == null || json.isJsonNull()) return null;
        Class<?> raw = rawClass(type);

        if (raw == UUID.class) return UUID.fromString(json.getAsString());
        if (raw == Integer.class || raw == int.class) return json.getAsInt();
        if (raw == Long.class || raw == long.class) return json.getAsLong();
        if (raw == Double.class || raw == double.class) return json.getAsDouble();
        if (raw == Float.class || raw == float.class) return json.getAsFloat();
        if (raw == Boolean.class || raw == boolean.class) return json.getAsBoolean();
        if (raw == Item.class) return BuiltInRegistries.ITEM.getValue(Identifier.parse(json.getAsString()));
        if (raw == Block.class) return BuiltInRegistries.BLOCK.getValue(Identifier.parse(json.getAsString()));
        if (raw == EntityType.class) return BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.parse(json.getAsString()));
        if (raw == Identifier.class) return Identifier.parse(json.getAsString());
        if (LockoutTeam.class.isAssignableFrom(raw)) return teams.get(json.getAsInt());

        if (Map.class.isAssignableFrom(raw)) {
            Type[] args = ((ParameterizedType) type).getActualTypeArguments();
            Map<Object, Object> map = new LinkedHashMap<>();
            for (JsonElement entry : json.getAsJsonArray()) {
                JsonArray pair = entry.getAsJsonArray();
                map.put(decode(pair.get(0), args[0], teams, fieldName), decode(pair.get(1), args[1], teams, fieldName));
            }
            return map;
        }
        if (Collection.class.isAssignableFrom(raw)) {
            Type elementType = ((ParameterizedType) type).getActualTypeArguments()[0];
            Collection<Object> collection = List.class.isAssignableFrom(raw) ? new ArrayList<>() : new LinkedHashSet<>();
            for (JsonElement element : json.getAsJsonArray()) {
                collection.add(decode(element, elementType, teams, fieldName));
            }
            return collection;
        }
        throw new IllegalStateException("Don't know how to restore Lockout." + fieldName + " (" + type + ")");
    }

    private static Class<?> rawClass(Type type) {
        if (type instanceof Class<?> c) return c;
        if (type instanceof ParameterizedType p) return (Class<?>) p.getRawType();
        if (type instanceof WildcardType w) return rawClass(w.getUpperBounds()[0]);
        throw new IllegalStateException("Unsupported type " + type);
    }

}
