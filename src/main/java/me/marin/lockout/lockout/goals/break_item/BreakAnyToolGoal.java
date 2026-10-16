package me.marin.lockout.lockout.goals.break_item;

import me.marin.lockout.lockout.interfaces.BreakItemGoal;
import net.minecraft.tags.ItemTags;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class BreakAnyToolGoal extends BreakItemGoal implements CycleItemTexturesProvider {

    private static final List<Item> OTHER_TOOLS = List.of(Items.FISHING_ROD, Items.FLINT_AND_STEEL, Items.SHEARS, Items.BRUSH, Items.CARROT_ON_A_STICK, Items.WARPED_FUNGUS_ON_A_STICK);

    public BreakAnyToolGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Break any Tool";
    }

    @Override
    public boolean isBrokenItem(Item item) {
        return item.builtInRegistryHolder().is(ItemTags.AXES) || item.builtInRegistryHolder().is(ItemTags.HOES)
                || item.builtInRegistryHolder().is(ItemTags.PICKAXES) || item.builtInRegistryHolder().is(ItemTags.SWORDS)
                || item.builtInRegistryHolder().is(ItemTags.SHOVELS) || item.builtInRegistryHolder().is(ItemTags.SPEARS)
                || OTHER_TOOLS.contains(item);
    }

    private static final List<Item> DISPLAY_ITEMS = List.of(Items.WOODEN_PICKAXE, Items.STONE_AXE, Items.FISHING_ROD, Items.SHEARS, Items.FLINT_AND_STEEL);
    @Override
    public List<Item> getItemsToDisplay() {
        return DISPLAY_ITEMS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
