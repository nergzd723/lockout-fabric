package me.marin.lockout.lockout.goals.break_item;

import me.marin.lockout.lockout.interfaces.BreakItemGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class BreakShieldGoal extends BreakItemGoal {

    public BreakShieldGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Break Shield";
    }

    @Override
    public boolean isBrokenItem(Item item) {
        return item == Items.SHIELD;
    }

    private static final ItemStack ITEM_STACK = Items.SHIELD.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
