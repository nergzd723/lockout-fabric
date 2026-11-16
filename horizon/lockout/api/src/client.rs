//! The window's side (`LockoutClient`): what the server's payloads say, the board in the
//! corner as the Fabric client draws it, the screens (`screens.rs`), the client commands,
//! and the Java client's tweaks: toasts hidden during a match, no moving before it starts,
//! status effects left of the board, the tracking compass held still.
use crate::goals::{self, Layer};
use crate::screens::{self, Screen};
use crate::wire::{self, DraftStatus, Payload, WireTeam};
use horizon_client::mod_api::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub const MOD_ID: &str = "lockout";
/// `Constants.GUI_*`: the corner board's padding and slots, and the centred board's.
pub const GUI_PADDING: i32 = 2;
pub const GUI_PADDING_BOTTOM: i32 = 13;
pub const GUI_SLOT_SIZE: i32 = 18;
pub const GUI_CENTER_PADDING: i32 = 7;
pub const GUI_CENTER_SLOT_SIZE: i32 = 18;
pub const GUI_CENTER_HOVERED_COLOR: u32 = 0x80FF_FFFF;
const LINE_HEIGHT: i32 = 9;
const ATLAS_PNG: &[u8] = include_bytes!("../data/atlas.png");
const ATLAS_JSON: &str = include_str!("../data/atlas.json");

#[derive(Debug, Clone)]
pub struct ClientGoal {
    pub id: String,
    pub data: String,
    pub name: String,
    pub layers: Vec<Layer>,
    pub has_tooltip: bool,
    pub completed_by: Option<usize>,
}

impl ClientGoal {
    pub fn new(id: &str, data: &str) -> Self {
        let def = goals::table().get(id);
        Self {
            id: id.to_owned(),
            data: data.to_owned(),
            name: def.map_or_else(|| id.to_owned(), |d| d.name(data)),
            layers: def.map_or_else(|| vec![Layer::Item("minecraft:barrier".into())], |d| d.layers(data)),
            has_tooltip: def.is_some_and(|d| d.has_tooltip()),
            completed_by: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    pub teams: Vec<WireTeam>,
    pub goals: Vec<ClientGoal>,
    pub ticks: i64,
    pub started: bool,
    pub running: bool,
}

impl Board {
    pub fn side(&self) -> i32 {
        (self.goals.len() as f64).sqrt().round() as i32
    }
    pub fn points(&self, team: usize) -> usize {
        self.goals.iter().filter(|g| g.completed_by == Some(team)).count()
    }
}

/// `DraftStatePayload`, kept while the draft runs.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftView {
    pub teams: Vec<wire::DraftTeam>,
    pub board_size: i32,
    pub picks: Vec<(String, String, i32)>,
    pub offer: Vec<(String, String)>,
    pub current_team: i32,
    pub ticks_left: i32,
    pub pick_ticks: i32,
}

/// Something for the window's host to do on its next tick.
#[derive(Debug, Clone)]
pub enum ClientAction {
    Send(ServerboundPlayPacket),
    CloseScreen,
    Sound(&'static str),
    ActionBar(String, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoardPosition {
    Left,
    #[default]
    Right,
    /// Not drawn (`/BoardPosition hidden`, Horizon only).
    Hidden,
}

#[derive(Debug, Default)]
pub struct Client {
    pub board: Option<Board>,
    /// The board was read from this process's own server (single player, a LAN host) for
    /// want of a payload; it is read again each frame until one comes.
    pub from_local: bool,
    pub tooltips: HashMap<String, String>,
    pub version_mismatch: Option<String>,
    /// The server runs Lockout: it sent its version.
    pub lockout_server: bool,
    pub draft: Option<DraftView>,
    /// The pick count when the player closed the draft screen; it opens again on the next pick.
    pub draft_dismissed_at: Option<usize>,
    pub position: BoardPosition,
    /// Client ticks since the game started (`LockoutClient.CURRENT_TICK`), for cycling icons.
    pub tick: u64,
    /// The local player's entity id (the `Login` packet) and name, found from it.
    pub entity_id: Option<i32>,
    pub name: Option<String>,
    /// A custom payload packet heard from the server, to build ours from: mods name the
    /// serverbound packet's type only through it.
    pub template: Option<ServerboundPlayPacket>,
    pub screen: Option<Screen>,
    pub builder: screens::Builder,
    /// Stacks built from SNBT, by text.
    pub stacks: HashMap<String, Option<ItemStack>>,
    pub atlas: Option<Atlas>,
    atlas_uploaded: bool,
    /// What the screen hooks (which only read the host) leave for the next window tick.
    pub later: Vec<ClientAction>,
    /// The random colours and pieces the cycling icons show now.
    dyed: (u64, i32),
    glint: (u64, usize),
}

/// The atlas's sprites by key: `[x, y, w, h]` in its pixels.
#[derive(Debug, Clone)]
pub struct Atlas {
    pub sprites: HashMap<String, [f32; 4]>,
}

fn atlas() -> Atlas {
    let v: serde_json::Value = serde_json::from_str(ATLAS_JSON).expect("atlas.json is valid");
    let sprites = v["sprites"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, r)| {
                    let r = r.as_array()?;
                    let n = |i: usize| r.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                    Some((k.clone(), [n(0), n(1), n(2), n(3)]))
                })
                .collect()
        })
        .unwrap_or_default();
    Atlas { sprites }
}

/// The atlas's pixels, padded to Horizon's 1024x1024 mod image.
fn atlas_rgba() -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ATLAS_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let size = 1024u32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..info.height.min(size) {
        let from = (y * info.line_size as u32) as usize;
        let to = (y * size * 4) as usize;
        let len = (info.width.min(size) * 4) as usize;
        out[to..to + len].copy_from_slice(&buf[from..from + len]);
    }
    Some((size, size, out))
}

