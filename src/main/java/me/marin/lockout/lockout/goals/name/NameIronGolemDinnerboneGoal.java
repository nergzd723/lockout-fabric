package me.marin.lockout.lockout.goals.name;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.NameEntityGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import java.util.List;

public class NameIronGolemDinnerboneGoal extends NameEntityGoal {

    private static final List<String> NAMES = List.of("Dinnerbone", "Grumm");

    public NameIronGolemDinnerboneGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Name an Iron Golem 'Dinnerbone'";
    }

    @Override
    public EntityType<?> getEntityType() {
        return EntityTypes.IRON_GOLEM;
    }

    @Override
    public List<String> getNames() {
        return NAMES;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/name/name_iron_golem.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
