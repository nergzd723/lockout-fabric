package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class ConstructCopperGolemGoal extends Goal {

    public ConstructCopperGolemGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Construct a Copper Golem";
    }

    private static final ItemStack ITEM_STACK = Items.COPPER_GOLEM_SPAWN_EGG.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
