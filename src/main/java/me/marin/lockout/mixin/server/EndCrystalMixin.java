package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.ExplodeEndCrystalGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.boss.enderdragon.EndCrystal;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(EndCrystal.class)
public class EndCrystalMixin {

    @Inject(method = "hurtServer", at = @At("RETURN"))
    public void onHurt(ServerLevel level, DamageSource source, float amount, CallbackInfoReturnable<Boolean> cir) {
        EndCrystal crystal = (EndCrystal) (Object) this;
        if (!cir.getReturnValue() || !crystal.isRemoved()) return;
        // Damage credited to a player, including their arrows and explosions they caused.
        if (!(source.getEntity() instanceof Player player)) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof ExplodeEndCrystalGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
