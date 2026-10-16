package me.marin.lockout.mixin.client;

import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutConfig;
import me.marin.lockout.client.LockoutClient;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

import static me.marin.lockout.Constants.GUI_PADDING;
import static me.marin.lockout.Constants.GUI_SLOT_SIZE;

@Mixin(Hud.class)
public abstract class InGameHudMixin {

    // If lockout board is visible, render effects to the left of it.
    @Redirect(method = "extractEffects", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;guiWidth()I"))
    private int renderStatusEffects(GuiGraphicsExtractor instance) {
        int width = instance.guiWidth();

        if (!Lockout.exists(LockoutClient.lockout)) {
            return width;
        }

        if (LockoutConfig.getInstance().boardPosition != LockoutConfig.BoardPosition.RIGHT) {
            return width;
        }

        return width - 2 * GUI_PADDING - LockoutClient.lockout.getBoard().size() * GUI_SLOT_SIZE;
    }

}