/// `config/lockout.json`, shared with the Fabric mod.
pub fn config_path(host: &dyn Host) -> Option<PathBuf> {
    host.config_dir().ok().map(|d| d.join("lockout.json"))
}

pub fn read_config(host: &dyn Host) -> serde_json::Value {
    config_path(host)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Sets `key` in `config/lockout.json`, keeping the rest, as `LockoutConfig.save` writes it.
pub fn write_config(host: &dyn Host, key: &str, value: serde_json::Value) {
    let mut config = read_config(host);
    if let Some(o) = config.as_object_mut() {
        o.insert(key.to_owned(), value);
    }
    if let (Some(path), Ok(text)) = (config_path(host), serde_json::to_string_pretty(&config)) {
        let _ = std::fs::write(path, text);
    }
}

impl Client {
    /// Disconnected: everything of the server goes; the Board Builder's board and the atlas
    /// stay, as the Java client's `BoardBuilderData` does.
    pub fn reset(&mut self) {
        let builder = std::mem::take(&mut self.builder);
        let (tick, atlas, uploaded) = (self.tick, self.atlas.take(), self.atlas_uploaded);
        *self = Self {
            builder,
            tick,
            atlas,
            atlas_uploaded: uploaded,
            ..Self::default()
        };
    }

    /// Whether the local player is on one of the board's teams (`amIPlayingLockout`).
    pub fn playing(&self) -> bool {
        match (&self.board, &self.name) {
            (Some(b), Some(n)) => b.teams.iter().any(|t| t.players.iter().any(|p| p == n)),
            _ => false,
        }
    }

    fn my_team(&self) -> Option<usize> {
        let name = self.name.as_ref()?;
        self.board.as_ref()?.teams.iter().position(|t| t.players.contains(name))
    }

    /// The server packet to send `payload` in, once the server has sent us one to copy.
    pub fn packet(&self, payload: &Payload) -> Option<ServerboundPlayPacket> {
        reply(self.template.as_ref()?, payload.channel(), payload.encode())
    }

    /// Applies one of the server's payloads; true when the screen should change.
    pub fn apply(&mut self, payload: Payload) {
        match payload {
            Payload::GoalsTeams { teams, goals, running } => {
                let goals = goals
                    .into_iter()
                    .map(|g| {
                        let mut goal = ClientGoal::new(&g.id, &g.data);
                        goal.completed_by = (g.completed_by >= 0).then_some(g.completed_by as usize);
                        goal
                    })
                    .collect();
                self.board = Some(Board {
                    teams,
                    goals,
                    ticks: 0,
                    started: false,
                    running,
                });
            }
            Payload::Start => {
                if let Some(b) = &mut self.board {
                    b.started = true;
                }
            }
            Payload::UpdateTimer { ticks } => {
                if let Some(b) = &mut self.board {
                    b.ticks = ticks;
                }
            }
            Payload::CompleteTask { goal, team } => {
                if let Some(b) = &mut self.board
                    && let Some(g) = b.goals.iter_mut().find(|g| g.id == goal)
                {
                    g.completed_by = (team >= 0 && (team as usize) < b.teams.len()).then_some(team as usize);
                }
            }
            Payload::End { .. } => {
                if let Some(b) = &mut self.board {
                    b.running = false;
                }
            }
            Payload::UpdateTooltip { goal, tooltip } => {
                self.tooltips.insert(goal, tooltip);
            }
            Payload::DraftState {
                status,
                teams,
                board_size,
                picks,
                offer,
                current_team,
                ticks_left,
                pick_ticks,
            } => {
                if status == DraftStatus::Running {
                    self.draft = Some(DraftView {
                        teams,
                        board_size,
                        picks,
                        offer,
                        current_team,
                        ticks_left,
                        pick_ticks,
                    });
                } else {
                    self.draft = None;
                    self.draft_dismissed_at = None;
                }
            }
            Payload::Version { .. } | Payload::CustomBoard { .. } | Payload::DraftAction { .. } => {}
        }
    }

    /// The draft's team of the local player.
    pub fn my_draft_team(&self) -> Option<usize> {
        let name = self.name.as_ref()?;
        self.draft
            .as_ref()?
            .teams
            .iter()
            .position(|t| t.team.players.contains(name))
    }
}

/// A serverbound custom payload built from a packet the server sent (see `template`).
pub fn reply(template: &ServerboundPlayPacket, channel: &str, data: Vec<u8>) -> Option<ServerboundPlayPacket> {
    match template.clone() {
        ServerboundPlayPacket::CustomPayload(mut p) => {
            p.channel = channel.into();
            p.data = data;
            Some(ServerboundPlayPacket::CustomPayload(p))
        }
        _ => None,
    }
}

/// A plugin message from the server. A Fabric Lockout server sends its version first and
/// drops players who don't answer it before the next ping, so the answer goes out from
/// here, before the game handles the packets that follow.
pub fn packet(c: &mut Client, host: &mut dyn Host, channel: &str, data: &[u8]) {
    let Some(Ok(payload)) = Payload::decode(channel, data) else {
        return;
    };
    if matches!(payload, Payload::GoalsTeams { .. }) {
        c.from_local = false;
    }
    match &payload {
        Payload::Version { version } => {
            c.lockout_server = true;
            let channels: Vec<&str> = wire::SERVERBOUND.iter().chain(wire::CLIENTBOUND).copied().collect();
            if let Some(t) = &c.template {
                if let Some(p) = reply(t, "minecraft:register", wire::register_body(&channels)) {
                    let _ = host.send_packet(p);
                }
                if let Some(p) = c.packet(&Payload::Version {
                    version: wire::LOCKOUT_VERSION.into(),
                }) {
                    let _ = host.send_packet(p);
                }
            }
            if version != wire::LOCKOUT_VERSION {
                c.version_mismatch = Some(version.clone());
                host.client_action_bar(
                    Text::colored(
                        format!(
                            "Wrong Lockout version: v{}. Server is using Lockout v{version}.",
                            wire::LOCKOUT_VERSION
                        ),
                        0xFF5555,
                    ),
                    false,
                );
            }
            // `LockoutConfig.load()` on every join.
            load_position(c, host);
            return;
        }
        Payload::GoalsTeams { .. } => {
            c.apply(payload);
            // A new board opens the board screen.
            screens::open(c, host, Screen::Board);
            return;
        }
        Payload::Start => {
            c.apply(payload);
            if c.screen.is_some() {
                screens::close(c, host);
            }
            return;
        }
        Payload::CompleteTask { team, .. } if *team >= 0 => {
            let mine = c.my_team();
            let playing = c.playing();
            c.apply(payload.clone());
            if playing && let Some(p) = host.player() {
                let sound = if mine == Some(*team as usize) {
                    "minecraft:block.note_block.chime"
                } else {
                    "minecraft:entity.guardian.death"
                };
                host.play_local_sound(sound, [p.x, p.y, p.z], 2.0, 1.0, 0);
            }
            return;
        }
        Payload::End { winners, .. } => {
            let mine = c.my_team();
            let won = mine.is_some_and(|t| winners.contains(&(t as i32)));
            c.apply(payload.clone());
            if c.name.is_some()
                && let Some(p) = host.player()
            {
                let sound = if won {
                    "minecraft:entity.pillager.celebrate"
                } else {
                    "minecraft:entity.warden.death"
                };
                host.play_local_sound(sound, [p.x, p.y, p.z], 2.0, 1.0, 0);
            }
            return;
        }
        Payload::DraftState { status, picks, .. } => {
            let running = *status == DraftStatus::Running;
            let picked = picks.len();
            c.apply(payload.clone());
            if !running {
                if c.screen == Some(Screen::Draft) {
                    screens::close(c, host);
                }
            } else if c.screen.is_none() && host.mod_screen().is_none() && c.draft_dismissed_at != Some(picked) {
                screens::open(c, host, Screen::Draft);
            }
            return;
        }
        _ => {}
    }
    c.apply(payload);
}

pub fn load_position(c: &mut Client, host: &dyn Host) {
    let config = read_config(host);
    c.position = match config.get("board position").and_then(|v| v.as_str()) {
        Some("left") => BoardPosition::Left,
        Some("hidden") => BoardPosition::Hidden,
        _ => BoardPosition::Right,
    };
}

/// The window's tick: the clock, the atlas, the local player's name.
pub fn tick(c: &mut Client, host: &mut dyn Host) {
    c.tick += 1;
    for action in std::mem::take(&mut c.later) {
        match action {
            ClientAction::Send(p) => {
                let _ = host.send_packet(p);
            }
            ClientAction::CloseScreen => screens::close(c, host),
            ClientAction::Sound(sound) => {
                if let Some(p) = host.player() {
                    host.play_local_sound(sound, [p.x, p.y, p.z], 1.0, 1.0, 0);
                }
            }
            ClientAction::ActionBar(text, color) => host.client_action_bar(Text::colored(text, color), false),
        }
    }
    if !c.atlas_uploaded {
        c.atlas_uploaded = true;
        c.atlas = Some(atlas());
        if let Some((w, h, rgba)) = atlas_rgba() {
            host.mod_image(w, h, &rgba);
        }
    }
    // The local player's name (`amIPlayingLockout` compares it with the teams'). The client's
    // entity list holds the others only; `entity_name` reaches this window's own player.
    if c.name.is_none()
        && let Some(id) = c.entity_id
    {
        c.name = host.entity(id as u32).and_then(|e| host.entity_name(e)).or_else(|| {
            host.client_entities()
                .into_iter()
                .find(|e| e.id as i32 == id)
                .and_then(|e| e.player_name)
        });
    }
    if let Some(d) = &mut c.draft {
        d.ticks_left = (d.ticks_left - 1).max(0);
    }
}

/// The resources reloaded: the atlas has to go up again.
pub fn resources_reloaded(c: &mut Client) {
    c.atlas_uploaded = false;
}

/// `mm:ss`, or `hh:mm:ss` past an hour, as `Utility.ticksToTimer`.
pub fn timer(ticks: i64) -> String {
    let t = ticks.abs() / 20;
    let (h, m, s) = ((t / 3600) % 24, (t / 60) % 60, t % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | rgb
}

/// Draws into one HUD batch, in order: what is drawn later is on top.
pub struct Painter<'a> {
    pub host: &'a dyn Host,
    pub batch: &'a mut HudBatch,
}

impl Painter<'_> {
    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        self.host
            .gui_paint(self.batch, &[GuiDraw::Fill { x0, y0, x1, y1, color }]);
    }
    /// Text with the Java mod's `§` colour codes.
    pub fn text(&mut self, s: &str, x: i32, y: i32, color: u32, shadow: bool) {
        self.host.gui_paint(
            self.batch,
            &[GuiDraw::Text {
                text: Text::legacy(s),
                x,
                y,
                color,
                shadow,
            }],
        );
    }
    pub fn width(&self, s: &str) -> i32 {
        self.host.text_width(&Text::legacy(s).plain()) as i32
    }
    pub fn item(&mut self, stack: &ItemStack, x: i32, y: i32, count: Option<&str>) {
        self.host
            .gui_draw_item_with(self.batch, stack, [x as f32, y as f32], count);
    }
    pub fn sprite(&mut self, atlas: &Atlas, key: &str, dst: [f32; 4]) -> bool {
        match atlas.sprites.get(key) {
            Some(src) => self.host.sprite(self.batch, crate::client::MOD_IMAGE, *src, dst),
            None => false,
        }
    }
    /// A nine-slice GUI sprite with `"stretch_inner": false`, as both Lockout frames' `.mcmeta`
    /// declare (`GuiGraphics.blitNineSlicedSprite`): the corners as they are, the edges and the
    /// middle tiled, a last partial tile cut from the tile's top left (`blitTiledSprite`).
    pub fn nine_slice(&mut self, atlas: &Atlas, key: &str, rect: [i32; 4], border: [i32; 4]) {
        let Some(&[sx, sy, sw, sh]) = atlas.sprites.get(key) else {
            return;
        };
        let [x, y, w, h] = rect.map(|v| v as f32);
        let [l, t, r, b] = border.map(|v| v as f32);
        // (source offset, source size, destination offset, destination size) per column and row.
        let cols = [(0.0, l, 0.0, l), (l, sw - l - r, l, w - l - r), (sw - r, r, w - r, r)];
        let rows = [(0.0, t, 0.0, t), (t, sh - t - b, t, h - t - b), (sh - b, b, h - b, b)];
        for (su, tile_w, du, dw) in cols {
            for (sv, tile_h, dv, dh) in rows {
                if tile_w <= 0.0 || tile_h <= 0.0 || dw <= 0.0 || dh <= 0.0 {
                    continue;
                }
                let mut i = 0.0;
                while i < dw {
                    let pw = tile_w.min(dw - i);
                    let mut j = 0.0;
                    while j < dh {
                        let ph = tile_h.min(dh - j);
                        self.host.sprite(
                            self.batch,
                            MOD_IMAGE,
                            [sx + su, sy + sv, pw, ph],
                            [x + du + i, y + dv + j, pw, ph],
                        );
                        j += tile_h;
                    }
                    i += tile_w;
                }
            }
        }
    }
    pub fn tooltip(&mut self, lines: &[String], at: [i32; 2]) {
        let lines: Vec<Text> = lines.iter().map(|l| Text::legacy(l)).collect();
        self.host.gui_tooltip(self.batch, &lines, [at[0] as f32, at[1] as f32]);
    }
}

