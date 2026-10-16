package me.marin.lockout.mixin.server;

import me.marin.lockout.lockout.goals.misc.LockMapGoal;
import me.marin.lockout.lockout.goals.workstation.UseCartographyTableGoal;
import net.minecraft.world.inventory.CartographyTableMenu;
import net.minecraft.world.item.MapItem;
import net.minecraft.world.level.saveddata.maps.MapItemSavedData;
import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.workstation.UseLoomGoal;
import me.marin.lockout.lockout.goals.workstation.UseStonecutterGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.LoomMenu;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.inventory.StonecutterMenu;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Slot.class)
public class SlotMixin {

    @Inject(method="onTake", at = @At("HEAD"))
    public void onTakeItem(Player player, ItemStack stack, CallbackInfo ci) {
        if (player.level().isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof UseStonecutterGoal) {
                if (player.containerMenu instanceof StonecutterMenu stonecutterScreenHandler) {
                    if ((Object) this == stonecutterScreenHandler.slots.get(1)) {
                        lockout.completeGoal(goal, player);
                    }
                }
            }

            if (player.containerMenu instanceof CartographyTableMenu cartographyTableMenu && (Object) this == cartographyTableMenu.slots.get(CartographyTableMenu.RESULT_SLOT)) {
                if (goal instanceof UseCartographyTableGoal) {
                    lockout.completeGoal(goal, player);
                }
                // The map is locked by the time the result slot hands it over.
                MapItemSavedData mapData = MapItem.getSavedData(stack, player.level());
                if (goal instanceof LockMapGoal && mapData != null && mapData.locked) {
                    lockout.completeGoal(goal, player);
                }
            }
            if (goal instanceof UseLoomGoal) {
                if (player.containerMenu instanceof LoomMenu loomScreenHandler) {
                    if ((Object) this == loomScreenHandler.getResultSlot()) {
                        lockout.completeGoal(goal, player);
                    }
                }
            }

        }
    }

}
