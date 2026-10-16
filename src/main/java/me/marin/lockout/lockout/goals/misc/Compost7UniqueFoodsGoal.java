package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.CompostUniqueFoodsGoal;

public class Compost7UniqueFoodsGoal extends CompostUniqueFoodsGoal {

    public Compost7UniqueFoodsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Compost 7 Unique Foods";
    }

    @Override
    public int getAmount() {
        return 7;
    }

}
