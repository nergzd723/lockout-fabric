package me.marin.lockout.lockout.goals.death;

import me.marin.lockout.Constants;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.Utility;
import me.marin.lockout.lockout.interfaces.UniqueTrackedGoal;
import me.marin.lockout.lockout.texture.CycleTexturesProvider;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemStack;
import org.apache.commons.lang3.text.WordUtils;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

/**
 * The team's deaths cover this many different damage types.
 */
public class Die5UniqueWaysGoal extends UniqueTrackedGoal<Identifier> implements CycleTexturesProvider {

    private static final List<Identifier> TEXTURES = List.of(
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/death/die_to_cactus.png"),
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/death/die_to_drowning.png"),
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/death/die_to_anvil.png"),
            Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/death/die_to_freezing.png")
    );

    public Die5UniqueWaysGoal(String id, String data) {
        super(id, data);
    }

    @Override
    public String getGoalName() {
        return "Die in 5 different ways";
    }

    @Override
    public int getAmount() {
        return 5;
    }

    @Override
    public Map<LockoutTeam, LinkedHashSet<Identifier>> getTrackerMap() {
        return LockoutServer.lockout.deathTypes;
    }

    @Override
    public String getProgressLabel() {
        return "Ways of dying";
    }

    @Override
    public String describe(Identifier value) {
        return WordUtils.capitalize(value.getPath().replace('_', ' '));
    }

    @Override
    public List<Identifier> getTexturesToDisplay() {
        return TEXTURES;
    }

    @Override
    public ItemStack getTextureItemStack() {
        return null;
    }

    @Override
    public boolean renderTexture(GuiGraphicsExtractor context, int x, int y, int tick) {
        CycleTexturesProvider.super.renderTexture(context, x, y, tick);
        Utility.drawStackCount(context, x, y, String.valueOf(getAmount()));
        return true;
    }

}
