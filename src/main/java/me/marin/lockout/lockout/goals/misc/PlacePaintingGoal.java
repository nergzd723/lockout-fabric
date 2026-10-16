package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.PlaceItemGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

public class PlacePaintingGoal extends PlaceItemGoal {

    public PlacePaintingGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Place Painting";
    }

    @Override
    public boolean isPlacedItem(Item item) {
        return item == Items.PAINTING;
    }

    private static final ItemStack ITEM_STACK = Items.PAINTING.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
