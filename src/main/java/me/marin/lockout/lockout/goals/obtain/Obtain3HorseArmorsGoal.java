package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.Utility;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import me.marin.lockout.lockout.interfaces.ObtainSomeOfTheItemsGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class Obtain3HorseArmorsGoal extends ObtainSomeOfTheItemsGoal {

    private static final List<Item> ITEMS = List.of(
            Items.LEATHER_HORSE_ARMOR, Items.COPPER_HORSE_ARMOR, Items.IRON_HORSE_ARMOR,
            Items.GOLDEN_HORSE_ARMOR, Items.DIAMOND_HORSE_ARMOR, Items.NETHERITE_HORSE_ARMOR
    );

    public Obtain3HorseArmorsGoal(String id, String data) {
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
        return "Obtain 3 types of Horse Armor";
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
