package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.SpyMobGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;

public class SpyEndermanGoal extends SpyMobGoal {

    public SpyEndermanGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on Enderman";
    }

    @Override
    public EntityType<?> getEntityType() {
        return EntityTypes.ENDERMAN;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/spy/spy_enderman.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
