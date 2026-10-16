package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class PickUpOpponentItemGoal extends Goal {

    public PickUpOpponentItemGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Pick up an item dropped by an Opponent";
    }

    private static final ItemStack ITEM_STACK = Items.PLAYER_HEAD.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
