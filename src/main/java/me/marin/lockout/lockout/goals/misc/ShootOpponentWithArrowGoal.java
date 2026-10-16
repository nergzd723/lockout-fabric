package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class ShootOpponentWithArrowGoal extends Goal {

    public ShootOpponentWithArrowGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Shoot an Opponent with an Arrow";
    }

    private static final ItemStack ITEM_STACK = Items.BOW.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
