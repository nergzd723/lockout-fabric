package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.ConstructCopperGolemGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.advancements.triggers.SummonedEntityTrigger;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityTypes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(SummonedEntityTrigger.class)
public class SummonedEntityCriterionMixin {

    // Building a golem triggers this for every player near the pumpkin.
    @Inject(method = "trigger", at = @At("HEAD"))
    public void onSummonEntity(ServerPlayer player, Entity entity, CallbackInfo ci) {
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        if (entity.getType() != EntityTypes.COPPER_GOLEM) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof ConstructCopperGolemGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
