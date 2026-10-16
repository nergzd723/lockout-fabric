package me.marin.lockout.lockout.goals.biome;

import me.marin.lockout.lockout.interfaces.VisitUniqueBiomesGoal;

public class Visit10UniqueBiomesGoal extends VisitUniqueBiomesGoal {

    public Visit10UniqueBiomesGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Visit 10 unique Biomes";
    }

    @Override
    public int getAmount() {
        return 10;
    }

}
