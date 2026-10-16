package me.marin.lockout.mixin.server.network;

import me.marin.lockout.LockoutInitializer;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.PacketUtils;
import net.minecraft.network.protocol.common.ServerboundPongPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.ServerCommonPacketListenerImpl;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ServerCommonPacketListenerImpl.class)
public abstract class ServerCommonNetworkHandlerMixin {

    @Shadow @Final protected MinecraftServer server;

    @Inject(method = "handlePong", at = @At("HEAD"))
    public void onPong(ServerboundPongPacket packet, CallbackInfo ci) {
        if (!((Object) this instanceof ServerGamePacketListenerImpl gameListener)) return;

        // Handle the pong through the server's packet queue (like the Lockout version payload),
        // so a version payload sent before the pong is always handled first.
        PacketUtils.ensureRunningOnSameThread(packet, gameListener, server.packetProcessor());

        ServerPlayer player = gameListener.getPlayer();
        int id = packet.getId();

        if (LockoutServer.waitingForVersionPacketPlayersMap.containsKey(player)) {
            if (LockoutServer.waitingForVersionPacketPlayersMap.get(player) == id) {
                player.connection.disconnect(Component.nullToEmpty("Missing Lockout mod.\nServer is using Lockout v" + LockoutInitializer.MOD_VERSION.getFriendlyString() + "."));
            }
        }
    }

}
