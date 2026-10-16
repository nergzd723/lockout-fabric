package me.marin.lockout.lockout.goals.mine;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.UniqueTrackedGoal;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

public class Mine3TypesOfIceGoal extends UniqueTrackedGoal<Block> implements CycleItemTexturesProvider {

    // Frosted Ice has no item, so this tracks blocks.
    public static final List<Block> ICE = List.of(Blocks.ICE, Blocks.PACKED_ICE, Blocks.BLUE_ICE, Blocks.FROSTED_ICE);
    private static final List<Item> DISPLAY_ITEMS = List.of(Items.ICE, Items.PACKED_ICE, Items.BLUE_ICE);

    public Mine3TypesOfIceGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Mine 3 types of Ice";
    }

    @Override
    public int getAmount() {
        return 3;
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Block>> getTrackerMap() {
        return LockoutServer.lockout.iceTypesMined;
    }

    @Override
    public String getProgressLabel() {
        return "Ice types mined";
    }

    @Override
    public String describe(Block value) {
        return value.getName().getString();
    }

    @Override
    public List<Item> getItemsToDisplay() {
        return DISPLAY_ITEMS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        CycleItemTexturesProvider.super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
