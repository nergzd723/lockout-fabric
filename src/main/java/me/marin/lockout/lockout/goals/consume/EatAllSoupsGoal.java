package me.marin.lockout.lockout.goals.consume;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.UniqueTrackedGoal;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

public class EatAllSoupsGoal extends UniqueTrackedGoal<Item> implements CycleItemTexturesProvider {

    private static final List<Item> SOUPS = List.of(Items.MUSHROOM_STEW, Items.RABBIT_STEW, Items.BEETROOT_SOUP, Items.SUSPICIOUS_STEW);

    public EatAllSoupsGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Eat every type of Soup";
    }

    @Override
    public int getAmount() {
        return SOUPS.size();
    }

    @Override
    public boolean counts(Item value) {
        return SOUPS.contains(value);
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Item>> getTrackerMap() {
        return LockoutServer.lockout.foodTypesEaten;
    }

    @Override
    public String getProgressLabel() {
        return "Soups eaten";
    }

    @Override
    public String describe(Item value) {
        return Utility.itemName(value);
    }

    @Override
    public List<Item> getItemsToDisplay() {
        return SOUPS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
