//! The mod's screens, drawn as the Fabric client's are laid out: the board screen
//! (`BoardScreen`, the B key during a match), the draft (`DraftScreen`) and the Board Builder
//! (`BoardBuilderScreen`, B before a match, `/BoardBuilder`). Horizon gives a mod one screen
//! of its own; which of these it shows is [`Client::screen`].
use crate::client::{
    self, Client, ClientAction, ClientGoal, GUI_CENTER_HOVERED_COLOR, GUI_CENTER_PADDING, GUI_CENTER_SLOT_SIZE,
    GUI_SLOT_SIZE, MOD_ID, Painter, argb, draw_goal, legacy,
};
use crate::goals::{self, DATA_NONE, DYES};
use crate::wire::{self, Payload, REROLL};
use horizon_client::mod_api::*;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Board,
    Draft,
    Builder,
}

pub fn open(c: &mut Client, host: &mut dyn Host, screen: Screen) {
    if host.mod_screen() == Some(MOD_ID) || host.mod_screen_open(MOD_ID) {
        c.screen = Some(screen);
    }
}

pub fn close(c: &mut Client, host: &mut dyn Host) {
    c.screen = None;
    host.mod_screen_close(MOD_ID);
}

/// The screen closed (Escape, or another screen took its place).
pub fn closed(c: &mut Client) {
    if c.screen == Some(Screen::Draft) {
        c.draft_dismissed_at = c.draft.as_ref().map(|d| d.picks.len());
    }
    c.screen = None;
}

fn gui_size(host: &dyn Host, batch: Option<&HudBatch>) -> (i32, i32) {
    match host.client_state() {
        Some(s) => (s.gui_size[0] as i32, s.gui_size[1] as i32),
        None => batch.map_or((0, 0), |b| (b.width as i32, b.height as i32)),
    }
}

fn cursor(host: &dyn Host) -> (i32, i32) {
    let [x, y] = host.cursor();
    (x as i32, y as i32)
}

fn inside(m: (i32, i32), r: [i32; 4]) -> bool {
    m.0 >= r[0] && m.0 < r[0] + r[2] && m.1 >= r[1] && m.1 < r[1] + r[3]
}

/// A vanilla `Button`: its sprite (`gui_nine_slice` takes the atlas key, `gui/sprites/...`)
/// and its centred label, grey while inactive.
fn button(p: &mut Painter<'_>, rect: [i32; 4], label: &str, active: bool, mouse: (i32, i32)) {
    let sprite = if !active {
        "gui/sprites/widget/button_disabled"
    } else if inside(mouse, rect) {
        "gui/sprites/widget/button_highlighted"
    } else {
        "gui/sprites/widget/button"
    };
    p.host
        .gui_nine_slice(p.batch, sprite, rect.map(|v| v as f32), 0xFFFF_FFFF);
    let w = p.width(label);
    let color = if active { 0xFFFF_FFFF } else { 0xFFA0_A0A0 };
    p.text(
        label,
        rect[0] + (rect[2] - w) / 2,
        rect[1] + (rect[3] - 8) / 2,
        color,
        true,
    );
}

fn text_field(p: &mut Painter<'_>, rect: [i32; 4], value: &str, placeholder: &str, focused: bool, tick: u64) {
    let sprite = if focused {
        "gui/sprites/widget/text_field_highlighted"
    } else {
        "gui/sprites/widget/text_field"
    };
    p.host
        .gui_nine_slice(p.batch, sprite, rect.map(|v| v as f32), 0xFFFF_FFFF);
    let (x, y) = (rect[0] + 4, rect[1] + (rect[3] - 8) / 2);
    if value.is_empty() && !focused {
        p.text(placeholder, x, y, 0xFF80_8080, true);
    } else {
        p.text(value, x, y, 0xFFE0_E0E0, true);
        if focused && (tick / 6).is_multiple_of(2) {
            let cx = x + p.width(value);
            p.fill(cx, y - 1, cx + 1, y + 9, 0xFFD0_D0D0);
        }
    }
}

fn background(p: &mut Painter<'_>, w: i32, h: i32) {
    // `Screen.renderTransparentBackground` over the world.
    p.fill(0, 0, w, h, 0xC010_1010);
}

fn border(p: &mut Painter<'_>, x: i32, y: i32, w: i32, h: i32, color: u32) {
    p.fill(x, y, x + w, y + 1, color);
    p.fill(x, y + h - 1, x + w, y + h, color);
    p.fill(x, y + 1, x + 1, y + h - 1, color);
    p.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
}

/// The centred board's frame (`gui_center`, a nine-slice of border 7) at its top left.
fn center_frame(c: &Client, p: &mut Painter<'_>, x: i32, y: i32, size: i32) {
    let px = 2 * GUI_CENTER_PADDING + size * GUI_CENTER_SLOT_SIZE;
    if let Some(a) = &c.atlas {
        p.nine_slice(a, "gui/sprites/gui_center", [x, y, px, px], [7, 7, 7, 7]);
    }
}

