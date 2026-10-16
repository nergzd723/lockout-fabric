package me.marin.lockout.server;

import me.marin.lockout.Lockout;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.client.LockoutBoard;
import me.marin.lockout.generator.GoalDataGenerator;
import me.marin.lockout.generator.GoalGroup;
import me.marin.lockout.generator.GoalRequirements;
import me.marin.lockout.lockout.GoalRegistry;
import me.marin.lockout.lockout.goals.util.GoalDataConstants;
import me.marin.lockout.network.DraftActionPayload;
import me.marin.lockout.network.DraftStatePayload;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundSoundPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.sounds.SoundSource;
import oshi.util.tuples.Pair;

import java.util.*;

/**
 * Teams build the board by picking goals in turns before the match starts.
 * Picks go in snake order (1, 2, ..., n, n, ..., 2, 1, 1, 2, ...), so no team keeps the first-pick advantage.
 * Each turn offers a few goals; when the timer runs out, one of them is picked at random.
 */
public class DraftSession {

    private final MinecraftServer server;
    private final List<LockoutTeamServer> teams;
    private final int boardSize;
    private final int offerSize;
    private final int pickTicks;
    private final int[] rerollsLeft;

    /** Goals that can still be offered, in random order. */
    private final Deque<String> pool = new ArrayDeque<>();
    private final List<Pair<String, String>> picks = new ArrayList<>();
    private final List<Integer> pickTeams = new ArrayList<>();
    private final List<Pair<String, String>> offer = new ArrayList<>();

    private int ticksLeft;
    private boolean done = false;

    public DraftSession(MinecraftServer server, List<LockoutTeamServer> teams, int boardSize, int offerSize, int pickSeconds, int rerolls) {
        this.server = server;
        this.teams = teams;
        this.boardSize = boardSize;
        this.offerSize = offerSize;
        this.pickTicks = pickSeconds * 20;
        this.rerollsLeft = new int[teams.size()];
        Arrays.fill(rerollsLeft, rerolls);

        // Same filtering as BoardGenerator: world requirements, team count and the random pool.
        List<String> candidates = new ArrayList<>();
        for (String goal : GoalRegistry.INSTANCE.getRegisteredGoals()) {
            GoalRequirements requirements = GoalRegistry.INSTANCE.getGoalGenerator(goal);
            if (requirements != null) {
                if (!requirements.isTeamsSizeOk(teams.size())) continue;
                if (!requirements.isPartOfRandomPool()) continue;
                if (!requirements.isSatisfied(LockoutServer.BIOME_LOCATE_DATA, LockoutServer.STRUCTURE_LOCATE_DATA)) continue;
            }
            candidates.add(goal);
        }
        Collections.shuffle(candidates, Lockout.random);
        pool.addAll(candidates);
    }

    public void start() {
        if (teams.size() == 1) {
            broadcast(Component.literal("Drafting a " + boardSize + "x" + boardSize + " Blackout board."));
        } else {
            String order = String.join(ChatFormatting.GRAY + " → ", teams.stream().map(t -> t.getColor() + t.getDisplayName()).toList());
            broadcast(Component.literal("Drafting a " + boardSize + "x" + boardSize + " board. Pick order: " + order + ChatFormatting.GRAY + " (snake order)."));
        }
        newOffer();
        sendState();
    }

    public void tick() {
        if (done) return;
        ticksLeft--;
        if (offer.isEmpty()) {
            // Ran out of goals to offer (only possible with a tiny goal pool).
            cancel();
            LockoutServer.draftSession = null;
        } else if (ticksLeft <= 0) {
            pick(Lockout.random.nextInt(offer.size()), true);
        } else if (ticksLeft % 20 == 0) {
            sendState();
        }
    }

    /**
     * Team picking in the current turn. Snake order: forward on even rounds, backward on odd rounds.
     */
    public int getCurrentTeam() {
        int round = picks.size() / teams.size();
        int position = picks.size() % teams.size();
        return round % 2 == 0 ? position : teams.size() - 1 - position;
    }

