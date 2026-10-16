package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class LootAbandonedCampGoal extends Goal {

    public LootAbandonedCampGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Loot an Abandoned Camp";
    }

    private static final ItemStack ITEM_STACK = Items.BARREL.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
