package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.ObtainAllItemsGoal;
import me.marin.lockout.lockout.interfaces.RequiresAmount;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import java.util.List;

public class Obtain16GlowLichenGoal extends ObtainAllItemsGoal implements RequiresAmount {

    private static final List<Item> ITEMS = List.of(Items.GLOW_LICHEN);

    public Obtain16GlowLichenGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return "Obtain 16 Glow Lichen";
    }

    @Override
    public int getAmount() {
        return 16;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
