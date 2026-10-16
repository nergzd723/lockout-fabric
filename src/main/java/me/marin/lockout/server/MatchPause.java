package me.marin.lockout.server;

import me.marin.lockout.Constants;
import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.network.StartLockoutPayload;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.ChatFormatting;
import net.minecraft.core.Holder;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.level.GameType;
import net.minecraft.world.phys.Vec3;

import java.util.*;

/**
 * A match restored after a server restart stays paused until every player is back online.
 * Players who are already back are frozen in adventure mode (and can't be hurt); once everyone is online,
 * a short countdown resumes the match. The match timer doesn't run while paused.
 */
public class MatchPause {

    private static final int RESUME_COUNTDOWN_SECONDS = 5;
    private static final int WAITING_MESSAGE_INTERVAL_TICKS = 20 * 15;
    private static final double MAX_DRIFT = 0.5;
    private static final Identifier FREEZE_MODIFIER_ID = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "paused");
    private static final List<Holder<Attribute>> FROZEN_ATTRIBUTES = List.of(Attributes.MOVEMENT_SPEED, Attributes.JUMP_STRENGTH);

    private static boolean paused = false;
    private static int resumeCountdownTicks = -1;
    private static int pausedTicks = 0;
    private static final Map<UUID, Vec3> frozenPositions = new HashMap<>();

    public static boolean isPaused() {
        return paused;
    }

    public static void reset() {
        paused = false;
        resumeCountdownTicks = -1;
        pausedTicks = 0;
        frozenPositions.clear();
    }

    public static void pause() {
        paused = true;
        resumeCountdownTicks = -1;
        pausedTicks = 0;
    }

    /**
     * Ends the pause without resuming, e.g. when a new match is started instead. Unfreezes everyone.
     */
    public static void cancel(MinecraftServer server) {
        if (!paused) return;
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            removeFreeze(player);
        }
        reset();
    }

    public static void tick(MinecraftServer server, Lockout lockout) {
        pausedTicks++;
        List<String> missing = new ArrayList<>();
        for (LockoutTeam team : lockout.getTeams()) {
            LockoutTeamServer serverTeam = (LockoutTeamServer) team;
            for (UUID playerId : serverTeam.getPlayers()) {
                ServerPlayer player = server.getPlayerList().getPlayer(playerId);
                if (player == null) {
                    missing.add(serverTeam.getPlayerName(playerId));
                } else {
                    keepFrozen(player);
                }
            }
        }

        if (!missing.isEmpty()) {
            if (resumeCountdownTicks >= 0) {
                broadcast(server, Component.literal(String.join(", ", missing) + " left. Waiting for everyone to be back.").withStyle(ChatFormatting.YELLOW));
                resumeCountdownTicks = -1;
            } else if (pausedTicks % WAITING_MESSAGE_INTERVAL_TICKS == 1) {
                broadcast(server, waitingMessage(missing));
            }
            return;
        }

        if (resumeCountdownTicks < 0) {
            resumeCountdownTicks = RESUME_COUNTDOWN_SECONDS * 20;
            broadcast(server, Component.literal("Everyone is back! Resuming in " + RESUME_COUNTDOWN_SECONDS + "...").withStyle(ChatFormatting.GREEN));
            return;
        }
        resumeCountdownTicks--;
        if (resumeCountdownTicks == 0) {
            resume(server, lockout);
        } else if (resumeCountdownTicks % 20 == 0 && resumeCountdownTicks <= 3 * 20) {
            broadcast(server, Component.literal("Resuming in " + resumeCountdownTicks / 20 + "..."));
        }
    }

    public static Component waitingMessage(List<String> missing) {
        return Component.literal("The match is paused until everyone is back. Waiting for: " + String.join(", ", missing)).withStyle(ChatFormatting.YELLOW);
    }

    public static List<String> getMissingPlayers(MinecraftServer server, Lockout lockout) {
        List<String> missing = new ArrayList<>();
        for (LockoutTeam team : lockout.getTeams()) {
            LockoutTeamServer serverTeam = (LockoutTeamServer) team;
            for (UUID playerId : serverTeam.getPlayers()) {
                if (server.getPlayerList().getPlayer(playerId) == null) {
                    missing.add(serverTeam.getPlayerName(playerId));
                }
            }
        }
        return missing;
    }

    private static void resume(MinecraftServer server, Lockout lockout) {
        reset();
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            if (!lockout.isLockoutPlayer(player.getUUID())) continue;
            removeFreeze(player);
            player.setGameMode(GameType.SURVIVAL);
        }
        // Restarted during the pre-match countdown: start right away.
        if (lockout.getTicks() < 0) {
            lockout.setTicks(0);
        }
        lockout.setStarted(true);
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            ServerPlayNetworking.send(player, StartLockoutPayload.INSTANCE);
            ServerPlayNetworking.send(player, lockout.getUpdateTimerPacket());
        }
        broadcast(server, Component.literal("The match has resumed!").withStyle(ChatFormatting.GREEN));
    }

    /**
     * Freezes the player in place (again, if they respawned or the freeze was lost) and pulls them back if they drifted.
     */
    private static void keepFrozen(ServerPlayer player) {
        AttributeInstance movement = player.getAttribute(Attributes.MOVEMENT_SPEED);
        if (movement != null && !movement.hasModifier(FREEZE_MODIFIER_ID)) {
            player.setGameMode(GameType.ADVENTURE);
            for (Holder<Attribute> attribute : FROZEN_ATTRIBUTES) {
                AttributeInstance instance = player.getAttribute(attribute);
                if (instance != null && !instance.hasModifier(FREEZE_MODIFIER_ID)) {
                    // Transient: never saved with the player, so a crash can't leave anyone frozen.
                    instance.addTransientModifier(new AttributeModifier(FREEZE_MODIFIER_ID, -1.0, AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL));
                }
            }
            player.getAbilities().invulnerable = true;
            player.onUpdateAbilities();
            frozenPositions.put(player.getUUID(), player.position());
            return;
        }

        Vec3 frozenAt = frozenPositions.get(player.getUUID());
        if (frozenAt != null && player.position().distanceTo(frozenAt) > MAX_DRIFT) {
            player.teleportTo(frozenAt.x, frozenAt.y, frozenAt.z);
        }
    }

    private static void removeFreeze(ServerPlayer player) {
        boolean wasFrozen = false;
        for (Holder<Attribute> attribute : FROZEN_ATTRIBUTES) {
            AttributeInstance instance = player.getAttribute(attribute);
            if (instance != null) {
                wasFrozen |= instance.removeModifier(FREEZE_MODIFIER_ID);
            }
        }
        if (wasFrozen) {
            player.getAbilities().invulnerable = false;
            player.onUpdateAbilities();
        }
        frozenPositions.remove(player.getUUID());
    }

    private static void broadcast(MinecraftServer server, Component message) {
        server.getPlayerList().broadcastSystemMessage(message, false);
    }

}
