package me.marin.lockout.lockout.goals.leash;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import me.marin.lockout.lockout.Goal;

public class LeashIronGolemGoal extends Goal implements TextureProvider {

    public LeashIronGolemGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Leash an Iron Golem";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/leash/leash_iron_golem.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
