package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.lockout.Goal;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.texture.CustomTextureRenderer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * Completed when a player has this many different mob types leashed to them at the same time.
 */
public abstract class LeashUniqueMobsGoal extends Goal implements RequiresAmount, CustomTextureRenderer {

    private static final ItemStack ITEM_STACK = Items.LEAD.getDefaultInstance();

    public LeashUniqueMobsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        context.item(ITEM_STACK, x, y);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
