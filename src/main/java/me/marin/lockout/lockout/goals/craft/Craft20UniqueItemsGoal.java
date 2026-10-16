package me.marin.lockout.lockout.goals.craft;

import me.marin.lockout.lockout.interfaces.CraftUniqueItemsGoal;

public class Craft20UniqueItemsGoal extends CraftUniqueItemsGoal {

    public Craft20UniqueItemsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Craft 20 Unique Items";
    }

    @Override
    public int getAmount() {
        return 20;
    }

}
