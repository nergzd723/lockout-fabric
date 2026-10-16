package me.marin.lockout.lockout.goals.mine;

import me.marin.lockout.lockout.interfaces.MineBlockGoal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class MineObsidianGoal extends MineBlockGoal {

    private static final List<Item> ITEMS = List.of(Items.OBSIDIAN);
    private static final Item ITEM = Items.OBSIDIAN;

    public MineObsidianGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Mine Obsidian";
    }

    @Override
    public ItemStack getTextureItemStack() {
        return ITEM.getDefaultInstance();
    }

    @Override
    public List<Item> getItems() {
        return ITEMS;
    }

}
