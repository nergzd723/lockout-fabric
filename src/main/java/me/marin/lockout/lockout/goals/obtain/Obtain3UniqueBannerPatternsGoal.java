package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.Utility;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import me.marin.lockout.lockout.interfaces.ObtainSomeOfTheItemsGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class Obtain3UniqueBannerPatternsGoal extends ObtainSomeOfTheItemsGoal {

    private static final List<Item> ITEMS = List.of(
            Items.FLOWER_BANNER_PATTERN, Items.CREEPER_BANNER_PATTERN, Items.SKULL_BANNER_PATTERN, Items.MOJANG_BANNER_PATTERN,
            Items.GLOBE_BANNER_PATTERN, Items.PIGLIN_BANNER_PATTERN, Items.FLOW_BANNER_PATTERN, Items.GUSTER_BANNER_PATTERN,
            Items.FIELD_MASONED_BANNER_PATTERN, Items.BORDURE_INDENTED_BANNER_PATTERN
    );

    public Obtain3UniqueBannerPatternsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public int getAmount() {
        return 3;
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return "Obtain 3 Unique Banner Patterns";
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
