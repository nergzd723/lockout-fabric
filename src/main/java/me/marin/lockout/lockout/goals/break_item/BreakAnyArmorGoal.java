package me.marin.lockout.lockout.goals.break_item;

import me.marin.lockout.lockout.interfaces.BreakItemGoal;
import net.minecraft.tags.ItemTags;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class BreakAnyArmorGoal extends BreakItemGoal implements CycleItemTexturesProvider {

    public BreakAnyArmorGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Break any Armor Piece";
    }

    @Override
    public boolean isBrokenItem(Item item) {
        return item.builtInRegistryHolder().is(ItemTags.HEAD_ARMOR) || item.builtInRegistryHolder().is(ItemTags.CHEST_ARMOR)
                || item.builtInRegistryHolder().is(ItemTags.LEG_ARMOR) || item.builtInRegistryHolder().is(ItemTags.FOOT_ARMOR);
    }

    private static final List<Item> DISPLAY_ITEMS = List.of(Items.LEATHER_HELMET, Items.IRON_CHESTPLATE, Items.GOLDEN_LEGGINGS, Items.CHAINMAIL_BOOTS);
    @Override
    public List<Item> getItemsToDisplay() {
        return DISPLAY_ITEMS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
