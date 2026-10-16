package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.lockout.interfaces.SpyUniqueMobsGoal;

public class Spy25UniqueMobsGoal extends SpyUniqueMobsGoal {

    public Spy25UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on 25 Unique Mobs";
    }

    @Override
    public int getAmount() {
        return 25;
    }

}