/// `Utility.getBoardHoveredIndex`: the square under the mouse of a centred board.
fn hovered_square(size: i32, w: i32, h: i32, offset: i32, m: (i32, i32)) -> Option<usize> {
    let px = 2 * GUI_CENTER_PADDING + size * GUI_CENTER_SLOT_SIZE;
    let x0 = w / 2 - px / 2 + GUI_CENTER_PADDING - offset;
    let y0 = h / 2 - px / 2 + GUI_CENTER_PADDING;
    for i in 0..size {
        for j in 0..size {
            let (x, y) = (x0 + j * GUI_CENTER_SLOT_SIZE, y0 + i * GUI_CENTER_SLOT_SIZE);
            if m.0 >= x - 1 && m.0 < x + GUI_CENTER_SLOT_SIZE && m.1 >= y - 1 && m.1 < y + GUI_CENTER_SLOT_SIZE {
                return Some((j + i * size) as usize);
            }
        }
    }
    None
}

pub fn draw(c: &mut Client, host: &dyn Host, batch: &mut HudBatch) {
    if host.mod_screen() != Some(MOD_ID) {
        return;
    }
    match c.screen {
        Some(Screen::Board) => draw_board(c, host, batch),
        Some(Screen::Draft) => draw_draft(c, host, batch),
        Some(Screen::Builder) => draw_builder(c, host, batch),
        None => {}
    }
}

// ---- the board screen ------------------------------------------------------------------

fn draw_board(c: &mut Client, host: &dyn Host, batch: &mut HudBatch) {
    let Some(board) = c.board.clone() else { return };
    let (w, h) = gui_size(host, Some(batch));
    let mouse = cursor(host);
    let mut p = Painter { host, batch };
    background(&mut p, w, h);
    let size = board.side();
    let px = 2 * GUI_CENTER_PADDING + size * GUI_CENTER_SLOT_SIZE;
    let (x0, y0) = (w / 2 - px / 2, h / 2 - px / 2);
    center_frame(c, &mut p, x0, y0, size);
    let hovered = hovered_square(size, w, h, 0, mouse);
    for (i, goal) in board.goals.iter().enumerate() {
        let x = x0 + GUI_CENTER_PADDING + 1 + (i as i32 % size) * GUI_CENTER_SLOT_SIZE;
        let y = y0 + GUI_CENTER_PADDING + 1 + (i as i32 / size) * GUI_CENTER_SLOT_SIZE;
        if let Some(team) = goal.completed_by.and_then(|t| board.teams.get(t)) {
            p.fill(x, y, x + 16, y + 16, argb(wire::color_rgb(&team.color)));
        }
        draw_goal(c, &mut p, goal, x, y);
        if hovered == Some(i) {
            p.fill(x, y, x + 16, y + 16, GUI_CENTER_HOVERED_COLOR);
        }
    }
    if let Some(goal) = hovered.and_then(|i| board.goals.get(i)) {
        // `Utility.drawGoalInformation`.
        let mut lines = vec![format!("{}{}", if goal.has_tooltip { "§n" } else { "" }, goal.name)];
        if goal.has_tooltip
            && let Some(t) = c.tooltips.get(&goal.id)
        {
            lines.extend(t.split('\n').map(str::to_owned));
        }
        p.tooltip(&lines, [mouse.0, mouse.1]);
    }
}

// ---- the draft screen ------------------------------------------------------------------

const CARD_WIDTH: i32 = 170;
const CARD_HEIGHT: i32 = 22;
const CARD_GAP: i32 = 4;
const UPCOMING_PICKS_SHOWN: usize = 8;

struct DraftLayout {
    board_x: i32,
    board_y: i32,
    offers_x: i32,
    offers_y: i32,
}

fn draft_layout(board_size: i32, w: i32, h: i32) -> DraftLayout {
    let px = 2 * GUI_CENTER_PADDING + board_size * GUI_CENTER_SLOT_SIZE;
    DraftLayout {
        board_x: w / 2 - px - 12,
        board_y: h / 2 - px / 2 + 6,
        offers_x: w / 2 + 12,
        offers_y: h / 2 - 40,
    }
}

fn draft_team_name(team: &wire::WireTeam) -> String {
    match team.players.as_slice() {
        [one] => one.clone(),
        _ => format!("Team {}", wire::color_display(&team.color)),
    }
}

fn reroll_rect(d: &client::DraftView, l: &DraftLayout) -> [i32; 4] {
    [
        l.offers_x,
        l.offers_y + d.offer.len() as i32 * (CARD_HEIGHT + CARD_GAP) + 4,
        CARD_WIDTH,
        20,
    ]
}

