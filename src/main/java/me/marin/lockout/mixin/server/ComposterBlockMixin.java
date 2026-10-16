package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.interfaces.CompostUniqueFoodsGoal;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.BlockHitResult;
import java.util.LinkedHashSet;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.workstation.UseComposterGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.ComposterBlock;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(ComposterBlock.class)
public class ComposterBlockMixin {

    @Inject(method = "extractProduce", at = @At("RETURN"))
    private static void emptyFullComposterMixin(Entity user, BlockState state, Level world, BlockPos pos, CallbackInfoReturnable<BlockState> cir) {
        if (user.level().isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        if (!(user instanceof Player player)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof UseComposterGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

    @Inject(method = "useItemOn", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/ComposterBlock;addLayer(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/server/level/ServerLevel;Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/item/component/Compostable;)Lnet/minecraft/world/level/block/state/BlockState;"))
    private void onCompostItem(ItemStack stack, BlockState state, Level level, BlockPos pos, Player player, InteractionHand hand, BlockHitResult hitResult, CallbackInfoReturnable<InteractionResult> cir) {
        if (level.isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;
        if (!lockout.isLockoutPlayer(player.getUUID())) return;
        if (!stack.has(DataComponents.FOOD)) return;

        LockoutTeamServer team = (LockoutTeamServer) lockout.getPlayerTeam(player.getUUID());
        if (!lockout.compostedFoods.computeIfAbsent(team, t -> new LinkedHashSet<>()).add(stack.getItem())) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal instanceof CompostUniqueFoodsGoal compostUniqueFoodsGoal) {
                compostUniqueFoodsGoal.evaluate(team);
            }
        }
    }

}
