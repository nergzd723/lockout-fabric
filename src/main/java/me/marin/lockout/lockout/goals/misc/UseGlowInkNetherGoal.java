package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.Constants;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.texture.CycleTexturesProvider;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import java.util.List;

public class UseGlowInkNetherGoal extends Goal implements CycleTexturesProvider {

    public UseGlowInkNetherGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Use Glow Ink on Crimson or Warped Sign";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    private static final List<Identifier> TEXTURES = List.of(
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/glow_crimson_sign.png"),
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/glow_warped_sign.png")
    );
    @Override
    public List<Identifier> getTexturesToDisplay() {
        return TEXTURES;
    }

}
