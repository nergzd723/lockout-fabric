package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.ObtainItemsGoal;
import me.marin.lockout.lockout.interfaces.RequiresAmount;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class Obtain32ArrowsGoal extends ObtainItemsGoal implements RequiresAmount {

    private static final List<Item> ITEMS = List.of(Items.ARROW, Items.SPECTRAL_ARROW);

    public Obtain32ArrowsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return "Obtain 32 Arrows";
    }

    @Override
    public int getAmount() {
        return 32;
    }

    /**
     * Arrows and Spectral Arrows don't add up, 32 of either type is needed.
     */
    @Override
    public boolean satisfiedBy(Inventory playerInventory) {
        for (Item item : ITEMS) {
            if (playerInventory.countItem(item) >= getAmount()) {
                return true;
            }
        }
        return false;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
