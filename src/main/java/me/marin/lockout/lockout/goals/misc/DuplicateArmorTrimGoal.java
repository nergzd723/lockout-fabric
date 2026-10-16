package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.goals.obtain.Obtain2UniqueArmorTrimsGoal;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class DuplicateArmorTrimGoal extends Goal implements CycleItemTexturesProvider {

    public DuplicateArmorTrimGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Duplicate any Armor Trim";
    }

    @Override
    public List<Item> getItemsToDisplay() {
        return Obtain2UniqueArmorTrimsGoal.ARMOR_TRIMS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
