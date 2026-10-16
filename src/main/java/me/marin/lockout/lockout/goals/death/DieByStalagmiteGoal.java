package me.marin.lockout.lockout.goals.death;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.interfaces.DieToDamageTypeGoal;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.world.damagesource.DamageType;
import net.minecraft.world.damagesource.DamageTypes;
import java.util.List;

public class DieByStalagmiteGoal extends DieToDamageTypeGoal {

    public DieByStalagmiteGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Die on a Stalagmite";
    }

    @Override
    public List<ResourceKey<DamageType>> getDamageRegistryKeys() {
        return List.of(DamageTypes.STALAGMITE);
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/death/die_to_stalagmite.png");

    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
