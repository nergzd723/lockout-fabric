package me.marin.lockout.lockout.goals.experience;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.ReachXPLevelGoal;
import net.minecraft.resources.Identifier;

public class ReachXPLevel20Goal extends ReachXPLevelGoal {

    public ReachXPLevel20Goal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Reach XP level 20";
    }

    @Override
    public int getAmount() {
        return 20;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/experience/level_20.png");

    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
