package me.marin.lockout.client.gui;

import com.mojang.blaze3d.platform.InputConstants;
import me.marin.lockout.LockoutTeam;
import me.marin.lockout.lockout.Goal;
import me.marin.lockout.lockout.GoalRegistry;
import me.marin.lockout.network.DraftActionPayload;
import me.marin.lockout.network.DraftStatePayload;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;

import java.util.ArrayList;
import java.util.List;

import static me.marin.lockout.Constants.*;

/**
 * Shown while the board is being drafted: the board filling up, the goals on offer, the timer and the pick order.
 */
public class DraftScreen extends Screen {

    private static final int CARD_WIDTH = 170;
    private static final int CARD_HEIGHT = 22;
    private static final int CARD_GAP = 4;
    private static final int UPCOMING_PICKS_SHOWN = 8;

    /** Pick count when the player closed the screen; it reopens on the next pick. */
    private static int dismissedAtPick = -1;

    private DraftStatePayload state;
    private final List<Goal> pickedGoals = new ArrayList<>();
    private final List<Goal> offeredGoals = new ArrayList<>();
    private Button rerollButton;

    public DraftScreen(DraftStatePayload state) {
        super(Component.literal("Draft"));
        setState(state);
    }

    public static void onState(DraftStatePayload state) {
        Minecraft client = Minecraft.getInstance();
        Screen screen = client.gui.screen();

        if (state.status() != DraftStatePayload.Status.RUNNING) {
            dismissedAtPick = -1;
            if (screen instanceof DraftScreen draftScreen) {
                draftScreen.closeWithoutDismissing();
            }
            return;
        }

        if (screen instanceof DraftScreen draftScreen) {
            draftScreen.setState(state);
        } else if (screen == null && state.picks().size() != dismissedAtPick) {
            client.gui.setScreen(new DraftScreen(state));
        }
    }

    private void setState(DraftStatePayload state) {
        boolean picksChanged = this.state == null || this.state.picks().size() != state.picks().size() || !this.state.offer().equals(state.offer());
        this.state = state;
        if (picksChanged) {
            pickedGoals.clear();
            for (DraftStatePayload.Pick pick : state.picks()) {
                pickedGoals.add(GoalRegistry.INSTANCE.newGoal(pick.goalId(), pick.data()));
            }
            offeredGoals.clear();
            for (DraftStatePayload.Offer offer : state.offer()) {
                offeredGoals.add(GoalRegistry.INSTANCE.newGoal(offer.goalId(), offer.data()));
            }
        }
        updateRerollButton();
    }

    private int getMyTeam() {
        String myName = Minecraft.getInstance().getUser().getName();
        for (int i = 0; i < state.teams().size(); i++) {
            if (state.teams().get(i).getPlayerNames().contains(myName)) {
                return i;
            }
        }
        return -1;
    }

    private boolean isMyTurn() {
        return getMyTeam() == state.currentTeam();
    }

    @Override
    protected void init() {
        super.init();
        rerollButton = Button.builder(Component.literal("Reroll"), b -> ClientPlayNetworking.send(new DraftActionPayload(DraftActionPayload.REROLL)))
                .width(CARD_WIDTH).pos(offersX(), offersY() + state.offer().size() * (CARD_HEIGHT + CARD_GAP) + 4).build();
        addRenderableWidget(rerollButton);
        updateRerollButton();
    }

    private void updateRerollButton() {
        if (rerollButton == null) return;
        int myTeam = getMyTeam();
        int rerolls = myTeam >= 0 ? state.rerollsLeft().get(myTeam) : 0;
        rerollButton.setMessage(Component.literal("Reroll (" + rerolls + " left)"));
        rerollButton.active = isMyTurn() && rerolls > 0;
        rerollButton.visible = myTeam >= 0;
        rerollButton.setY(offersY() + state.offer().size() * (CARD_HEIGHT + CARD_GAP) + 4);
    }

    // Layout: board on the left of the center line, offered goals on the right.

    private int boardPixelSize() {
        return 2 * GUI_CENTER_PADDING + state.boardSize() * GUI_CENTER_SLOT_SIZE;
    }