/// Horizon's sprite name for a mod's own image (`hud::MOD_IMAGE_SPRITE`).
pub const MOD_IMAGE: &str = "horizon/modimage";

/// A goal's square at `(x, y)`, as its Java class draws it (`Goal.render`).
pub fn draw_goal(c: &mut Client, p: &mut Painter<'_>, goal: &ClientGoal, x: i32, y: i32) {
    let tick = c.tick;
    for layer in goal.layers.clone() {
        match layer {
            Layer::Item(id) => {
                if let Some(stack) = stack(c, p.host, &id) {
                    p.item(&stack, x, y, None);
                }
            }
            Layer::CycleItems(ids) if !ids.is_empty() => {
                let id = &ids[(tick % (60 * ids.len() as u64) / 60) as usize];
                if let Some(stack) = stack(c, p.host, id) {
                    p.item(&stack, x, y, None);
                }
            }
            Layer::Texture(key) => {
                if let Some(a) = &c.atlas {
                    p.sprite(a, &key, [x as f32, y as f32, 16.0, 16.0]);
                }
            }
            Layer::CycleTextures(keys) if !keys.is_empty() => {
                let key = &keys[(tick % (60 * keys.len() as u64) / 60) as usize];
                if let Some(a) = &c.atlas {
                    p.sprite(a, key, [x as f32, y as f32, 16.0, 16.0]);
                }
            }
            Layer::Count(text) => {
                // `drawStackCount`: right-aligned in the corner, white with a shadow.
                let w = p.width(&text);
                p.text(&text, x + 19 - 2 - w, y + 6 + 3, 0xFFFF_FFFF, true);
            }
            Layer::DyedCycle(ids) if !ids.is_empty() => {
                let change = tick / 60;
                if c.dyed.0 != change || c.dyed.1 == 0 {
                    c.dyed = (change, (random(change) & 0xFF_FFFF) as i32 | 1);
                }
                let id = &ids[(tick % (60 * ids.len() as u64) / 60) as usize];
                let snbt = format!(
                    "{{id:\"{id}\",count:1,components:{{\"minecraft:dyed_color\":{}}}}}",
                    c.dyed.1
                );
                if let Some(stack) = stack(c, p.host, &snbt) {
                    p.item(&stack, x, y, None);
                }
            }
            Layer::GlintCycle(sets) if !sets.is_empty() => {
                let set = &sets[(tick % 240 / 60) as usize % sets.len()];
                let change = tick / 60;
                if c.glint.0 != change || c.glint == (0, 0) {
                    c.glint = (change, random(change) as usize);
                }
                if set.is_empty() {
                    continue;
                }
                let id = &set[c.glint.1 % set.len()];
                let snbt =
                    format!("{{id:\"{id}\",count:1,components:{{\"minecraft:enchantment_glint_override\":true}}}}");
                if let Some(stack) = stack(c, p.host, &snbt) {
                    p.item(&stack, x, y, None);
                }
            }
            _ => {}
        }
    }
}

