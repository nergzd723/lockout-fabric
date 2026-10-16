package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;

public class UseGlowInkWarpedGoal extends Goal implements TextureProvider {

    public UseGlowInkWarpedGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Use Glow Ink on Warped Sign";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    private static final Identifier TEXTURE = Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/glow_warped_sign.png");
    @Override
    public Identifier getTextureIdentifier() {
        return TEXTURE;
    }

}
