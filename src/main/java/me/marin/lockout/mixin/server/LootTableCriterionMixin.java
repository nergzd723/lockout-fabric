package me.marin.lockout.mixin.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.misc.LootAbandonedCampGoal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.advancements.triggers.LootTableTrigger;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.storage.loot.BuiltInLootTables;
import net.minecraft.world.level.storage.loot.LootTable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Set;

@Mixin(LootTableTrigger.class)
public class LootTableCriterionMixin {

    private static final Set<ResourceKey<LootTable>> ABANDONED_CAMP_LOOT = Set.of(
            BuiltInLootTables.ABANDONED_CAMP_BARREL, BuiltInLootTables.ABANDONED_CAMP_COMMON_CHEST, BuiltInLootTables.ABANDONED_CAMP_SECRET_CHEST);

    // Fires when a player opens a container that still has its loot table, i.e. hasn't been looted yet.
    @Inject(method = "trigger", at = @At("HEAD"))
    public void onLootTable(ServerPlayer player, ResourceKey<LootTable> lootTable, CallbackInfo ci) {
        if (!ABANDONED_CAMP_LOOT.contains(lootTable)) return;
        Lockout lockout = LockoutServer.lockout;
        if (!Lockout.isLockoutRunning(lockout)) return;

        for (Goal goal : lockout.getBoard().getGoals()) {
            if (goal == null) continue;
            if (goal.isCompleted()) continue;

            if (goal instanceof LootAbandonedCampGoal) {
                lockout.completeGoal(goal, player);
            }
        }
    }

}
