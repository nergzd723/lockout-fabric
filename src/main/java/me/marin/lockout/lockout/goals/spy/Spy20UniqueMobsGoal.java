package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.lockout.interfaces.SpyUniqueMobsGoal;

public class Spy20UniqueMobsGoal extends SpyUniqueMobsGoal {

    public Spy20UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on 20 Unique Mobs";
    }

    @Override
    public int getAmount() {
        return 20;
    }

}