fn random(seed: u64) -> u64 {
    let mut r = crate::board::Rng::new(seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
    r.next_u64()
}

/// The stack for an item id or SNBT, cached.
pub fn stack(c: &mut Client, host: &dyn Host, id: &str) -> Option<ItemStack> {
    if !id.starts_with('{') {
        return Some(ItemStack::new(id, 1));
    }
    c.stacks
        .entry(id.to_owned())
        .or_insert_with(|| host.item_from_snbt(id))
        .clone()
}

/// The board in the corner (`Utility.drawBingoBoard`).
pub fn draw_hud(c: &mut Client, host: &dyn Host, batch: &mut HudBatch) {
    let Some(board) = c.board.clone() else { return };
    let state = host.client_state();
    // Left of the screen, the board is hidden while the debug screen is open.
    // The Java client hides a left board under the debug screen, whose right column its
    // `DebugHudMixin` takes away; Horizon's right column stays, so the board hides on either side.
    if c.position == BoardPosition::Hidden || state.as_ref().is_some_and(|s| s.debug_screen) {
        return;
    }
    let side = board.side();
    if side == 0 {
        return;
    }
    let gui_width = state.as_ref().map_or(batch.width, |s| s.gui_size[0]) as i32;
    let width = 2 * GUI_PADDING + side * GUI_SLOT_SIZE;
    let height = GUI_PADDING + GUI_PADDING_BOTTOM + side * GUI_SLOT_SIZE;
    let right = if c.position == BoardPosition::Left {
        width
    } else {
        gui_width
    };
    let left = right - width;
    let mut p = Painter { host, batch };
    if let Some(a) = &c.atlas {
        p.nine_slice(a, "gui/sprites/gui", [left, 0, width, height], [2, 2, 2, 13]);
    }
    let start_x = left + GUI_PADDING + 1;
    let mut y = GUI_PADDING + 1;
    for row in 0..side {
        let mut x = start_x;
        for col in 0..side {
            if let Some(goal) = board.goals.get((row * side + col) as usize) {
                if let Some(team) = goal.completed_by.and_then(|t| board.teams.get(t)) {
                    p.fill(x, y, x + 16, y + 16, argb(wire::color_rgb(&team.color)));
                }
                draw_goal(c, &mut p, goal, x, y);
            }
            x += GUI_SLOT_SIZE;
        }
        y += GUI_SLOT_SIZE;
    }
    let x = start_x + 2;
    let y = y + 1;
    let points: Vec<String> = board
        .teams
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}{}§r", legacy(&t.color), board.points(i)))
        .collect();
    p.text(&points.join("§r§7-"), x, y, 0xFF00_0000, true);
    let clock = timer(board.ticks);
    p.text(&format!("§f{clock}"), right - p.width(&clock) - 4, y, 0xFF00_0000, true);
    let names: Vec<String> = board
        .teams
        .iter()
        .flat_map(|t| t.players.iter().map(move |n| format!("{}{n}", legacy(&t.color))))
        .collect();
    let widest = names.iter().map(|n| p.width(n)).max().unwrap_or(0);
    let mut y = y + 20;
    if names.is_empty() {
        return;
    }
    match c.position {
        BoardPosition::Right => {
            p.fill(
                gui_width - widest - 3 - 1,
                y - 2,
                gui_width - 1,
                y + names.len() as i32 * LINE_HEIGHT + 1,
                0x8000_0000,
            );
            for n in &names {
                p.text(n, gui_width - p.width(n) - 2, y, 0xFF00_0000, true);
                y += LINE_HEIGHT;
            }
        }
        BoardPosition::Left => {
            p.fill(
                1,
                y - 2,
                widest + 3 + 1,
                y + names.len() as i32 * LINE_HEIGHT + 1,
                0x8000_0000,
            );
            for n in &names {
                p.text(n, 3, y, 0xFF00_0000, true);
                y += LINE_HEIGHT;
            }
        }
        BoardPosition::Hidden => {}
    }
}

