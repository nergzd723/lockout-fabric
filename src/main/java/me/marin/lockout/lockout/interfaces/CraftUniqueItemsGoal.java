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

public abstract class CraftUniqueItemsGoal extends UniqueTrackedGoal<Item> implements CustomTextureRenderer {

    private static final ItemStack ITEM_STACK = Items.CRAFTING_TABLE.getDefaultInstance();

    public CraftUniqueItemsGoal(String id, String data) {
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

    @Override
    public Map<LockoutTeam, LinkedHashSet<Item>> getTrackerMap() {
        return LockoutServer.lockout.uniqueItemsCraftedByTeam;
    }

    @Override
    public String getProgressLabel() {
        return "Unique Items crafted";
    }

    @Override
    public String describe(Item item) {
        return Utility.itemName(item);
    }

}
