package me.marin.lockout.network;

import me.marin.lockout.Constants;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

/**
 * Sent by a drafting player: pick one of the offered goals, or reroll the offer.
 *
 * @param action index of the offered goal to pick, or {@link #REROLL}
 */
public record DraftActionPayload(int action) implements CustomPacketPayload {

    public static final int REROLL = -1;

    public static final Type<DraftActionPayload> ID = new Type<>(Constants.DRAFT_ACTION_PACKET);
    public static final StreamCodec<RegistryFriendlyByteBuf, DraftActionPayload> CODEC = StreamCodec.composite(ByteBufCodecs.VAR_INT,
            DraftActionPayload::action,
            DraftActionPayload::new);

    @Override
    public Type<? extends CustomPacketPayload> type() {
        return ID;
    }

}