    public void handleAction(ServerPlayer player, DraftActionPayload payload) {
        if (done) return;
        int team = getCurrentTeam();
        if (!teams.get(team).getPlayers().contains(player.getUUID())) {
            player.sendSystemMessage(Component.literal("It's not your team's turn to pick.").withStyle(ChatFormatting.RED));
            return;
        }

        if (payload.action() == DraftActionPayload.REROLL) {
            if (rerollsLeft[team] <= 0) return;
            rerollsLeft[team]--;
            broadcast(Component.literal(teams.get(team).getColor() + teams.get(team).getDisplayName() + ChatFormatting.GRAY + " rerolled."));
            pool.addAll(offer.stream().map(Pair::getA).toList());
            newOffer();
            sendState();
        } else if (payload.action() >= 0 && payload.action() < offer.size()) {
            pick(payload.action(), false);
        }
    }

    private void pick(int offerIndex, boolean timedOut) {
        int team = getCurrentTeam();
        Pair<String, String> goal = offer.remove(offerIndex);
        picks.add(goal);
        pickTeams.add(team);
        // Goals that weren't picked can be offered again later.
        pool.addAll(offer.stream().map(Pair::getA).toList());
        offer.clear();

        String goalName = GoalRegistry.INSTANCE.newGoal(goal.getA(), goal.getB()).getGoalName();
        LockoutTeamServer lockoutTeam = teams.get(team);
        broadcast(Component.literal(lockoutTeam.getColor() + lockoutTeam.getDisplayName() + ChatFormatting.RESET + (timedOut ? " ran out of time, got " : " picked ") + ChatFormatting.WHITE + goalName));
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            player.connection.send(new ClientboundSoundPacket(SoundEvents.NOTE_BLOCK_PLING, SoundSource.MASTER, player.getX(), player.getY(), player.getZ(), 0.6f, timedOut ? 0.8f : 1.6f, player.getRandom().nextLong()));
        }

        if (picks.size() == boardSize * boardSize) {
            finish();
        } else {
            newOffer();
            sendState();
        }
    }

    private void newOffer() {
        offer.clear();
        List<String> pickedTypes = picks.stream().map(Pair::getA).toList();

        // Respect goal groups (no conflicting goals on the board), checked against what's already picked.
        List<String> skipped = new ArrayList<>();
        while (offer.size() < offerSize && !pool.isEmpty()) {
            String goal = pool.poll();
            if (GoalGroup.canAdd(goal, pickedTypes)) {
                offer.add(new Pair<>(goal, generateData(goal)));
            } else {
                skipped.add(goal);
            }
        }
        pool.addAll(skipped);

        // Out of compatible goals: offer anything that isn't on the board yet.
        while (offer.size() < offerSize && !pool.isEmpty()) {
            String goal = pool.poll();
            offer.add(new Pair<>(goal, generateData(goal)));
        }

        ticksLeft = pickTicks;
    }

    private String generateData(String goal) {
        Optional<GoalDataGenerator> generator = GoalRegistry.INSTANCE.getDataGenerator(goal);
        return generator.map(g -> g.generateData(LockoutServer.AVAILABLE_DYE_COLORS)).orElse(GoalDataConstants.DATA_NONE);
    }

    private void finish() {
        done = true;
        sendState(DraftStatePayload.Status.FINISHED);
        broadcast(Component.literal("The board is drafted!").withStyle(ChatFormatting.GREEN));
        LockoutServer.draftSession = null;
        LockoutServer.startLockout(teams, new LockoutBoard(new ArrayList<>(picks)));
    }

    public void cancel() {
        done = true;
        sendState(DraftStatePayload.Status.CANCELLED);
        broadcast(Component.literal("The draft was cancelled.").withStyle(ChatFormatting.RED));
    }

    private void sendState() {
        sendState(DraftStatePayload.Status.RUNNING);
    }

    private void sendState(DraftStatePayload.Status status) {
        List<DraftStatePayload.Pick> pickList = new ArrayList<>();
        for (int i = 0; i < picks.size(); i++) {
            pickList.add(new DraftStatePayload.Pick(picks.get(i).getA(), picks.get(i).getB(), pickTeams.get(i)));
        }
        List<DraftStatePayload.Offer> offerList = offer.stream().map(g -> new DraftStatePayload.Offer(g.getA(), g.getB())).toList();
        List<Integer> rerolls = Arrays.stream(rerollsLeft).boxed().toList();

        var payload = new DraftStatePayload(status, new ArrayList<LockoutTeam>(teams), boardSize, pickList, offerList,
                getCurrentTeam(), Math.max(ticksLeft, 0), pickTicks, rerolls);
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            ServerPlayNetworking.send(player, payload);
        }
    }

    private void broadcast(Component message) {
        server.getPlayerList().broadcastSystemMessage(message, false);
    }

}
