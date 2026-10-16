package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.lockout.interfaces.SpyUniqueMobsGoal;

public class Spy10UniqueMobsGoal extends SpyUniqueMobsGoal {

    public Spy10UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on 10 Unique Mobs";
    }

    @Override
    public int getAmount() {
        return 10;
    }

}