    private int boardX() {
        return width / 2 - boardPixelSize() - 12;
    }

    private int boardY() {
        return height / 2 - boardPixelSize() / 2 + 6;
    }

    private int offersX() {
        return width / 2 + 12;
    }

    private int offersY() {
        return height / 2 - 40;
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor context, int mouseX, int mouseY, float delta) {
        super.extractRenderState(context, mouseX, mouseY, delta);
        Font font = Minecraft.getInstance().font;

        drawHeader(context, font);
        Goal hovered = drawBoard(context, font, mouseX, mouseY);
        Goal hoveredOffer = drawOffers(context, font, mouseX, mouseY);
        drawPickOrder(context, font);

        if (hoveredOffer != null) hovered = hoveredOffer;
        if (hovered != null) {
            List<FormattedCharSequence> tooltip = new ArrayList<>();
            tooltip.add(Component.literal(hovered.getGoalName()).getVisualOrderText());
            int pickIndex = pickedGoals.indexOf(hovered);
            if (pickIndex >= 0) {
                LockoutTeam team = state.teams().get(state.picks().get(pickIndex).team());
                tooltip.add(Component.literal(ChatFormatting.GRAY + "Picked by " + team.getColor() + team.getDisplayName()).getVisualOrderText());
            }
            context.setTooltipForNextFrame(font, tooltip, mouseX, mouseY);
        }
    }

    private void drawHeader(GuiGraphicsExtractor context, Font font) {
        int y = 12;
        String title = ChatFormatting.BOLD + "Draft" + ChatFormatting.RESET + ChatFormatting.GRAY + "  pick " + (state.picks().size() + 1) + " of " + state.boardSize() * state.boardSize();
        context.text(font, title, width / 2 - font.width(title) / 2, y, 0xFFFFFFFF, true);
        y += font.lineHeight + 4;

        LockoutTeam current = state.teams().get(state.currentTeam());
        String status;
        if (isMyTurn()) {
            status = ChatFormatting.GREEN + (state.teams().size() == 1 ? "Pick a goal!" : "Your team's pick!");
        } else {
            status = current.getColor() + current.getDisplayName() + ChatFormatting.WHITE + " is picking...";
        }
        context.text(font, status, width / 2 - font.width(status) / 2, y, 0xFFFFFFFF, true);
        y += font.lineHeight + 4;

        // Timer bar
        int barWidth = 160;
        int barX = width / 2 - barWidth / 2;
        float fraction = state.pickTicks() == 0 ? 0 : (float) state.ticksLeft() / state.pickTicks();
        int seconds = (state.ticksLeft() + 19) / 20;
        int barColor = seconds <= 3 ? 0xFFE04848 : 0xFF6FD06F;
        context.fill(barX, y, barX + barWidth, y + 4, 0xFF303030);
        context.fill(barX, y, barX + Math.round(barWidth * fraction), y + 4, barColor);
        String timer = seconds + "s";
        context.text(font, timer, barX + barWidth + 6, y - 2, 0xFFFFFFFF, true);
    }

    private Goal drawBoard(GuiGraphicsExtractor context, Font font, int mouseX, int mouseY) {
        int size = state.boardSize();
        int x0 = boardX();
        int y0 = boardY();
        context.blitSprite(RenderPipelines.GUI_TEXTURED, GUI_CENTER_IDENTIFIER, x0, y0, boardPixelSize(), boardPixelSize());

        Goal hovered = null;
        for (int i = 0; i < size * size; i++) {
            int x = x0 + GUI_CENTER_PADDING + 1 + (i % size) * GUI_CENTER_SLOT_SIZE;
            int y = y0 + GUI_CENTER_PADDING + 1 + (i / size) * GUI_CENTER_SLOT_SIZE;
            if (i < pickedGoals.size()) {
                Goal goal = pickedGoals.get(i);
                int teamColor = LockoutTeam.colorRgb(state.teams().get(state.picks().get(i).team()).getColor());
                context.fill(x, y, x + 16, y + 16, 0x70000000 | teamColor);
                goal.render(context, font, x, y);
                if (mouseX >= x && mouseX < x + 16 && mouseY >= y && mouseY < y + 16) {
                    context.fill(x, y, x + 16, y + 16, GUI_CENTER_HOVERED_COLOR);
                    hovered = goal;
                }
            } else if (i == pickedGoals.size()) {
                // The slot the next pick goes into.
                int color = 0xFF000000 | LockoutTeam.colorRgb(state.teams().get(state.currentTeam()).getColor());
                context.fill(x, y, x + 16, y + 1, color);
                context.fill(x, y + 15, x + 16, y + 16, color);
                context.fill(x, y, x + 1, y + 16, color);
                context.fill(x + 15, y, x + 16, y + 16, color);
            }
        }
        return hovered;
    }

