package me.marin.lockout;

import lombok.Getter;
import net.minecraft.ChatFormatting;
import net.minecraft.world.scores.TeamColor;
import org.apache.commons.lang3.StringUtils;

import java.util.List;

public class LockoutTeam {

    private final List<String> players;
    @Getter
    private final ChatFormatting color;
    @Getter
    private int points = 0;

    public LockoutTeam(List<String> playerNames, ChatFormatting formattingColor) {
        this.players = playerNames;
        this.color = formattingColor;
    }

    public List<String> getPlayerNames() {
        return players;
    }

    public String getDisplayName() {
        return players.size() == 1 ? players.get(0) : "Team " + formattingToString(color);
    }

    public void addPoint() {
        this.points++;
    }
    public void takePoint() {
        this.points--;
    }

    public static String formattingToString(ChatFormatting formatting) {
        return StringUtils.capitalize(colorName(formatting).replace("_", " "));
    }

    // The 16 color formattings share their order (and ids) with TeamColor.
    public static TeamColor toTeamColor(ChatFormatting formatting) {
        return TeamColor.VALUES.get(formatting.ordinal());
    }

    public static ChatFormatting fromTeamColor(TeamColor teamColor) {
        return colorById(teamColor.ordinal());
    }

    public static ChatFormatting colorById(int id) {
        return ChatFormatting.values()[id];
    }

    public static ChatFormatting colorByName(String name) {
        TeamColor teamColor = TeamColor.byName(name);
        return teamColor == null ? null : fromTeamColor(teamColor);
    }

    public static String colorName(ChatFormatting formatting) {
        return toTeamColor(formatting).getSerializedName();
    }

    public static int colorRgb(ChatFormatting formatting) {
        return toTeamColor(formatting).rgb();
    }

}
