package me.marin.lockout.mixin.server;

import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalIntRef;
import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.PickUpOpponentItemGoal;
import me.marin.lockout.server.DroppedItemOwners;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Objects;
import java.util.UUID;

@Mixin(ItemEntity.class)
public class ItemEntityMixin {

    @Inject(method = "playerTouch", at = @At("HEAD"))
    private void beforePickup(Player player, CallbackInfo ci, @Share("countBefore") LocalIntRef countBefore) {
        countBefore.set(((ItemEntity) (Object) this).getItem().getCount());
    }

    @Inject(method = "playerTouch", at = @At("RETURN"))
    private void afterPickup(Player player, CallbackInfo ci, @Share("countBefore") LocalIntRef countBefore) {
        ItemEntity item = (ItemEntity) (Object) this;
        if (player.level().isClientSide()) return;
        boolean pickedUp = item.isRemoved() || item.getItem().getCount() < countBefore.get();
        if (!pickedUp) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        UUID owner = DroppedItemOwners.get(item);
        if (owner == null || !lockout.isLockoutPlayer(owner) || !lockout.isLockoutPlayer(player.getUUID())) return;
        LockoutTeam ownerTeam = lockout.getPlayerTeam(owner);
        if (Objects.equals(ownerTeam, lockout.getPlayerTeam(player.getUUID()))) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof PickUpOpponentItemGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
