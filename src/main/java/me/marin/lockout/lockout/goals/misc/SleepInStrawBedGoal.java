package me.marin.lockout.lockout.goals.misc;

import me.marin.lockout.lockout.interfaces.IncrementStatGoal;
import net.minecraft.resources.Identifier;
import net.minecraft.stats.Stats;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import java.util.List;

public class SleepInStrawBedGoal extends IncrementStatGoal {

    public SleepInStrawBedGoal(String id, String data) {
        super(id, data);
    }

    private static final List<Identifier> STATS = List.of(Stats.SLEEP_IN_STRAW_BED);
    @Override
    public List<Identifier> getStats() {
        return STATS;
    }

    @Override
    public String getGoalName() {
        return "Sleep in a Straw Bed";
    }

    private static final ItemStack ITEM_STACK = Items.STRAW_BED.getDefaultInstance();
    @Override
    public ItemStack getTextureItemStack() {
        return ITEM_STACK;
    }

}
