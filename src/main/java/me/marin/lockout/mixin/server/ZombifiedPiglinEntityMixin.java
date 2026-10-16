package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.AngerZombifiedPiglinGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.EntityReference;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.zombie.ZombifiedPiglin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;


@Mixin(ZombifiedPiglin.class)
public class ZombifiedPiglinEntityMixin {

    @Inject(method = "setPersistentAngerTarget", at = @At("HEAD"))
    public void setAngryAt(EntityReference<LivingEntity> angryAt, CallbackInfo ci) {
        ZombifiedPiglin pigman = (ZombifiedPiglin) (Object) this;
        if (pigman.level().isClientSide()) return;
        if (angryAt == null) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) {
            return;
        }

        ServerPlayer player;
        try {
            player = pigman.level().getServer().getPlayerList().getPlayer(angryAt.getUUID());
            if (player == null) {
                return;
            }
        } catch (Exception ignored) {
            // angryAt UUID does not belong to a player, ignore
            return;
        }

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (!(goal instanceof AngerZombifiedPiglinGoal)) continue;
            if (goal.isCompleted()) continue;

            lockout.completeGoal(goal, player);
            return;
        }
    }

}
