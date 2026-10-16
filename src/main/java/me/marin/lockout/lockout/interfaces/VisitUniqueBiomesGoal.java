package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.texture.CustomTextureRenderer;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import org.apache.commons.lang3.text.WordUtils;
import java.util.LinkedHashSet;
import java.util.Map;

/**
 * Counts the Overworld biomes the team has visited (the biomes of the "Adventuring Time" advancement).
 */
public abstract class VisitUniqueBiomesGoal extends UniqueTrackedGoal<Identifier> implements CustomTextureRenderer {

    private static final ItemStack ITEM_STACK = Items.COMPASS.getDefaultInstance();

    public VisitUniqueBiomesGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Identifier>> getTrackerMap() {
        return LockoutServer.lockout.biomesVisited;
    }

    @Override
    public String getProgressLabel() {
        return "Biomes visited";
    }

    @Override
    public String describe(Identifier value) {
        return WordUtils.capitalize(value.getPath().replace('_', ' '));
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
