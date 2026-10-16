package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.ObtainItemsGoal;
import me.marin.lockout.mixin.server.PlayerInventoryAccessor;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class ObtainStackOf64Goal extends ObtainItemsGoal {

    private static final int STACK_SIZE = 64;
    private static final List<Item> DISPLAY_ITEMS = List.of(Items.COBBLESTONE, Items.DIRT, Items.OAK_LOG, Items.SAND);

    public ObtainStackOf64Goal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Obtain a stack of any Item";
    }

    @Override
    public List<Item> getItems() {
        return DISPLAY_ITEMS;
    }

    /**
     * A single stack of 64 in the main inventory (including hotbar).
     */
    @Override
    public boolean satisfiedBy(Inventory playerInventory) {
        for (ItemStack stack : ((PlayerInventoryAccessor) playerInventory).getPlayerInventory()) {
            if (stack != null && stack.getCount() >= STACK_SIZE) {
                return true;
            }
        }
        return false;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(STACK_SIZE));
        return true;
    }

}
