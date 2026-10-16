package me.marin.lockout.mixin.server;

import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.goals.mine.Mine3TypesOfIceGoal;
import java.util.LinkedHashSet;
import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.interfaces.MineBlockGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Block.class)
public class BlockMixin {

    @Inject(method = "playerWillDestroy", at = @At("HEAD"))
    public void onBreak(Level world, BlockPos pos, BlockState state, Player player, CallbackInfoReturnable<BlockState> cir) {
        if (player.level().isClientSide()) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        if (Mine3TypesOfIceGoal.ICE.contains(state.getBlock()) && lockout.isLockoutPlayer(player.getUUID())) {
            LockoutTeamServer team = (LockoutTeamServer) lockout.getPlayerTeam(player.getUUID());
            if (lockout.iceTypesMined.computeIfAbsent(team, t -> new LinkedHashSet<>()).add(state.getBlock())) {
                for (Goal goal : lockout.getBoard().getGoals()) {
                    if (goal instanceof Mine3TypesOfIceGoal mineIceGoal) {
                        mineIceGoal.evaluate(team);
                    }
                }
            }
        }

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (!(goal instanceof MineBlockGoal mineBlockGoal)) continue;
            if (goal.isCompleted()) continue;

            if (mineBlockGoal.getItems().contains(state.getBlock().asItem())) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
