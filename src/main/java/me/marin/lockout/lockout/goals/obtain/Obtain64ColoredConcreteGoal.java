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

public class Obtain64ColoredConcreteGoal extends ObtainAllItemsGoal implements RequiresAmount {

    private final ItemStack ITEM_STACK;
    private final List<Item> ITEMS;

    private final String GOAL_NAME;

    public Obtain64ColoredConcreteGoal(String id, String data) {
        super(id, data);
        DyeColor DYE_COLOR = GoalDataConstants.getDyeColor(data);

        GOAL_NAME = "Obtain 64 " + GoalDataConstants.getDyeColorFormatted(DYE_COLOR) + " Concrete";
        ITEMS = List.of(getConcreteColor(data));
        ITEM_STACK = getConcreteColor(data).getDefaultInstance();
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

    public static Item getConcreteColor(String colorString) {
        return switch (colorString) {
            default -> null;
            case "white" -> Items.CONCRETE.pick(DyeColor.WHITE);
            case "orange" -> Items.CONCRETE.pick(DyeColor.ORANGE);
            case "magenta" -> Items.CONCRETE.pick(DyeColor.MAGENTA);
            case "light_blue" -> Items.CONCRETE.pick(DyeColor.LIGHT_BLUE);
            case "yellow" -> Items.CONCRETE.pick(DyeColor.YELLOW);
            case "lime" -> Items.CONCRETE.pick(DyeColor.LIME);
            case "pink" -> Items.CONCRETE.pick(DyeColor.PINK);
            case "gray" -> Items.CONCRETE.pick(DyeColor.GRAY);
            case "light_gray" -> Items.CONCRETE.pick(DyeColor.LIGHT_GRAY);
            case "cyan" -> Items.CONCRETE.pick(DyeColor.CYAN);
            case "purple" -> Items.CONCRETE.pick(DyeColor.PURPLE);
            case "blue" -> Items.CONCRETE.pick(DyeColor.BLUE);
            case "brown" -> Items.CONCRETE.pick(DyeColor.BROWN);
            case "green" -> Items.CONCRETE.pick(DyeColor.GREEN);
            case "red" -> Items.CONCRETE.pick(DyeColor.RED);
            case "black" -> Items.CONCRETE.pick(DyeColor.BLACK);
        };
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        super.renderTexture(context, x, y, tick);
        context.itemDecorations(Minecraft.getInstance().font, ITEM_STACK, x, y);
        return true;
    }

}

