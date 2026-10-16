package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.texture.TextureProvider;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.item.ItemStack;
import java.util.List;

/**
 * Completed when a player uses a Name Tag with one of the names on the entity.
 */
public abstract class NameEntityGoal extends Goal implements TextureProvider {

    public NameEntityGoal(String id, String data) {
        super(id, data);
    }

    public abstract EntityType<?> getEntityType();

    public abstract List<String> getNames();

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
