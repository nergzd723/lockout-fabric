package me.marin.lockout.lockout.goals.break_item;

import me.marin.lockout.lockout.interfaces.BreakItemGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class BreakFishingRodGoal extends BreakItemGoal {

    public BreakFishingRodGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Break Fishing Rod";
    }

    @Override
    public boolean isBrokenItem(Item item) {
        return item == Items.FISHING_ROD;
    }

    private static final ItemStack ITEM_STACK = Items.FISHING_ROD.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
