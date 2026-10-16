package me.marin.lockout.lockout.goals.craft;

import me.marin.lockout.lockout.interfaces.CraftUniqueItemsGoal;

public class Craft100UniqueItemsGoal extends CraftUniqueItemsGoal {

    public Craft100UniqueItemsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Craft 100 Unique Items";
    }

    @Override
    public int getAmount() {
        return 100;
    }

}
