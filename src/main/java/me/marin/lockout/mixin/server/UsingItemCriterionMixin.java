package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.interfaces.SpyMobGoal;
import me.marin.lockout.lockout.interfaces.SpyUniqueMobsGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.advancements.triggers.UsingItemTrigger;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.projectile.ProjectileUtil;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.LinkedHashSet;

@Mixin(UsingItemTrigger.class)
public class UsingItemCriterionMixin {

    private static final double SPYGLASS_RANGE = 100.0;

    // Called every tick while an item is being used.
    @Inject(method = "trigger", at = @At("HEAD"))
    public void onUsingItem(ServerPlayer player, ItemStack stack, CallbackInfo ci) {
        if (!stack.is(Items.SPYGLASS)) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        if (!lockout.isLockoutPlayer(player.getUUID())) return;

        boolean hasSpyGoal = lockout.getBoard().getGoals().stream()
                .anyMatch(goal -> goal != null && !goal.isCompleted() && (goal instanceof SpyMobGoal || goal instanceof SpyUniqueMobsGoal));
        if (!hasSpyGoal) return;

        // Same check as the vanilla spyglass advancements (player predicate "looking_at").
        Vec3 eye = player.getEyePosition();
        Vec3 view = player.getViewVector(1.0F);
        Vec3 end = eye.add(view.x * SPYGLASS_RANGE, view.y * SPYGLASS_RANGE, view.z * SPYGLASS_RANGE);
        EntityHitResult hit = ProjectileUtil.getEntityHitResult(player.level(), player, eye, end, new AABB(eye, end).inflate(1.0), entity -> !entity.isSpectator(), 0.0F);
        if (hit == null || !(hit.getEntity() instanceof Mob mob) || !player.hasLineOfSight(mob)) return;

        LockoutTeamServer team = (LockoutTeamServer) lockout.getPlayerTeam(player.getUUID());
        boolean newMobType = lockout.spiedMobs.computeIfAbsent(team, t -> new LinkedHashSet<>()).add(mob.getType());

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (newMobType && goal instanceof SpyUniqueMobsGoal spyUniqueMobsGoal) {
                spyUniqueMobsGoal.evaluate(team);
            }
            if (goal instanceof SpyMobGoal spyMobGoal && mob.getType() == spyMobGoal.getEntityType()) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
