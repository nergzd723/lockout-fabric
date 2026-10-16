package me.marin.lockout.lockout.goals.craft;

import me.marin.lockout.lockout.interfaces.CraftUniqueItemsGoal;

public class Craft50UniqueItemsGoal extends CraftUniqueItemsGoal {

    public Craft50UniqueItemsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Craft 50 Unique Items";
    }

    @Override
    public int getAmount() {
        return 50;
    }

}
