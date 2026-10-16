package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.CompostUniqueFoodsGoal;

public class Compost3UniqueFoodsGoal extends CompostUniqueFoodsGoal {

    public Compost3UniqueFoodsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Compost 3 Unique Foods";
    }

    @Override
    public int getAmount() {
        return 3;
    }

}
