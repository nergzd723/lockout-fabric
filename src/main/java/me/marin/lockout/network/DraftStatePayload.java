package me.marin.lockout.network;

import me.marin.lockout.Constants;
import me.marin.lockout.LockoutTeam;
import net.minecraft.ChatFormatting;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;

import java.util.ArrayList;
import java.util.List;

/**
 * Full state of the draft, sent to every player whenever it changes and once a second for the timer.
 *
 * @param picks       goals picked so far, in pick order (board order)
 * @param offer       goals the current team can pick from
 * @param currentTeam index of the team that is picking
 * @param ticksLeft   ticks until a random offered goal is picked
 * @param pickTicks   ticks each pick lasts
 * @param rerollsLeft rerolls left per team
 */
public record DraftStatePayload(Status status, List<LockoutTeam> teams, int boardSize, List<Pick> picks, List<Offer> offer,
                                int currentTeam, int ticksLeft, int pickTicks, List<Integer> rerollsLeft) implements CustomPacketPayload {

    public enum Status { RUNNING, FINISHED, CANCELLED }

    public record Pick(String goalId, String data, int team) {}
    public record Offer(String goalId, String data) {}

    public static final Type<DraftStatePayload> ID = new Type<>(Constants.DRAFT_STATE_PACKET);
    public static final StreamCodec<RegistryFriendlyByteBuf, DraftStatePayload> CODEC = new StreamCodec<>() {
        @Override
        public DraftStatePayload decode(RegistryFriendlyByteBuf buf) {
            Status status = Status.values()[buf.readVarInt()];

            int teamCount = buf.readVarInt();
            List<LockoutTeam> teams = new ArrayList<>(teamCount);
            List<Integer> rerollsLeft = new ArrayList<>(teamCount);
            for (int i = 0; i < teamCount; i++) {
                ChatFormatting color = LockoutTeam.colorByName(buf.readUtf());
                int playerCount = buf.readVarInt();
                List<String> playerNames = new ArrayList<>(playerCount);
                for (int j = 0; j < playerCount; j++) {
                    playerNames.add(buf.readUtf());
                }
                teams.add(new LockoutTeam(playerNames, color));
                rerollsLeft.add(buf.readVarInt());
            }

            int boardSize = buf.readVarInt();
            int pickCount = buf.readVarInt();
            List<Pick> picks = new ArrayList<>(pickCount);
            for (int i = 0; i < pickCount; i++) {
                picks.add(new Pick(buf.readUtf(), buf.readUtf(), buf.readVarInt()));
            }
            int offerCount = buf.readVarInt();
            List<Offer> offer = new ArrayList<>(offerCount);
            for (int i = 0; i < offerCount; i++) {
                offer.add(new Offer(buf.readUtf(), buf.readUtf()));
            }

            int currentTeam = buf.readVarInt();
            int ticksLeft = buf.readVarInt();
            int pickTicks = buf.readVarInt();
            return new DraftStatePayload(status, teams, boardSize, picks, offer, currentTeam, ticksLeft, pickTicks, rerollsLeft);
        }

        @Override
        public void encode(RegistryFriendlyByteBuf buf, DraftStatePayload payload) {
            buf.writeVarInt(payload.status().ordinal());

            buf.writeVarInt(payload.teams().size());
            for (int i = 0; i < payload.teams().size(); i++) {
                LockoutTeam team = payload.teams().get(i);
                buf.writeUtf(LockoutTeam.colorName(team.getColor()));
                buf.writeVarInt(team.getPlayerNames().size());
                for (String playerName : team.getPlayerNames()) {
                    buf.writeUtf(playerName);
                }
                buf.writeVarInt(payload.rerollsLeft().get(i));
            }

            buf.writeVarInt(payload.boardSize());
            buf.writeVarInt(payload.picks().size());
            for (Pick pick : payload.picks()) {
                buf.writeUtf(pick.goalId());
                buf.writeUtf(pick.data());
                buf.writeVarInt(pick.team());
            }
            buf.writeVarInt(payload.offer().size());
            for (Offer offer : payload.offer()) {
                buf.writeUtf(offer.goalId());
                buf.writeUtf(offer.data());
            }

            buf.writeVarInt(payload.currentTeam());
            buf.writeVarInt(payload.ticksLeft());
            buf.writeVarInt(payload.pickTicks());
        }
    };

    @Override
    public Type<? extends CustomPacketPayload> type() {
        return ID;
    }

}
