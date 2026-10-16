package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.UseGoldenDandelionGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.entity.AgeableMob;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.function.Consumer;
import java.util.function.Supplier;

@Mixin(AgeableMob.class)
public class AgeableMobMixin {

    // Every mob that can be fed a Golden Dandelion (after the baby check) goes through this.
    @Inject(method = "setAgeLocked(Lnet/minecraft/world/entity/Mob;Ljava/util/function/Supplier;Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/item/ItemStack;Ljava/util/function/Consumer;)V", at = @At("HEAD"))
    private static void onUseGoldenDandelion(Mob mob, Supplier<Boolean> isAgeLocked, Player player, ItemStack stack, Consumer<Mob> setAgeLocked, CallbackInfo ci) {
        if (player == null || mob.level().isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof UseGoldenDandelionGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
