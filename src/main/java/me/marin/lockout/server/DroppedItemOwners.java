package me.marin.lockout.server;

import net.minecraft.world.entity.item.ItemEntity;

import java.util.Map;
import java.util.UUID;
import java.util.WeakHashMap;

/**
 * Which player dropped an item, including items dropped on death (vanilla only records a thrower for items
 * thrown on purpose). Weak keys, so picked up and despawned items are forgotten.
 */
public class DroppedItemOwners {

    private static final Map<ItemEntity, UUID> OWNERS = new WeakHashMap<>();

    public static void record(ItemEntity item, UUID playerId) {
        OWNERS.put(item, playerId);
    }

    public static UUID get(ItemEntity item) {
        UUID owner = OWNERS.get(item);
        if (owner == null && item.getOwner() != null) {
            owner = item.getOwner().getUUID();
        }
        return owner;
    }

}
