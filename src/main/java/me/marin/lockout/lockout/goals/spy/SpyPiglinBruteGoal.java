package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.SpyMobGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;

public class SpyPiglinBruteGoal extends SpyMobGoal {

    public SpyPiglinBruteGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on Piglin Brute";
    }

    @Override
    public EntityType<?> getEntityType() {
        return EntityTypes.PIGLIN_BRUTE;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/spy/spy_piglin_brute.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
