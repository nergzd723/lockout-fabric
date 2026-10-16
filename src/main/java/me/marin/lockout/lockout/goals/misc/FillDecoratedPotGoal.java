package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class FillDecoratedPotGoal extends Goal {

    public FillDecoratedPotGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Fill a Decorated Pot";
    }

    private static final ItemStack ITEM_STACK = Items.DECORATED_POT.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
