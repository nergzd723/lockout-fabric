package me.marin.lockout.lockout.goals.spy;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.SpyMobGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;

public class SpyIronGolemGoal extends SpyMobGoal {

    public SpyIronGolemGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Spy on Iron Golem";
    }

    @Override
    public EntityType<?> getEntityType() {
        return EntityTypes.IRON_GOLEM;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/spy/spy_iron_golem.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
