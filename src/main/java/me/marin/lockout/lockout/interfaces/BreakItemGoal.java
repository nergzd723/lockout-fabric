package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.lockout.Goal;
import net.minecraft.world.item.Item;

/**
 * Completed when a player breaks an item through durability damage.
 */
public abstract class BreakItemGoal extends Goal {

    public BreakItemGoal(String id, String data) {
        super(id, data);
    }

    public abstract boolean isBrokenItem(Item item);

}
