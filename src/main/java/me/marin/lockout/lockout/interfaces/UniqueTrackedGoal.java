package me.marin.lockout.lockout.interfaces;

import me.marin.lockout.LockoutTeam;
import me.marin.lockout.LockoutTeamServer;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.server.LockoutServer;
import net.minecraft.ChatFormatting;
import net.minecraft.world.entity.player.Player;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * A goal completed by a team collecting enough unique things (items crafted, mob types spied on, ...).
 * The values are recorded in {@link #getTrackerMap()} by whatever hook observes them, and then {@link #evaluate} is called.
 */
public abstract class UniqueTrackedGoal<T> extends Goal implements RequiresAmount, Trackable<LockoutTeam, LinkedHashSet<T>>, HasTooltipInfo {

    public UniqueTrackedGoal(String id, String data) {
        super(id, data);
    }

    /**
     * Label for the progress line in the tooltip, e.g. "Unique Items crafted".
     */
    public abstract String getProgressLabel();

    /**
     * How a tracked value is shown in the tooltip.
     */
    public abstract String describe(T value);

    /**
     * Only these values count towards the goal. Defaults to every tracked value.
     */
    public boolean counts(T value) {
        return true;
    }

    public List<T> getProgress(LockoutTeam team) {
        Set<T> values = getTrackerMap().getOrDefault(team, new LinkedHashSet<>());
        return values.stream().filter(this::counts).toList();
    }

    public void evaluate(LockoutTeamServer team) {
        if (isCompleted()) return;
        team.sendTooltipUpdate(this);
        if (getProgress(team).size() >= getAmount()) {
            LockoutServer.lockout.completeGoal(this, team);
        }
    }

    @Override
    public List<String> getTooltip(LockoutTeam team, Player player) {
        List<String> tooltip = new ArrayList<>();
        List<T> progress = getProgress(team);

        tooltip.add(" ");
        tooltip.add(getProgressLabel() + ": " + progress.size() + "/" + getAmount());
        tooltip.addAll(HasTooltipInfo.commaSeparatedList(progress.stream().map(this::describe).toList()));
        tooltip.add(" ");

        return tooltip;
    }

    @Override
    public List<String> getSpectatorTooltip() {
        List<String> tooltip = new ArrayList<>();

        tooltip.add(" ");
        for (LockoutTeam team : LockoutServer.lockout.getTeams()) {
            tooltip.add(team.getColor() + team.getDisplayName() + ChatFormatting.RESET + ": " + getProgress(team).size() + "/" + getAmount());
        }
        tooltip.add(" ");

        return tooltip;
    }

}