fn draw_draft(c: &mut Client, host: &dyn Host, batch: &mut HudBatch) {
    let Some(d) = c.draft.clone() else { return };
    let (w, h) = gui_size(host, Some(batch));
    let mouse = cursor(host);
    let my_team = c.my_draft_team();
    let my_turn = my_team == Some(d.current_team as usize);
    let l = draft_layout(d.board_size, w, h);
    let mut p = Painter { host, batch };
    background(&mut p, w, h);
    let mut hovered: Option<(String, Option<usize>)> = None;

    // The header: pick n of m, whose turn, the timer bar.
    let mut y = 12;
    let title = format!(
        "§lDraft§r§7  pick {} of {}",
        d.picks.len() + 1,
        d.board_size * d.board_size
    );
    p.text(&title, w / 2 - p.width(&title) / 2, y, 0xFFFF_FFFF, true);
    y += 9 + 4;
    let status = if my_turn {
        format!(
            "§a{}",
            if d.teams.len() == 1 {
                "Pick a goal!"
            } else {
                "Your team's pick!"
            }
        )
    } else {
        let t = &d.teams[d.current_team.max(0) as usize % d.teams.len().max(1)].team;
        format!("{}{}§f is picking...", legacy(&t.color), draft_team_name(t))
    };
    p.text(&status, w / 2 - p.width(&status) / 2, y, 0xFFFF_FFFF, true);
    y += 9 + 4;
    let bar = 160;
    let bar_x = w / 2 - bar / 2;
    let fraction = if d.pick_ticks == 0 {
        0.0
    } else {
        d.ticks_left as f32 / d.pick_ticks as f32
    };
    let seconds = (d.ticks_left + 19) / 20;
    p.fill(bar_x, y, bar_x + bar, y + 4, 0xFF30_3030);
    p.fill(
        bar_x,
        y,
        bar_x + (bar as f32 * fraction).round() as i32,
        y + 4,
        if seconds <= 3 { 0xFFE0_4848 } else { 0xFF6F_D06F },
    );
    p.text(&format!("{seconds}s"), bar_x + bar + 6, y - 2, 0xFFFF_FFFF, true);

    // The board filling up.
    let size = d.board_size;
    center_frame(c, &mut p, l.board_x, l.board_y, size);
    for i in 0..(size * size) as usize {
        let x = l.board_x + GUI_CENTER_PADDING + 1 + (i as i32 % size) * GUI_CENTER_SLOT_SIZE;
        let y = l.board_y + GUI_CENTER_PADDING + 1 + (i as i32 / size) * GUI_CENTER_SLOT_SIZE;
        if let Some((id, data, team)) = d.picks.get(i) {
            let goal = ClientGoal::new(id, data);
            let color = d
                .teams
                .get(*team as usize)
                .map_or(0xFFFFFF, |t| wire::color_rgb(&t.team.color));
            p.fill(x, y, x + 16, y + 16, 0x7000_0000 | color);
            draw_goal(c, &mut p, &goal, x, y);
            if inside(mouse, [x, y, 16, 16]) {
                p.fill(x, y, x + 16, y + 16, GUI_CENTER_HOVERED_COLOR);
                hovered = Some((goal.name.clone(), Some(*team as usize)));
            }
        } else if i == d.picks.len() {
            // The slot the next pick goes into.
            let color = argb(
                d.teams
                    .get(d.current_team as usize)
                    .map_or(0xFFFFFF, |t| wire::color_rgb(&t.team.color)),
            );
            border(&mut p, x, y, 16, 16, color);
        }
    }

    // The goals on offer.
    p.text("§7On offer:", l.offers_x, l.offers_y - 9 - 3, 0xFFFF_FFFF, true);
    for (i, (id, data)) in d.offer.iter().enumerate() {
        let goal = ClientGoal::new(id, data);
        let top = l.offers_y + i as i32 * (CARD_HEIGHT + CARD_GAP);
        let card = [l.offers_x, top, CARD_WIDTH, CARD_HEIGHT];
        let over = inside(mouse, card);
        if over {
            hovered = Some((goal.name.clone(), None));
        }
        let back = if !my_turn {
            0xC020_2020
        } else if over {
            0xE050_5A70
        } else {
            0xC030_3848
        };
        p.fill(card[0], top, card[0] + CARD_WIDTH, top + CARD_HEIGHT, back);
        if my_turn && over {
            p.fill(card[0], top, card[0] + 2, top + CARD_HEIGHT, 0xFF6F_D06F);
        }
        draw_goal(c, &mut p, &goal, card[0] + 3, top + 3);
        let mut name = goal.name.clone();
        let max = CARD_WIDTH - 26;
        if p.width(&name) > max {
            while !name.is_empty() && p.width(&format!("{name}...")) > max {
                name.pop();
            }
            name.push_str("...");
        }
        p.text(
            &format!("{}{name}", if my_turn { "§f" } else { "§7" }),
            card[0] + 23,
            top + (CARD_HEIGHT - 9) / 2 + 1,
            0xFFFF_FFFF,
            true,
        );
    }
    if let Some(team) = my_team {
        let rerolls = d.teams.get(team).map_or(0, |t| t.rerolls);
        button(
            &mut p,
            reroll_rect(&d, &l),
            &format!("Reroll ({rerolls} left)"),
            my_turn && rerolls > 0,
            mouse,
        );
    }

    // The pick order ahead.
    let n = d.teams.len();
    if n > 1 {
        let picked = d.picks.len();
        let total = (d.board_size * d.board_size) as usize;
        let upcoming: Vec<String> = (picked..total.min(picked + UPCOMING_PICKS_SHOWN))
            .map(|i| {
                let (round, position) = (i / n, i % n);
                let team = if round % 2 == 0 { position } else { n - 1 - position };
                let t = &d.teams[team].team;
                format!(
                    "{}{}{}§r",
                    if i == picked { "§n" } else { "" },
                    legacy(&t.color),
                    draft_team_name(t)
                )
            })
            .collect();
        let line = format!("§7Next: {}", upcoming.join("§7, "));
        p.text(&line, w / 2 - p.width(&line) / 2, h - 20, 0xFFFF_FFFF, true);
    }

    if let Some((name, team)) = hovered {
        let mut lines = vec![name];
        if let Some(t) = team.and_then(|t| d.teams.get(t)) {
            lines.push(format!(
                "§7Picked by {}{}",
                legacy(&t.team.color),
                draft_team_name(&t.team)
            ));
        }
        p.tooltip(&lines, [mouse.0, mouse.1]);
    }
}

