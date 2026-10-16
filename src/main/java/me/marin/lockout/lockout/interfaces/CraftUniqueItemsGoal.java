package me.marin.lockout.lockout.interfaces;

import net.minecraft.network.chat.CommonComponents;
import net.minecraft.core.component.DataComponents;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.texture.CustomTextureRenderer;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

public abstract class CraftUniqueItemsGoal extends Goal implements RequiresAmount, Trackable<LockoutTeam, LinkedHashSet<Item>>, CustomTextureRenderer, HasTooltipInfo {

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
    public List<String> getTooltip(LockoutTeam team, Player player) {
        List<String> tooltip = new ArrayList<>();
        var crafts = getTrackerMap().getOrDefault(team, new LinkedHashSet<>());

        tooltip.add(" ");
        tooltip.add("Unique Items crafted: " + crafts.size() + "/" + getAmount());
        tooltip.addAll(HasTooltipInfo.commaSeparatedList(crafts.stream().map(item -> item.components().getOrDefault(DataComponents.ITEM_NAME, CommonComponents.EMPTY).getString()).toList()));
        tooltip.add(" ");

        return tooltip;
    }

    @Override
    public List<String> getSpectatorTooltip() {
        List<String> tooltip = new ArrayList<>();

        tooltip.add(" ");
        for (LockoutTeam team : LockoutServer.lockout.getTeams()) {
            var crafts = getTrackerMap().getOrDefault(team, new LinkedHashSet<>());
            tooltip.add(team.getColor() + team.getDisplayName() + ChatFormatting.RESET + ": " + crafts.size() + "/" + getAmount());
        }
        tooltip.add(" ");

        return tooltip;
    }

}
