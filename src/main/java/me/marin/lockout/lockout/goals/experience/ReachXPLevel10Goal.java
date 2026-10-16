package me.marin.lockout.lockout.goals.experience;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.ReachXPLevelGoal;
import net.minecraft.resources.Identifier;

public class ReachXPLevel10Goal extends ReachXPLevelGoal {

    public ReachXPLevel10Goal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Reach XP level 10";
    }

    @Override
    public int getAmount() {
        return 10;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/experience/level_10.png");

    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