fn click_draft(c: &mut Client, host: &dyn Host, button: i32) -> bool {
    let Some(d) = c.draft.clone() else { return false };
    if button != 0 || c.my_draft_team() != Some(d.current_team as usize) {
        return false;
    }
    let (w, h) = gui_size(host, None);
    let mouse = cursor(host);
    let l = draft_layout(d.board_size, w, h);
    let action = (0..d.offer.len())
        .find(|i| {
            inside(
                mouse,
                [
                    l.offers_x,
                    l.offers_y + *i as i32 * (CARD_HEIGHT + CARD_GAP),
                    CARD_WIDTH,
                    CARD_HEIGHT,
                ],
            )
        })
        .map(|i| i as i32)
        .or_else(|| inside(mouse, reroll_rect(&d, &l)).then_some(REROLL));
    if let Some(action) = action
        && let Some(packet) = c.packet(&Payload::DraftAction { action })
    {
        c.later.push(ClientAction::Send(packet));
        return true;
    }
    false
}

// ---- the Board Builder -----------------------------------------------------------------

/// What the Board Builder holds between openings (`BoardBuilderData`).
#[derive(Debug, Clone)]
pub struct Builder {
    pub title: String,
    pub size: usize,
    pub goals: Vec<Option<(String, String)>>,
    pub modifying: Option<usize>,
    pub search: String,
    pub panel: Panel,
    pub focus: Option<Field>,
    pub scroll: f32,
    pub data_fields: Vec<String>,
    pub error: Option<String>,
    pub data_error: Option<String>,
    /// The goals the search lists, with the random data each was given (`GoalEntry`).
    entries: Vec<(String, String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    None,
    Search,
    EditData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Search,
    Data(usize),
}

impl Default for Builder {
    fn default() -> Self {
        let mut rng = crate::board::Rng::new(0x10c0_u64);
        let entries = goals::table()
            .goals
            .iter()
            .map(|g| {
                let data = match g.data.as_deref() {
                    Some("leather_dye") => {
                        let pieces = [
                            "leather_helmet",
                            "leather_chestplate",
                            "leather_leggings",
                            "leather_boots",
                        ];
                        format!("{}&{}", pieces[rng.below(4)], DYES[rng.below(16)])
                    }
                    Some(_) => DYES[rng.below(16)].to_owned(),
                    None => DATA_NONE.to_owned(),
                };
                let name = if g.data.is_some() {
                    format!("[*] {}", title_case(&g.id))
                } else {
                    g.name(&data)
                };
                (g.id.clone(), data, name)
            })
            .collect();
        Self {
            title: String::new(),
            size: 5,
            goals: vec![None; 25],
            modifying: None,
            search: String::new(),
            panel: Panel::None,
            focus: None,
            scroll: 0.0,
            data_fields: Vec::new(),
            error: None,
            data_error: None,
            entries,
        }
    }
}

/// `WordUtils.capitalize("KILL_COLORED_SHEEP".replace("_", " ").toLowerCase())`.
fn title_case(id: &str) -> String {
    id.to_lowercase()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Builder {
    pub fn clear(&mut self) {
        self.goals.iter_mut().for_each(|g| *g = None);
        self.modifying = None;
    }

    /// One more column on the right and row at the bottom.
    pub fn increment(&mut self) {
        if self.size >= crate::server::MAX_BOARD_SIZE {
            return;
        }
        let (row, col) = self.modifying.map_or((0, 0), |m| (m / self.size, m % self.size));
        self.size += 1;
        for i in 0..self.size - 1 {
            self.goals.insert(self.size * i + (self.size - 1), None);
        }
        if self.modifying.is_some() {
            self.modifying = Some(row * self.size + col);
        }
        self.goals.extend(std::iter::repeat_n(None, self.size));
    }

    /// The rightmost column and bottom row go, with their goals.
    pub fn decrement(&mut self) {
        if self.size <= crate::server::MIN_BOARD_SIZE {
            return;
        }
        let (mut row, mut col) = self.modifying.map_or((0, 0), |m| (m / self.size, m % self.size));
        self.size -= 1;
        for _ in 0..self.size + 1 {
            self.goals.pop();
        }
        for i in (0..self.size).rev() {
            self.goals.remove((self.size + 1) * i + self.size);
        }
        if self.modifying.is_some() {
            row = row.min(self.size - 1);
            col = col.min(self.size - 1);
            self.modifying = Some(row * self.size + col);
        }
    }

    pub fn set_board(&mut self, title: &str, size: usize, goals: &[(String, String)]) {
        self.title = title.to_owned();
        self.size = size;
        self.modifying = None;
        self.goals = goals
            .iter()
            .map(|(id, data)| goals::table().get(id).map(|_| (id.clone(), data.clone())))
            .collect();
    }

    fn visible(&self) -> Vec<&(String, String, String)> {
        let q = self.search.to_lowercase();
        self.entries
            .iter()
            .filter(|(_, _, name)| name.to_lowercase().contains(&q))
            .collect()
    }

    fn offset(&self) -> i32 {
        match self.panel {
            Panel::Search => 100,
            Panel::EditData => 50,
            Panel::None => 0,
        }
    }

    /// The data's parts (`GoalDataGenerator`'s generators): a leather piece and a dye, or a dye.
    fn generators(&self) -> Vec<&'static str> {
        let Some((id, _)) = self.modifying.and_then(|m| self.goals.get(m).cloned().flatten()) else {
            return Vec::new();
        };
        match goals::table().get(&id).and_then(|g| g.data.as_deref()) {
            Some("leather_dye") => vec!["Leather Armor Piece", "Dye Color"],
            Some(_) => vec!["Dye Color"],
            None => Vec::new(),
        }
    }
}

struct BuilderLayout {
    title: [i32; 4],
    save: [i32; 4],
    close: [i32; 4],
    clear: [i32; 4],
    plus: [i32; 4],
    minus: [i32; 4],
    list: [i32; 4],
    close_search: [i32; 4],
    search: [i32; 4],
    data_x: i32,
    data_y: i32,
}

fn builder_layout(b: &Builder, w: i32, h: i32) -> BuilderLayout {
    let (cx, cy) = (w / 2, h / 2);
    let off = b.offset();
    let half = crate::client::GUI_PADDING + (b.size as i32 * GUI_SLOT_SIZE) / 2;
    let bottom = h - 30;
    let close = [w - 50 - 10, bottom, 50, 20];
    let list = [cx + half + 35 - off, 40, w / 2 - 125 + off, h - 80];
    let close_search = [list[0], list[1] - 21, 20, 20];
    let n = b.generators().len() as i32;
    BuilderLayout {
        title: [cx - 60 - off, cy - half - 18 - 8, 120, 18],
        save: [10, bottom, 85, 20],
        close,
        clear: [close[0] - 85 - 10, bottom, 85, 20],
        plus: [cx + half - off + 8, cy - 10, 20, 20],
        minus: [cx - half - off - 20 - 8, cy - 10, 20, 20],
        list,
        close_search,
        search: [
            close_search[0] + 20 + 1 + 5,
            close_search[1] + 1,
            list[2] - 20 - 2 - 5,
            18,
        ],
        data_x: cx + 100 - off,
        data_y: cy - (18 + n * 45) / 2,
    }
}

fn draw_builder(c: &mut Client, host: &dyn Host, batch: &mut HudBatch) {
    let (w, h) = gui_size(host, Some(batch));
    let mouse = cursor(host);
    let b = c.builder.clone();
    let l = builder_layout(&b, w, h);
    let tick = c.tick;
    let mut p = Painter { host, batch };
    background(&mut p, w, h);
    text_field(
        &mut p,
        l.title,
        &b.title,
        "Board Name",
        b.focus == Some(Field::Title),
        tick,
    );
    button(&mut p, l.save, "Save Board", true, mouse);
    button(&mut p, l.clear, "Clear Board", true, mouse);
    button(&mut p, l.close, "Close", true, mouse);
    button(&mut p, l.plus, "+", b.size != crate::server::MAX_BOARD_SIZE, mouse);
    button(&mut p, l.minus, "-", b.size != crate::server::MIN_BOARD_SIZE, mouse);
    if let Some(e) = &b.error {
        p.text(e, 10, h - 45, 0xFFFF_0000, false);
    }

    // The board.
    let size = b.size as i32;
    let px = 2 * GUI_CENTER_PADDING + size * GUI_CENTER_SLOT_SIZE;
    let (x0, y0) = (w / 2 - px / 2 - b.offset(), h / 2 - px / 2);
    center_frame(c, &mut p, x0, y0, size);
    let hovered = hovered_square(size, w, h, b.offset(), mouse);
    let mut tooltip = None;
    for (i, goal) in b.goals.iter().enumerate() {
        let x = x0 + GUI_CENTER_PADDING + 1 + (i as i32 % size) * GUI_CENTER_SLOT_SIZE;
        let y = y0 + GUI_CENTER_PADDING + 1 + (i as i32 / size) * GUI_CENTER_SLOT_SIZE;
        let goal = goal.as_ref().map(|(id, data)| ClientGoal::new(id, data));
        if let Some(g) = &goal {
            draw_goal(c, &mut p, g, x, y);
        }
        if hovered == Some(i) {
            p.fill(x, y, x + 16, y + 16, GUI_CENTER_HOVERED_COLOR);
            if let Some(g) = &goal {
                let mut lines = vec![g.name.clone()];
                if g.data != DATA_NONE {
                    lines.push("§7§oRight-click to edit data.".into());
                }
                tooltip = Some(lines);
            }
        }
        if b.modifying == Some(i) {
            border(&mut p, x - 1, y - 1, GUI_SLOT_SIZE, GUI_SLOT_SIZE, 0xFFFF_0000);
        }
    }

    match b.panel {
        Panel::Search => {
            button(&mut p, l.close_search, "<", true, mouse);
            text_field(
                &mut p,
                l.search,
                &b.search,
                "Search goals..",
                b.focus == Some(Field::Search),
                tick,
            );
            let [lx, ly, lw, lh] = l.list;
            p.fill(lx, ly, lx + lw, ly + lh, 0x8000_0000);
            let visible = b.visible();
            let row_width = lw - 3 * 2 - 4;
            for (i, (id, data, name)) in visible.iter().enumerate() {
                let y = ly + 4 + i as i32 * 18 - b.scroll as i32 - 3;
                if y + 18 < ly || y > ly + lh {
                    continue;
                }
                let x = lx + 3;
                let goal = ClientGoal::new(id, data);
                draw_goal(c, &mut p, &goal, x, y);
                p.text(name, x + 18, y + 5, 0xFFFF_FFFF, false);
                if inside(mouse, [x - 1, y - 1, row_width + 2, 18]) && inside(mouse, l.list) {
                    border(&mut p, x - 1, y - 1, row_width + 2, 18, 0xFFC0_C0C0);
                }
            }
        }
        Panel::EditData => {
            let mut y = l.data_y;
            for (i, name) in b.generators().iter().enumerate() {
                p.text(name, l.data_x, y, 0xFFFF_FFFF, false);
                y += 15;
                let value = b.data_fields.get(i).cloned().unwrap_or_default();
                text_field(
                    &mut p,
                    [l.data_x, y, 150, 18],
                    &value,
                    "",
                    b.focus == Some(Field::Data(i)),
                    tick,
                );
                y += 30;
            }
            button(&mut p, [l.data_x, y, 20, 20], "<", true, mouse);
            button(&mut p, [l.data_x + 25, y, 50, 20], "Save", true, mouse);
            if let Some(e) = &b.data_error {
                p.text(e, l.data_x + 75 - p.width(e) / 2, y + 25, 0xFFFF_0000, false);
            }
        }
        Panel::None => {}
    }
    if let Some(lines) = tooltip {
        p.tooltip(&lines, [mouse.0, mouse.1]);
    }
}

fn click_builder(c: &mut Client, host: &dyn Host, button: i32) -> bool {
    let (w, h) = gui_size(host, None);
    let mouse = cursor(host);
    let l = builder_layout(&c.builder, w, h);
    let b = &mut c.builder;
    if button == 0 {
        if inside(mouse, l.title) {
            b.focus = Some(Field::Title);
            return true;
        }
        if inside(mouse, l.save) {
            save_board(c, host);
            return true;
        }
        if inside(mouse, l.clear) {
            b.clear();
            b.panel = Panel::None;
            b.modifying = None;
            return true;
        }
        if inside(mouse, l.close) {
            b.panel = Panel::None;
            b.modifying = None;
            c.later.push(ClientAction::CloseScreen);
            return true;
        }
        if inside(mouse, l.plus) {
            b.increment();
            return true;
        }
        if inside(mouse, l.minus) {
            b.decrement();
            return true;
        }
    }
    let size = b.size as i32;
    if (button == 0 || button == 1)
        && let Some(i) = hovered_square(size, w, h, b.offset(), mouse)
    {
        let goal = b.goals.get(i).cloned().flatten();
        b.modifying = Some(i);
        b.data_error = None;
        if button == 1 && goal.as_ref().is_some_and(|(_, d)| d != DATA_NONE) {
            b.panel = Panel::EditData;
            b.data_fields = goal
                .map(|(_, d)| d.split('&').map(str::to_owned).collect())
                .unwrap_or_default();
            b.focus = None;
        } else {
            b.panel = Panel::Search;
            b.focus = Some(Field::Search);
        }
        c.later.push(ClientAction::Sound("minecraft:ui.button.click"));
        return true;
    }
    if button != 0 {
        return false;
    }
    match b.panel {
        Panel::Search => {
            if inside(mouse, l.close_search) {
                b.panel = Panel::None;
                b.modifying = None;
                return true;
            }
            if inside(mouse, l.search) {
                b.focus = Some(Field::Search);
                return true;
            }
            if inside(mouse, l.list) {
                let row = (mouse.1 - l.list[1] + b.scroll as i32 - 3 + 3) / 18;
                let picked = b
                    .visible()
                    .get(row.max(0) as usize)
                    .map(|(id, data, _)| (id.clone(), data.clone()));
                if row >= 0
                    && let (Some(goal), Some(m)) = (picked, b.modifying)
                {
                    b.goals[m] = Some(goal);
                    c.later.push(ClientAction::Sound("minecraft:ui.button.click"));
                }
                return true;
            }
        }
        Panel::EditData => {
            let mut y = l.data_y;
            for i in 0..b.generators().len() {
                y += 15;
                if inside(mouse, [l.data_x, y, 150, 18]) {
                    b.focus = Some(Field::Data(i));
                    return true;
                }
                y += 30;
            }
            if inside(mouse, [l.data_x, y, 20, 20]) {
                b.panel = Panel::None;
                b.modifying = None;
                return true;
            }
            if inside(mouse, [l.data_x + 25, y, 50, 20]) {
                let generators = b.generators();
                for (i, g) in generators.iter().enumerate() {
                    let v = b.data_fields.get(i).map(String::as_str).unwrap_or("");
                    let ok = match *g {
                        "Dye Color" => DYES.contains(&v),
                        _ => [
                            "leather_helmet",
                            "leather_chestplate",
                            "leather_leggings",
                            "leather_boots",
                        ]
                        .contains(&v),
                    };
                    if !ok {
                        b.data_error = Some(format!("Invalid '{g}'."));
                        return true;
                    }
                }
                if let Some(m) = b.modifying
                    && let Some(Some((id, _))) = b.goals.get(m).cloned()
                {
                    b.goals[m] = Some((id, b.data_fields.join("&")));
                }
                b.panel = Panel::None;
                b.modifying = None;
                return true;
            }
        }
        Panel::None => {}
    }
    b.focus = None;
    false
}

/// The boards folder (`.minecraft/lockout-boards`), shared with the Fabric client.
pub fn boards_dir(host: &dyn Host) -> PathBuf {
    host.instance_dir().join("lockout-boards")
}

/// A saved board's goals (`JSONBoard`): id and data, data `null` when none.
pub fn read_board(host: &dyn Host, name: &str) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(boards_dir(host).join(format!("{name}.json"))).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let goals = v.get("goals").and_then(|g| g.as_array()).ok_or("no goals")?;
    Ok(goals
        .iter()
        .map(|g| {
            let id = g.get("id").and_then(|i| i.as_str()).unwrap_or("").to_owned();
            let data = g.get("data").and_then(|d| d.as_str()).unwrap_or(DATA_NONE).to_owned();
            (id, data)
        })
        .collect())
}

