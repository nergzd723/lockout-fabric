package me.marin.lockout.lockout.goals.leash;

import me.marin.lockout.lockout.interfaces.LeashUniqueMobsGoal;

public class Leash4UniqueMobsGoal extends LeashUniqueMobsGoal {

    public Leash4UniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Leash 4 unique Mobs at once";
    }

    @Override
    public int getAmount() {
        return 4;
    }

}
