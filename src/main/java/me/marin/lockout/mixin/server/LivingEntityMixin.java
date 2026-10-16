package me.marin.lockout.mixin.server;

import me.marin.lockout.lockout.goals.misc.ShootOpponentWithArrowGoal;
import me.marin.lockout.server.DroppedItemOwners;
import net.minecraft.world.entity.projectile.arrow.AbstractArrow;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.server.level.ServerPlayer;
import java.util.Objects;
import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.Deal400DamageGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(LivingEntity.class)
public class LivingEntityMixin {

    @Inject(method = "createItemStackToDrop", at = @At("RETURN"))
    public void onCreateDrop(ItemStack stack, boolean randomly, boolean includeThrower, CallbackInfoReturnable<ItemEntity> cir) {
        // Remember who dropped it, also for death drops (which vanilla doesn't mark).
        if ((Object) this instanceof ServerPlayer player && cir.getReturnValue() != null) {
            DroppedItemOwners.record(cir.getReturnValue(), player.getUUID());
        }
    }

    @Inject(method = "hurtServer", at = @At("RETURN"))
    public void onDamage(ServerLevel world, DamageSource source, float amount, CallbackInfoReturnable<Boolean> cir) {
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        if (!(source.getEntity() instanceof Player player) || !cir.getReturnValue()) return;
        if (player.level().isClientSide()) return;

        if (!lockout.isLockoutPlayer(player.getUUID())) return;
        LockoutTeamServer team = (LockoutTeamServer) lockout.getPlayerTeam(player.getUUID());
        lockout.damageDealt.putIfAbsent(team, 0d);
        lockout.damageDealt.merge(team, (double)amount, Double::sum);

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof ShootOpponentWithArrowGoal && source.getDirectEntity() instanceof AbstractArrow
                    && (Object) this instanceof Player target && lockout.isLockoutPlayer(target.getUUID())
                    && !Objects.equals(lockout.getPlayerTeam(target.getUUID()), team)) {
                lockout.completeGoal(goal, player);
            }
            if (goal instanceof Deal400DamageGoal deal400DamageGoal) {
                team.sendTooltipUpdate(deal400DamageGoal);
                if (lockout.damageDealt.get(team) >= 400) {
                    lockout.completeGoal(goal, player);
                }
            }
        }
    }

}