/// The saved boards' names.
pub fn saved_boards(host: &dyn Host) -> Vec<String> {
    std::fs::read_dir(boards_dir(host))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .and_then(|n| n.strip_suffix(".json"))
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `BoardBuilderIO.getSuitableName`: `name`, or `name (1)`, `(2)`... when it is taken.
pub fn suitable_name(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_owned();
    }
    let mut n = 1;
    while taken.contains(&format!("{name} ({n})")) {
        n += 1;
    }
    format!("{name} ({n})")
}

/// `JSONBoard` as Gson writes it: pretty, `data` left out when there is none.
pub fn board_json(goals: &[(String, String)]) -> String {
    let list: Vec<serde_json::Value> = goals
        .iter()
        .map(|(id, data)| {
            if data == DATA_NONE {
                serde_json::json!({"id": id})
            } else {
                serde_json::json!({"id": id, "data": data})
            }
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({"goals": list})).unwrap_or_default()
}

fn save_board(c: &mut Client, host: &dyn Host) {
    let b = &mut c.builder;
    let Some(goals) = b.goals.iter().cloned().collect::<Option<Vec<_>>>() else {
        b.error = Some("The board is not full.".into());
        return;
    };
    let mut unique = goals.clone();
    unique.sort();
    unique.dedup();
    if unique.len() < goals.len() {
        b.error = Some("Some goals are duplicated, fix and try again.".into());
        return;
    }
    let mut name = b.title.trim().to_owned();
    if name.is_empty() {
        name = "Custom Board".into();
    }
    let dir = boards_dir(host);
    let name = suitable_name(&name, &saved_boards(host));
    let written = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(dir.join(format!("{name}.json")), board_json(&goals)));
    if written.is_err() {
        c.builder.error = Some("Error while saving board. Check logs.".into());
        return;
    }
    c.builder.error = None;
    c.later.push(ClientAction::ActionBar(
        format!("Saved custom board as {name}.json!"),
        0x55FF55,
    ));
    c.builder.panel = Panel::None;
    c.builder.modifying = None;
    c.later.push(ClientAction::CloseScreen);
}

