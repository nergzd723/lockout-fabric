package me.marin.lockout.lockout.goals.wear_armor;

import me.marin.lockout.lockout.interfaces.ObtainItemsGoal;
import me.marin.lockout.mixin.server.PlayerInventoryAccessor;
import net.minecraft.core.component.DataComponents;
import net.minecraft.resources.ResourceKey;
import net.minecraft.tags.ItemTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.equipment.EquipmentAsset;
import net.minecraft.world.item.equipment.Equippable;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

public class WearUniqueArmorGoal extends ObtainItemsGoal {

    private static final Map<EquipmentSlot, TagKey<Item>> ARMOR_SLOTS = Map.of(
            EquipmentSlot.HEAD, ItemTags.HEAD_ARMOR,
            EquipmentSlot.CHEST, ItemTags.CHEST_ARMOR,
            EquipmentSlot.LEGS, ItemTags.LEG_ARMOR,
            EquipmentSlot.FEET, ItemTags.FOOT_ARMOR
    );
    private static final List<Item> DISPLAY_ITEMS = List.of(Items.LEATHER_HELMET, Items.IRON_CHESTPLATE, Items.GOLDEN_LEGGINGS, Items.DIAMOND_BOOTS);

    public WearUniqueArmorGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Wear Full Armor with different materials";
    }

    @Override
    public List<Item> getItems() {
        return DISPLAY_ITEMS;
    }

    /**
     * All 4 armor slots hold armor, and no two pieces share a material.
     */
    @Override
    public boolean satisfiedBy(Inventory playerInventory) {
        Set<ResourceKey<EquipmentAsset>> materials = new HashSet<>();
        for (var entry : ARMOR_SLOTS.entrySet()) {
            ItemStack stack = ((PlayerInventoryAccessor) playerInventory).getEquipment().get(entry.getKey());
            if (stack == null || stack.isEmpty() || !stack.is(entry.getValue())) return false;

            Equippable equippable = stack.get(DataComponents.EQUIPPABLE);
            if (equippable == null || equippable.assetId().isEmpty()) return false;
            materials.add(equippable.assetId().get());
        }
        return materials.size() == ARMOR_SLOTS.size();
    }

}
