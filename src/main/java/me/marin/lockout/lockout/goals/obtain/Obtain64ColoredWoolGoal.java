package me.marin.lockout.lockout.goals.obtain;

import me.marin.lockout.lockout.goals.util.GoalDataConstants;
import me.marin.lockout.lockout.interfaces.ObtainAllItemsGoal;
import me.marin.lockout.lockout.interfaces.RequiresAmount;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class Obtain64ColoredWoolGoal extends ObtainAllItemsGoal implements RequiresAmount {

    private final ItemStack ITEM_STACK;
    private final List<Item> ITEMS;

    private final String GOAL_NAME;

    public Obtain64ColoredWoolGoal(String id, String data) {
        super(id, data);
        DyeColor DYE_COLOR = GoalDataConstants.getDyeColor(data);

        GOAL_NAME = "Obtain 64 " + GoalDataConstants.getDyeColorFormatted(DYE_COLOR) + " Wool";
        ITEMS = List.of(getWoolColor(data));
        ITEM_STACK = getWoolColor(data).getDefaultInstance();
        ITEM_STACK.setCount(64);
    }

    @Override
    public int getAmount() {
        return 64;
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

    @Override
    public String getGoalName() {
        return GOAL_NAME;
    }

    public static Item getWoolColor(String colorString) {
        return switch (colorString) {
            default -> null;
            case "white" -> Items.WOOL.pick(DyeColor.WHITE);
            case "orange" -> Items.WOOL.pick(DyeColor.ORANGE);
            case "magenta" -> Items.WOOL.pick(DyeColor.MAGENTA);
            case "light_blue" -> Items.WOOL.pick(DyeColor.LIGHT_BLUE);
            case "yellow" -> Items.WOOL.pick(DyeColor.YELLOW);
            case "lime" -> Items.WOOL.pick(DyeColor.LIME);
            case "pink" -> Items.WOOL.pick(DyeColor.PINK);
            case "gray" -> Items.WOOL.pick(DyeColor.GRAY);
            case "light_gray" -> Items.WOOL.pick(DyeColor.LIGHT_GRAY);
            case "cyan" -> Items.WOOL.pick(DyeColor.CYAN);
            case "purple" -> Items.WOOL.pick(DyeColor.PURPLE);
            case "blue" -> Items.WOOL.pick(DyeColor.BLUE);
            case "brown" -> Items.WOOL.pick(DyeColor.BROWN);
            case "green" -> Items.WOOL.pick(DyeColor.GREEN);
            case "red" -> Items.WOOL.pick(DyeColor.RED);
            case "black" -> Items.WOOL.pick(DyeColor.BLACK);
        };
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        context.itemDecorations(Minecraft.getInstance().font, ITEM_STACK, x, y);
        return true;
    }

}

