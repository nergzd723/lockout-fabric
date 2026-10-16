package me.marin.lockout.lockout.goals.biome;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.lockout.interfaces.UniqueTrackedGoal;
import me.marin.lockout.lockout.texture.CycleItemTexturesProvider;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import org.apache.commons.lang3.text.WordUtils;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

public class VisitAllCaveBiomesGoal extends UniqueTrackedGoal<Identifier> implements CycleItemTexturesProvider {

    public static final List<Identifier> CAVE_BIOMES = List.of(
            Identifier.fromNamespaceAndPath("minecraft", "lush_caves"),
            Identifier.fromNamespaceAndPath("minecraft", "dripstone_caves"),
            Identifier.fromNamespaceAndPath("minecraft", "deep_dark")
    );
    private static final List<Item> DISPLAY_ITEMS = List.of(Items.MOSS_BLOCK, Items.POINTED_DRIPSTONE, Items.SCULK);

    public VisitAllCaveBiomesGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Visit all Cave Biomes";
    }

    @Override
    public int getAmount() {
        return CAVE_BIOMES.size();
    }

    @Override
    public boolean counts(Identifier value) {
        return CAVE_BIOMES.contains(value);
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Identifier>> getTrackerMap() {
        return LockoutServer.lockout.biomesVisited;
    }

    @Override
    public String getProgressLabel() {
        return "Cave Biomes visited";
    }

    @Override
    public String describe(Identifier value) {
        return WordUtils.capitalize(value.getPath().replace('_', ' '));
    }

    @Override
    public List<Item> getItemsToDisplay() {
        return DISPLAY_ITEMS;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

}
