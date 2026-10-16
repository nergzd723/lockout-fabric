package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.lockout.interfaces.ObtainSomeOfTheItemsGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class ObtainCopperChestGoal extends ObtainSomeOfTheItemsGoal {

    private static final List<Item> ITEMS = Items.COPPER_CHEST.asList();

    public ObtainCopperChestGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public int getAmount() {
        return 1;
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return "Obtain Copper Chest";
    }

}