// ---- input -----------------------------------------------------------------------------

pub fn mouse_pressed(c: &mut Client, host: &dyn Host, button: i32) -> bool {
    if host.mod_screen() != Some(MOD_ID) {
        return false;
    }
    match c.screen {
        Some(Screen::Draft) => click_draft(c, host, button),
        Some(Screen::Builder) => click_builder(c, host, button),
        _ => false,
    }
}

pub fn scrolled(c: &mut Client, host: &dyn Host, dy: f64) -> bool {
    if host.mod_screen() != Some(MOD_ID) || c.screen != Some(Screen::Builder) || c.builder.panel != Panel::Search {
        return false;
    }
    let (w, h) = gui_size(host, None);
    let l = builder_layout(&c.builder, w, h);
    let content = c.builder.visible().len() as f32 * 18.0;
    let max = (content - l.list[3] as f32 + 4.0).max(0.0);
    c.builder.scroll = (c.builder.scroll - dy as f32 * 9.0).clamp(0.0, max);
    true
}

fn field(b: &mut Builder) -> Option<&mut String> {
    match b.focus? {
        Field::Title => Some(&mut b.title),
        Field::Search => Some(&mut b.search),
        Field::Data(i) => b.data_fields.get_mut(i),
    }
}

/// A key on the screen: true when the mod takes it. The Board Builder keeps Escape
/// (`shouldCloseOnEsc` false).
pub fn key(c: &mut Client, host: &dyn Host, key: &str) -> bool {
    if host.mod_screen() != Some(MOD_ID) || c.screen != Some(Screen::Builder) {
        return false;
    }
    match key {
        "key.keyboard.escape" => true,
        "key.keyboard.backspace" => {
            if let Some(f) = field(&mut c.builder) {
                f.pop();
            }
            if c.builder.focus == Some(Field::Search) {
                c.builder.scroll = 0.0;
            }
            true
        }
        _ => c.builder.focus.is_some(),
    }
}

