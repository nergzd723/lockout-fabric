package me.marin.lockout.lockout.goals.biome;

import me.marin.lockout.lockout.interfaces.VisitUniqueBiomesGoal;

public class Visit15UniqueBiomesGoal extends VisitUniqueBiomesGoal {

    public Visit15UniqueBiomesGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Visit 15 unique Biomes";
    }

    @Override
    public int getAmount() {
        return 15;
    }

}
