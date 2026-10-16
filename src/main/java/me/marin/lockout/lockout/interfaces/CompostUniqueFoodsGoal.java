package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.texture.CustomTextureRenderer;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.LinkedHashSet;
import java.util.Map;

public abstract class CompostUniqueFoodsGoal extends UniqueTrackedGoal<Item> implements CustomTextureRenderer {

    private static final ItemStack ITEM_STACK = Items.COMPOSTER.getDefaultInstance();

    public CompostUniqueFoodsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Item>> getTrackerMap() {
        return LockoutServer.lockout.compostedFoods;
    }

    @Override
    public String getProgressLabel() {
        return "Unique Foods composted";
    }

    @Override
    public String describe(Item value) {
        return Utility.itemName(value);
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
