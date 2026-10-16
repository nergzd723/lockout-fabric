package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.item.ItemStack;

/**
 * Completed when a player looks at the mob through a Spyglass.
 */
public abstract class SpyMobGoal extends Goal implements TextureProvider {

    public SpyMobGoal(String id, String data) {
        super(id, data);
    }

    public abstract EntityType<?> getEntityType();

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
