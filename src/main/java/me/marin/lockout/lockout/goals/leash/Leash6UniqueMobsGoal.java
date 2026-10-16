package me.marin.lockout.lockout.goals.leash;

import me.marin.lockout.lockout.interfaces.LeashUniqueMobsGoal;

public class Leash6UniqueMobsGoal extends LeashUniqueMobsGoal {

    public Leash6UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Leash 6 unique Mobs at once";
    }

    @Override
    public int getAmount() {
        return 6;
    }

}
