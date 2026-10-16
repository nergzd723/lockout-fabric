package me.marin.lockout.mixin.server;

import net.minecraft.server.level.ServerPlayer;
import net.minecraft.network.protocol.game.ClientboundSoundPacket;
import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.have_more.HaveMostUniqueCraftsGoal;
import me.marin.lockout.lockout.goals.misc.DuplicateArmorTrimGoal;
import me.marin.lockout.lockout.goals.obtain.Obtain2UniqueArmorTrimsGoal;
import me.marin.lockout.lockout.interfaces.CraftUniqueItemsGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.CraftingMenu;
import net.minecraft.world.inventory.InventoryMenu;
import net.minecraft.world.inventory.ResultSlot;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.Objects;
import java.util.Set;

@Mixin(ResultSlot.class)
public class CraftingResultSlotMixin {

    @Shadow @Final
    private Player player;

    @Shadow
    private int removeCount;

    @Inject(method = "checkTakeAchievements(Lnet/minecraft/world/item/ItemStack;)V", at = @At("HEAD"))
    public void onCraft(ItemStack stack, CallbackInfo ci) {
        if (player.level().isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        if (removeCount < 0 || stack.isEmpty()) {
            return;
        }

        if (!(player.containerMenu instanceof CraftingMenu || player.containerMenu instanceof InventoryMenu)) return;

        if (lockout.isLockoutPlayer(player.getUUID())) {
            LockoutTeamServer team = (LockoutTeamServer) lockout.getPlayerTeam(player.getUUID());
            boolean newTeamCraft = lockout.uniqueItemsCraftedByTeam.computeIfAbsent(team, t -> new LinkedHashSet<>()).add(stack.getItem());
            boolean armorTrim = Obtain2UniqueArmorTrimsGoal.ARMOR_TRIMS.contains(stack.getItem());

            for (Goal goal : lockout.getBoard().getGoals()) {
                if (goal == null) continue;
                if (goal.isCompleted()) continue;

                if (newTeamCraft && goal instanceof CraftUniqueItemsGoal craftUniqueItemsGoal) {
                    craftUniqueItemsGoal.evaluate(team);
                }
                // Trim templates can only be crafted by duplicating one
                if (armorTrim && goal instanceof DuplicateArmorTrimGoal) {
                    lockout.completeGoal(goal, player);
                }
            }
        }

        lockout.uniqueCrafts.putIfAbsent(player.getUUID(), new HashSet<>());
        Set<Item> crafts = lockout.uniqueCrafts.get(player.getUUID());
        boolean addedNew = crafts.add(stack.getItem());

        if (!addedNew) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;

            if (goal instanceof HaveMostUniqueCraftsGoal) {
                ((ServerPlayer) player).connection.send(new ClientboundSoundPacket(SoundEvents.NOTE_BLOCK_IRON_XYLOPHONE, SoundSource.BLOCKS, player.getX(), player.getY(), player.getZ(), 2, 2, player.getRandom().nextLong()));
                if (crafts.size() % 5 == 0) {
                    player.sendSystemMessage(Component.nullToEmpty(ChatFormatting.GRAY + "" + ChatFormatting.ITALIC + "You have crafted " + crafts.size() + " unique items."));
                }
                player.sendOverlayMessage(Component.nullToEmpty("Unique crafts: " + crafts.size()));

                if (crafts.size() > lockout.mostUniqueCrafts) {
                    if (!Objects.equals(lockout.mostUniqueCraftsPlayer, player.getUUID())) {
                        lockout.updateGoalCompletion(goal, player.getUUID());
                    }

                    lockout.mostUniqueCraftsPlayer = player.getUUID();
                    lockout.mostUniqueCrafts = crafts.size();
                }
            }
        }
    }

}
