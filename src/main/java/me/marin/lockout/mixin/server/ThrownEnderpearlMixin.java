package me.marin.lockout.mixin.server;

import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalRef;
import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.PearlThroughNetherPortalGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.projectile.throwableitemprojectile.ThrownEnderpearl;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.HitResult;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Set;

@Mixin(ThrownEnderpearl.class)
public class ThrownEnderpearlMixin {

    @Inject(method = "onHit", at = @At("HEAD"))
    private void beforeTeleport(HitResult hitResult, CallbackInfo ci, @Share("owner") LocalRef<ServerPlayer> owner, @Share("fromDimension") LocalRef<ResourceKey<Level>> fromDimension) {
        ThrownEnderpearl pearl = (ThrownEnderpearl) (Object) this;
        if (pearl.level().isClientSide()) return;
        if (pearl.getOwner() instanceof ServerPlayer player) {
            owner.set(player);
            fromDimension.set(player.level().dimension());
        }
    }

    // The pearl went through a portal, landed in the other dimension and pulled its owner along.
    @Inject(method = "onHit", at = @At("TAIL"))
    private void afterTeleport(HitResult hitResult, CallbackInfo ci, @Share("owner") LocalRef<ServerPlayer> owner, @Share("fromDimension") LocalRef<ResourceKey<Level>> fromDimension) {
        ServerPlayer player = owner.get();
        if (player == null) return;
        ResourceKey<Level> from = fromDimension.get();
        ResourceKey<Level> to = player.level().dimension();
        if (from == to || !Set.of(from, to).equals(Set.of(Level.OVERWORLD, Level.NETHER))) return;

        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof PearlThroughNetherPortalGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
