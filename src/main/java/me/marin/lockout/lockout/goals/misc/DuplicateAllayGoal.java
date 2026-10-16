package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import me.marin.lockout.lockout.Goal;

public class DuplicateAllayGoal extends Goal implements TextureProvider {

    public DuplicateAllayGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Duplicate Allay";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/duplicate_allay.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
