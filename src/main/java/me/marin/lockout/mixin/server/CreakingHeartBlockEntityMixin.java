package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.kill.KillCreakingGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.monster.creaking.Creaking;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.block.entity.CreakingHeartBlockEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Optional;

@Mixin(CreakingHeartBlockEntity.class)
public abstract class CreakingHeartBlockEntityMixin {

    @Shadow
    private Optional<Creaking> getCreakingProtector() {
        throw new AssertionError();
    }

    // A Creaking can't be killed directly: breaking its heart while it's out is what kills it.
    @Inject(method = "removeProtector", at = @At("HEAD"))
    public void onRemoveProtector(DamageSource source, CallbackInfo ci) {
        if (source == null || !(source.getEntity() instanceof Player player)) return;
        if (player.level().isClientSide() || getCreakingProtector().isEmpty()) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof KillCreakingGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
