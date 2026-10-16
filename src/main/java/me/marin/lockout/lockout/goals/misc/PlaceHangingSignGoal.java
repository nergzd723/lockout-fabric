package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.PlaceItemGoal;
import net.minecraft.tags.ItemTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class PlaceHangingSignGoal extends PlaceItemGoal {

    public PlaceHangingSignGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Place a Hanging Sign";
    }

    @Override
    public boolean isPlacedItem(Item item) {
        return item.builtInRegistryHolder().is(ItemTags.HANGING_SIGNS);
    }

    private static final ItemStack ITEM_STACK = Items.OAK_HANGING_SIGN.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
