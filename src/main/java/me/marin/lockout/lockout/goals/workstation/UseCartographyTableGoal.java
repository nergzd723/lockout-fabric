package me.marin.lockout.lockout.goals.workstation;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class UseCartographyTableGoal extends Goal {

    public UseCartographyTableGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Use Cartography Table";
    }

    private static final ItemStack ITEM_STACK = Items.CARTOGRAPHY_TABLE.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
