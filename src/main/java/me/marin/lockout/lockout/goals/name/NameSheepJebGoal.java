package me.marin.lockout.lockout.goals.name;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.NameEntityGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import java.util.List;

public class NameSheepJebGoal extends NameEntityGoal {

    private static final List<String> NAMES = List.of("jeb_");

    public NameSheepJebGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Name a Sheep 'jeb_'";
    }

    @Override
    public EntityType<?> getEntityType() {
        return EntityTypes.SHEEP;
    }

    @Override
    public List<String> getNames() {
        return NAMES;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/name/name_sheep.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
