package me.marin.lockout.server;

import me.marin.lockout.Constants;
import me.marin.lockout.LockoutTeamServer;
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
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.phys.Vec3;

import java.util.*;

/**
 * Before the first pick, drafting players get a few seconds to look around. They're frozen in place, but jumping
 * toggles a bird's-eye view 30 blocks up, and they're handed a Spyglass to zoom with (right click).
 * Everything happens on the server; no client support is needed.
 */
public class DraftScouting {

    private static final double BIRDS_EYE_HEIGHT = 30.0;
    private static final double MAX_DRIFT = 0.5;
    private static final float DEFAULT_FLYING_SPEED = new Abilities().getFlyingSpeed();
    private static final Identifier FREEZE_MODIFIER_ID = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "scouting");
    private static final List<Holder<Attribute>> FROZEN_ATTRIBUTES = List.of(Attributes.MOVEMENT_SPEED, Attributes.JUMP_STRENGTH);

    private final MinecraftServer server;
    private final Set<UUID> players = new HashSet<>();
    private final Map<UUID, Vec3> groundPositions = new HashMap<>();
    private final Set<UUID> inBirdsEye = new HashSet<>();
    private final Set<UUID> wasJumping = new HashSet<>();
    private int ticksLeft;

    public DraftScouting(MinecraftServer server, List<LockoutTeamServer> teams, int seconds) {
        this.server = server;
        this.ticksLeft = seconds * 20;
        for (LockoutTeamServer team : teams) {
            players.addAll(team.getPlayers());
        }
    }

    public void start() {
        server.getPlayerList().broadcastSystemMessage(Component.literal("Scouting for " + (ticksLeft / 20) + "s before the draft: jump for a bird's-eye view, right click the Spyglass to zoom.").withStyle(ChatFormatting.AQUA), false);
        for (UUID playerId : players) {
            ServerPlayer player = server.getPlayerList().getPlayer(playerId);
            if (player != null) {
                freeze(player);
            }
        }
    }

    /**
     * @return true once scouting is over
     */
    public boolean tick() {
        ticksLeft--;
        for (UUID playerId : players) {
            ServerPlayer player = server.getPlayerList().getPlayer(playerId);
            if (player == null) continue;
            if (!groundPositions.containsKey(playerId)) {
                freeze(player); // joined during scouting
            }

            boolean jumping = player.getLastClientInput().jump();
            if (jumping && !wasJumping.contains(playerId)) {
                toggleBirdsEye(player);
            }
            if (jumping) wasJumping.add(playerId); else wasJumping.remove(playerId);

            Vec3 anchor = anchor(playerId);
            if (player.position().distanceTo(anchor) > MAX_DRIFT) {
                player.teleportTo(anchor.x, anchor.y, anchor.z);
            }
            if (ticksLeft % 20 == 0) {
                player.sendOverlayMessage(Component.literal(ChatFormatting.AQUA + "Scouting: " + (ticksLeft / 20) + "s" + ChatFormatting.GRAY + "  ·  Jump: bird's-eye view  ·  Right click: zoom"));
            }
        }
        if (ticksLeft <= 0) {
            end();
            return true;
        }
        return false;
    }

    /**
     * Puts everyone back on the ground and gives back control. Safe to call more than once.
     */
    public void end() {
        for (UUID playerId : new ArrayList<>(groundPositions.keySet())) {
            ServerPlayer player = server.getPlayerList().getPlayer(playerId);
            if (player != null) {
                release(player);
            }
        }
        groundPositions.clear();
        inBirdsEye.clear();
    }

    private Vec3 anchor(UUID playerId) {
        Vec3 ground = groundPositions.get(playerId);
        return inBirdsEye.contains(playerId) ? ground.add(0, BIRDS_EYE_HEIGHT, 0) : ground;
    }

    private void toggleBirdsEye(ServerPlayer player) {
        UUID playerId = player.getUUID();
        if (!inBirdsEye.remove(playerId)) {
            inBirdsEye.add(playerId);
        }
        // Hover while up high: flying with no flying speed means no falling and no moving.
        Abilities abilities = player.getAbilities();
        abilities.mayfly = inBirdsEye.contains(playerId);
        abilities.flying = inBirdsEye.contains(playerId);
        player.onUpdateAbilities();
        Vec3 anchor = anchor(playerId);
        player.teleportTo(anchor.x, anchor.y, anchor.z);
    }

    private void freeze(ServerPlayer player) {
        groundPositions.put(player.getUUID(), player.position());
        for (Holder<Attribute> attribute : FROZEN_ATTRIBUTES) {
            AttributeInstance instance = player.getAttribute(attribute);
            if (instance != null && !instance.hasModifier(FREEZE_MODIFIER_ID)) {
                // Transient: never saved with the player.
                instance.addTransientModifier(new AttributeModifier(FREEZE_MODIFIER_ID, -1.0, AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL));
            }
        }
        Abilities abilities = player.getAbilities();
        abilities.invulnerable = true;
        abilities.setFlyingSpeed(0);
        player.onUpdateAbilities();

        if (!player.getInventory().contains(stack -> stack.is(Items.SPYGLASS))) {
            player.getInventory().add(new ItemStack(Items.SPYGLASS));
        }
    }

    private void release(ServerPlayer player) {
        Vec3 ground = groundPositions.get(player.getUUID());
        if (ground != null) {
            player.teleportTo(ground.x, ground.y, ground.z);
        }
        for (Holder<Attribute> attribute : FROZEN_ATTRIBUTES) {
            AttributeInstance instance = player.getAttribute(attribute);
            if (instance != null) {
                instance.removeModifier(FREEZE_MODIFIER_ID);
            }
        }
        Abilities abilities = player.getAbilities();
        abilities.invulnerable = false;
        abilities.mayfly = false;
        abilities.flying = false;
        abilities.setFlyingSpeed(DEFAULT_FLYING_SPEED);
        player.onUpdateAbilities();
        for (int slot = 0; slot < player.getInventory().getContainerSize(); slot++) {
            if (player.getInventory().getItem(slot).is(Items.SPYGLASS)) {
                player.getInventory().setItem(slot, ItemStack.EMPTY);
            }
        }
    }

}