/// `§c` for a team colour's name.
pub fn legacy(color: &str) -> String {
    let i = wire::TEAM_COLORS.iter().position(|(n, _)| *n == color).unwrap_or(15);
    crate::server::legacy_color(i)
}

/// The board's width when it sits on the right: the status effects move left of it
/// (`InGameHudMixin`).
pub fn effects_offset(c: &Client) -> Option<f32> {
    let board = c.board.as_ref()?;
    (c.position == BoardPosition::Right).then(|| (2 * GUI_PADDING + board.side() * GUI_SLOT_SIZE) as f32)
}

/// `InputMixin`: a lockout player can't move (but can sneak) until the match starts.
pub fn hold_still(c: &Client) -> bool {
    c.board.as_ref().is_some_and(|b| b.running && !b.started) && c.playing()
}

/// The client commands (`/BoardPosition`, `/BoardBuilder`, `/SetCustomBoard`).
pub const COMMANDS: [&str; 3] = ["BoardPosition", "BoardBuilder", "SetCustomBoard"];

pub fn command(c: &mut Client, host: &mut dyn Host, line: &str) -> bool {
    let line = line.trim_start_matches('/');
    let (name, arg) = line.split_once(' ').map_or((line, ""), |(n, a)| (n, a.trim()));
    let say = |host: &mut dyn Host, text: &str, color: u32| host.client_action_bar(Text::colored(text, color), false);
    match name {
        "BoardPosition" => {
            let position = match arg {
                "left" => BoardPosition::Left,
                "right" => BoardPosition::Right,
                "hidden" => BoardPosition::Hidden,
                _ => {
                    say(host, "Invalid board position.", 0xFF5555);
                    return true;
                }
            };
            c.position = position;
            write_config(host, "board position", serde_json::Value::from(arg));
            let line = if position == BoardPosition::Hidden {
                "Board hidden. /BoardPosition left or right shows it again.".to_owned()
            } else {
                "Updated board position. Note: Opening debug hud (F3) will hide the board.".to_owned()
            };
            say(host, &line, 0xFFFFFF);
            true
        }
        "BoardBuilder" => {
            if !arg.is_empty() {
                match screens::read_board(host, arg) {
                    Ok(goals) => {
                        let size = (goals.len() as f64).sqrt() as usize;
                        if size * size != goals.len() || !(3..=7).contains(&size) {
                            say(host, "Board doesn't have a valid number of goals!", 0xFF5555);
                            return true;
                        }
                        c.builder.set_board(arg, size, &goals);
                    }
                    Err(_) => {
                        say(host, "Error while trying to read board.", 0xFF5555);
                        return true;
                    }
                }
            }
            screens::open(c, host, Screen::Builder);
            true
        }
        "SetCustomBoard" => {
            match screens::read_board(host, arg) {
                Ok(goals) => {
                    let size = (goals.len() as f64).sqrt() as usize;
                    if size * size != goals.len() || !(3..=7).contains(&size) {
                        say(host, "Board doesn't have a valid number of goals!", 0xFF5555);
                    } else if let Some(p) = c.packet(&Payload::CustomBoard { goals: Some(goals) }) {
                        let _ = host.send_packet(p);
                    }
                }
                Err(_) => say(host, "Error while trying to read board.", 0xFF5555),
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::WireGoal;

    /// Records the sprites drawn.
    #[derive(Default)]
    struct Sprites(std::cell::RefCell<Vec<([f32; 4], [f32; 4])>>);

    impl Host for Sprites {
        fn sprite(&self, _batch: &mut HudBatch, _name: &str, src: [f32; 4], dst: [f32; 4]) -> bool {
            self.0.borrow_mut().push((src, dst));
            true
        }
        fn client_state(&self) -> Option<NativeClientState> {
            None
        }
    }

    #[test]
    fn the_hud_frame_tiles_its_slots_as_java_does() {
        let host = Sprites::default();
        let mut batch = HudBatch::new([1920, 1080]);
        let atlas = Atlas {
            sprites: [("gui/sprites/gui".to_string(), [0.0, 0.0, 58.0, 69.0])].into(),
        };
        let mut p = Painter {
            host: &host,
            batch: &mut batch,
        };
        // A 5x5 board's frame, 94 by 105, from the 58 by 69 sprite with a 3-slot middle.
        p.nine_slice(&atlas, "gui/sprites/gui", [0, 0, 94, 105], [2, 2, 2, 13]);
        let drawn = host.0.borrow();
        // `stretch_inner: false`: the middle is a full 54-pixel tile (three slots) and a cut
        // 36-pixel one (two), each way, so the slot lines fall every 18 pixels.
        let middle: Vec<[f32; 4]> = drawn
            .iter()
            .filter(|(s, _)| s[0] == 2.0 && s[1] == 2.0)
            .map(|(s, d)| [s[2], s[3], d[0], d[1]])
            .collect();
        assert_eq!(
            middle,
            vec![
                [54.0, 54.0, 2.0, 2.0],
                [54.0, 36.0, 2.0, 56.0],
                [36.0, 54.0, 56.0, 2.0],
                [36.0, 36.0, 56.0, 56.0]
            ]
        );
        // Nothing is stretched.
        assert!(drawn.iter().all(|(s, d)| s[2] == d[2] && s[3] == d[3]));
    }

    #[test]
    fn a_hidden_board_draws_nothing() {
        let host = Sprites::default();
        let mut batch = HudBatch::new([1920, 1080]);
        let mut c = Client {
            atlas: Some(Atlas {
                sprites: [("gui/sprites/gui".to_string(), [0.0, 0.0, 58.0, 69.0])].into(),
            }),
            ..Client::default()
        };
        c.apply(Payload::GoalsTeams {
            teams: vec![WireTeam {
                color: "red".into(),
                players: vec!["Alex".into()],
            }],
            goals: (0..9)
                .map(|_| WireGoal {
                    id: "ENTER_END".into(),
                    data: "null".into(),
                    completed_by: -1,
                })
                .collect(),
            running: true,
        });
        assert_eq!(effects_offset(&c), Some(58.0));
        c.position = BoardPosition::Hidden;
        draw_hud(&mut c, &host, &mut batch);
        assert!(host.0.borrow().is_empty());
        assert_eq!(effects_offset(&c), None);
    }

    #[test]
    fn the_timer_reads_like_the_java_one() {
        assert_eq!(timer(-1200), "01:00");
        assert_eq!(timer(20 * 3725), "01:02:05");
    }

    #[test]
    fn payloads_build_and_update_the_board() {
        let mut c = Client::default();
        c.apply(Payload::GoalsTeams {
            teams: vec![
                WireTeam {
                    color: "red".into(),
                    players: vec!["Alex".into()],
                },
                WireTeam {
                    color: "blue".into(),
                    players: vec!["Steve".into()],
                },
            ],
            goals: vec![
                WireGoal {
                    id: "KILL_COLORED_SHEEP".into(),
                    data: "lime".into(),
                    completed_by: -1,
                },
                WireGoal {
                    id: "ENTER_NETHER".into(),
                    data: "null".into(),
                    completed_by: 1,
                },
                WireGoal {
                    id: "SOME_GOAL_FROM_A_NEWER_MOD".into(),
                    data: "null".into(),
                    completed_by: -1,
                },
                WireGoal {
                    id: "KILL_7_UNIQUE_HOSTILE_MOBS".into(),
                    data: "null".into(),
                    completed_by: -1,
                },
            ],
            running: true,
        });
        c.apply(Payload::CompleteTask {
            goal: "KILL_COLORED_SHEEP".into(),
            team: 0,
        });
        let b = c.board.as_ref().unwrap();
        assert_eq!(b.side(), 2);
        assert_eq!((b.points(0), b.points(1)), (1, 1));
        assert_eq!(b.goals[0].name, "Kill Lime Sheep");
        assert_eq!(b.goals[2].layers, vec![Layer::Item("minecraft:barrier".into())]);
        c.apply(Payload::CompleteTask {
            goal: "ENTER_NETHER".into(),
            team: -1,
        });
        assert_eq!(c.board.as_ref().unwrap().points(1), 0);
        c.name = Some("Alex".into());
        assert!(c.playing() && hold_still(&c));
        c.apply(Payload::Start);
        assert!(!hold_still(&c));
    }

    #[test]
    fn the_atlas_decodes() {
        let (w, h, rgba) = atlas_rgba().expect("atlas.png decodes");
        assert_eq!((w, h, rgba.len()), (1024, 1024, 1024 * 1024 * 4));
        assert!(atlas().sprites.contains_key("gui/sprites/gui"));
    }
}
