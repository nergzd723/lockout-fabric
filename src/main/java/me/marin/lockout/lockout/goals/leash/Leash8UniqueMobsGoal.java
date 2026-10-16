package me.marin.lockout.lockout.goals.leash;

import me.marin.lockout.lockout.interfaces.LeashUniqueMobsGoal;

public class Leash8UniqueMobsGoal extends LeashUniqueMobsGoal {

    public Leash8UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Leash 8 unique Mobs at once";
    }

    @Override
    public int getAmount() {
        return 8;
    }

}
