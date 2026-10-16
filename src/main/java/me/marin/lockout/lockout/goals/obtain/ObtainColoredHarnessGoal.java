package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.lockout.goals.util.GoalDataConstants;
import me.marin.lockout.lockout.interfaces.ObtainAllItemsGoal;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class ObtainColoredHarnessGoal extends ObtainAllItemsGoal {

    private final List<Item> ITEMS;
    private final String GOAL_NAME;

    public ObtainColoredHarnessGoal(String id, String data) {
        super(id, data);
        DyeColor DYE_COLOR = GoalDataConstants.getDyeColor(data);
        GOAL_NAME = "Obtain " + GoalDataConstants.getDyeColorFormatted(DYE_COLOR) + " Harness";
        ITEMS = List.of(Items.HARNESS.pick(DYE_COLOR));
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return GOAL_NAME;
    }

}