    private Goal drawOffers(GuiGraphicsExtractor context, Font font, int mouseX, int mouseY) {
        boolean canPick = isMyTurn();
        Goal hovered = null;
        int x = offersX();
        int y = offersY();
        context.text(font, ChatFormatting.GRAY + "On offer:", x, y - font.lineHeight - 3, 0xFFFFFFFF, true);

        for (int i = 0; i < offeredGoals.size(); i++) {
            Goal goal = offeredGoals.get(i);
            int top = y + i * (CARD_HEIGHT + CARD_GAP);
            boolean isHovered = mouseX >= x && mouseX < x + CARD_WIDTH && mouseY >= top && mouseY < top + CARD_HEIGHT;
            if (isHovered) hovered = goal;

            int background = !canPick ? 0xC0202020 : isHovered ? 0xE0505A70 : 0xC0303848;
            context.fill(x, top, x + CARD_WIDTH, top + CARD_HEIGHT, background);
            if (canPick && isHovered) {
                context.fill(x, top, x + 2, top + CARD_HEIGHT, 0xFF6FD06F);
            }
            goal.render(context, font, x + 3, top + 3);

            String name = goal.getGoalName();
            int maxTextWidth = CARD_WIDTH - 26;
            if (font.width(name) > maxTextWidth) {
                name = font.plainSubstrByWidth(name, maxTextWidth - font.width("...")) + "...";
            }
            context.text(font, (canPick ? ChatFormatting.WHITE : ChatFormatting.GRAY) + name, x + 23, top + (CARD_HEIGHT - font.lineHeight) / 2 + 1, 0xFFFFFFFF, true);
        }
        return hovered;
    }

    private void drawPickOrder(GuiGraphicsExtractor context, Font font) {
        int teamCount = state.teams().size();
        if (teamCount == 1) return;
        int picked = state.picks().size();
        int total = state.boardSize() * state.boardSize();

        List<String> upcoming = new ArrayList<>();
        for (int i = picked; i < Math.min(total, picked + UPCOMING_PICKS_SHOWN); i++) {
            int round = i / teamCount;
            int position = i % teamCount;
            int team = round % 2 == 0 ? position : teamCount - 1 - position;
            LockoutTeam lockoutTeam = state.teams().get(team);
            upcoming.add((i == picked ? ChatFormatting.UNDERLINE + "" : "") + lockoutTeam.getColor() + lockoutTeam.getDisplayName() + ChatFormatting.RESET);
        }
        String line = ChatFormatting.GRAY + "Next: " + String.join(ChatFormatting.GRAY + ", ", upcoming);
        context.text(font, line, width / 2 - font.width(line) / 2, height - 20, 0xFFFFFFFF, true);
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
        if (event.button() == InputConstants.MOUSE_BUTTON_LEFT && isMyTurn()) {
            int x = offersX();
            int y = offersY();
            for (int i = 0; i < offeredGoals.size(); i++) {
                int top = y + i * (CARD_HEIGHT + CARD_GAP);
                if (event.x() >= x && event.x() < x + CARD_WIDTH && event.y() >= top && event.y() < top + CARD_HEIGHT) {
                    ClientPlayNetworking.send(new DraftActionPayload(i));
                    return true;
                }
            }
        }
        return super.mouseClicked(event, doubleClick);
    }

    @Override
    public void onClose() {
        dismissedAtPick = state.picks().size();
        super.onClose();
    }

    private void closeWithoutDismissing() {
        super.onClose();
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }

}
