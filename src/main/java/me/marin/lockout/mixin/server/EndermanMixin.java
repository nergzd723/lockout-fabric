package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.EnrageEndermanGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.Enderman;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Enderman.class)
public class EndermanMixin {

    @Inject(method = "setTarget", at = @At("HEAD"))
    public void onSetTarget(LivingEntity target, CallbackInfo ci) {
        Enderman mob = (Enderman) (Object) this;
        if (!(target instanceof Player player)) return;
        if (mob.level().isClientSide()) return;
        if (mob.getTarget() == target) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof EnrageEndermanGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
