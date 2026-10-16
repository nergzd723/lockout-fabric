package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.lockout.interfaces.SpyUniqueMobsGoal;

public class Spy15UniqueMobsGoal extends SpyUniqueMobsGoal {

    public Spy15UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on 15 Unique Mobs";
    }

    @Override
    public int getAmount() {
        return 15;
    }

}
