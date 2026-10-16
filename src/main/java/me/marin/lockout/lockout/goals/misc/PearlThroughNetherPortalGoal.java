package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import me.marin.lockout.lockout.Goal;

public class PearlThroughNetherPortalGoal extends Goal implements TextureProvider {

    public PearlThroughNetherPortalGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Teleport through Nether Portal using Ender Pearl";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/pearl_nether_portal.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
