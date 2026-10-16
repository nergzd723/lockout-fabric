package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.Item;

/**
 * Completed when a player successfully uses (places) the item on a block.
 */
public abstract class PlaceItemGoal extends Goal {

    public PlaceItemGoal(String id, String data) {
        super(id, data);
    }

    public abstract boolean isPlacedItem(Item item);

}