pub fn typed(c: &mut Client, host: &dyn Host, ch: char) -> bool {
    if host.mod_screen() != Some(MOD_ID) || c.screen != Some(Screen::Builder) || ch.is_control() {
        return false;
    }
    let search = c.builder.focus == Some(Field::Search);
    let Some(f) = field(&mut c.builder) else { return false };
    if f.chars().count() < 32 {
        f.push(ch);
    }
    if search {
        c.builder.scroll = 0.0;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_keeps_goals_in_place() {
        let mut b = Builder {
            size: 3,
            goals: (0..9).map(|i| Some((format!("G{i}"), DATA_NONE.to_owned()))).collect(),
            ..Builder::default()
        };
        b.increment();
        assert_eq!(b.size, 4);
        assert_eq!(b.goals.len(), 16);
        assert_eq!(b.goals[4].as_ref().unwrap().0, "G3", "row 1 starts with the old row 1");
        assert!(b.goals[3].is_none() && b.goals[15].is_none());
        b.decrement();
        let ids: Vec<String> = b.goals.iter().map(|g| g.as_ref().unwrap().0.clone()).collect();
        assert_eq!(ids, (0..9).map(|i| format!("G{i}")).collect::<Vec<_>>());
    }

    #[test]
    fn saved_boards_look_like_the_java_ones() {
        let json = board_json(&[
            ("ENTER_END".into(), DATA_NONE.into()),
            ("KILL_COLORED_SHEEP".into(), "red".into()),
        ]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["goals"][0], serde_json::json!({"id": "ENTER_END"}));
        assert_eq!(v["goals"][1]["data"], "red");
        assert_eq!(
            suitable_name("Custom Board", &["Custom Board".into(), "Custom Board (1)".into()]),
            "Custom Board (2)"
        );
        assert_eq!(title_case("KILL_COLORED_SHEEP"), "Kill Colored Sheep");
    }
}
