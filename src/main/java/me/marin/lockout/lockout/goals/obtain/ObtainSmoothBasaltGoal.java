package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.lockout.interfaces.ObtainAllItemsGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class ObtainSmoothBasaltGoal extends ObtainAllItemsGoal {

    private static final List<Item> ITEMS = List.of(Items.SMOOTH_BASALT);

    public ObtainSmoothBasaltGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return "Obtain Smooth Basalt";
    }

}
