package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class UseGoldenDandelionGoal extends Goal {

    public UseGoldenDandelionGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Feed Golden Dandelion to any Baby Mob";
    }

    private static final ItemStack ITEM_STACK = Items.GOLDEN_DANDELION.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
