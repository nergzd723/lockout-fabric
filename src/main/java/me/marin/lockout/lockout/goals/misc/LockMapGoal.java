package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class LockMapGoal extends Goal {

    public LockMapGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Lock a Map using Cartography Table";
    }

    private static final ItemStack ITEM_STACK = Items.FILLED_MAP.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
