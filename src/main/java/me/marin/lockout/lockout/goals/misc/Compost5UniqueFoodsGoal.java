package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.CompostUniqueFoodsGoal;

public class Compost5UniqueFoodsGoal extends CompostUniqueFoodsGoal {

    public Compost5UniqueFoodsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Compost 5 Unique Foods";
    }

    @Override
    public int getAmount() {
        return 5;
    }

}
