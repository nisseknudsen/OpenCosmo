//! Player movement, faithfully ported (core cases only - bombs, damage
//! recoil, ice-slide application, dizzy/scooter states are not yet ported)
//! from cosmore's `MovePlayer()` (game1.c:8438-8822) and `TestPlayerMove()`
//! (game1.c:1022-1127). Movement is tile-grid stepped, not sub-pixel, driven
//! by a fixed-timestep tick matching the original's ~18.2Hz PC timer.

use crate::data::{
    GameData, TILE_ATTR_BLOCK_EAST, TILE_ATTR_BLOCK_NORTH, TILE_ATTR_BLOCK_SOUTH,
    TILE_ATTR_BLOCK_WEST, TILE_ATTR_CAN_CLING, TILE_ATTR_SLOPED,
};
use crate::level::{tile_topleft_to_center, CurrentLevel};
use crate::sfx::{snd, PlaySfx};
use crate::tileset::TILE_PX;
use bevy::prelude::*;

pub const PLAYER_WIDTH: i32 = 3;
pub const PLAYER_HEIGHT: i32 = 5;
const JUMP_TABLE: [i32; 10] = [-2, -1, -1, -1, -1, -1, -1, 0, 0, 0];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FaceDir {
    West,
    East,
}

#[derive(PartialEq, Eq)]
enum MoveResult {
    Free,
    Blocked,
    Sloped,
}

#[derive(Component)]
pub struct Player {
    pub x: i32, // leftmost column
    pub y: i32, // bottommost row
    pub face_dir: FaceDir,
    pub jump_time: u32,
    pub fall_time: u32,
    pub is_falling: bool,
    pub cling_dir: Option<FaceDir>,
    pub cmd_jump_latch: bool,
    pub can_cling: bool,
    pub frame: usize,
    /// Starting value 4 (game1.c:10580). HurtPlayer() decrements on
    /// contact damage; 0 triggers a respawn (our simplified stand-in for
    /// death) in enemy.rs::hazard_damage.
    pub health: i32,
    /// Invincibility frames after taking damage - 44 ticks matches the
    /// original's HurtPlayer() (game1.c:6927).
    pub hurt_cooldown: u32,
    /// 0 = alive. >0 = playing the death animation (PLAYER_DEAD_1/2,
    /// floating upward), counting ticks - mirrors playerDeadTime
    /// (game1.c:9230-9256), which respawns once it passes 36.
    pub dead_timer: u32,
    /// Shown in the status bar's "Bombs" field; stocked by collecting
    /// ACT_BOMB_IDLE and spent by `combat::place_bomb`.
    pub bombs: u32,
    /// How many health cells the meter shows - `playerHealthCells`, which
    /// starts at 3 (game1.c:10581) and grows with power-ups.
    pub health_cells: u32,
    /// `playerRecoilLeft` / `isPlayerRecoiling` - the springy upward bounce
    /// a successful pounce launches the player into (game1.c:8698-8721).
    pub recoil_left: i32,
    pub is_recoiling: bool,
    /// `clingslip` - set for the ticks where the player is sliding down a
    /// clingable-but-slippery wall. A local in the original's `MovePlayer`,
    /// but the scroll-follow that reads it lives in a different system here.
    pub cling_slip: bool,
    /// `movecount` (game1.c:8441) - drives the walk cycle at half tick rate.
    pub move_count: u32,
    /// `idlecount` (game1.c:8440) - how long the player has stood still,
    /// which schedules the blink/look-around/head-shake idle animations.
    pub idle_count: u32,
    /// `isPlayerInvincible` as a countdown. The original tracks it on the
    /// bubble actor, which sets the flag every tick it lives and clears it
    /// at 240 (game1.c:5311-5333); a counter here is the same thing without
    /// needing the actor to own player state.
    pub invincible_ticks: u32,
    /// Being shoved by something else - the pusher robot, and in the
    /// original the scooter and rocket too (`SetPlayerPush`,
    /// game1.c:8341-8360). While this is running the player's own
    /// movement commands are ignored, which is what `blockMovementCmds`
    /// does there.
    pub push: Option<Push>,
    /// A head-shake queued by something that disorients: leaving a pipe,
    /// a transporter, a hard landing. It only starts once the player is
    /// back on the ground (game1.c:9135).
    pub dizzy_queued: bool,
    /// Ticks of head-shake left. While this runs the player cannot move,
    /// act, or pounce (game1.c:8453, 6848).
    pub dizzy_left: u32,
    /// `blockActionCmds` - blocks the whole player tick without being a
    /// hold, which is how the tulip launcher and bear trap freeze you
    /// (game1.c:8453).
    pub block_action: bool,
    /// `isPlayerLongJumping` - a pounce or a shove gives a longer arc than
    /// an ordinary jump (game1.c:6862).
    pub long_jumping: bool,
    /// `pounceStreak` - ten single-recoil pounces in a row earn a bonus in
    /// the original (game1.c:6874-6880); anything else breaks the run.
    pub pounce_streak: u32,
    /// `isPounceReady` - set while the player is descending onto something
    /// (game1.c:7085); a pounce only counts toward a streak when it is set.
    pub pounce_ready: bool,
    /// `playerFallDeadTime` - the separate death animation for falling off
    /// the bottom of the map (game1.c:9166-9190).
    pub fall_dead_time: u32,
    /// Held in place by something - the bear trap. `blockMovementCmds` in
    /// the original (game1.c:7753); the player can still be hurt and can
    /// still die, they just cannot walk out.
    pub held_ticks: u32,
    /// Stand-in for `random()`, used only for idle animation jitter.
    rng: u32,
}

/// One shove in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Push {
    pub dx: i32,
    pub dy: i32,
    /// Cells moved per tick.
    pub speed: u32,
    pub time: u32,
    pub max_time: u32,
    /// Whether jumping cancels it.
    pub abortable: bool,
    /// Whether hitting something stops it.
    pub blockable: bool,
}

impl Player {
    /// `SetPlayerPush` (game1.c:8341-8360).
    pub fn set_push(&mut self, dx: i32, dy: i32, max_time: u32, speed: u32, abortable: bool, blockable: bool) {
        self.push = Some(Push {
            dx,
            dy,
            speed,
            time: 0,
            max_time,
            abortable,
            blockable,
        });
    }

    pub fn spawn_at(x: i32, y: i32) -> Self {
        Player {
            x,
            y,
            face_dir: FaceDir::East,
            jump_time: 0,
            fall_time: 0,
            is_falling: true,
            cling_dir: None,
            cmd_jump_latch: false,
            can_cling: false,
            frame: 0,
            health: 4,
            hurt_cooldown: 0,
            dead_timer: 0,
            bombs: 0,
            health_cells: 3,
            recoil_left: 0,
            is_recoiling: false,
            cling_slip: false,
            move_count: 0,
            idle_count: 0,
            invincible_ticks: 0,
            push: None,
            dizzy_queued: false,
            dizzy_left: 0,
            block_action: false,
            long_jumping: false,
            pounce_streak: 0,
            pounce_ready: false,
            fall_dead_time: 0,
            held_ticks: 0,
            rng: 0x1337_beef,
        }
    }

    fn next_rand(&mut self, modulo: u32) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        if modulo == 0 {
            0
        } else {
            self.rng % modulo
        }
    }

    /// `playerBaseFrame` - which of the two 23-frame facing blocks the
    /// local frame index is taken from (player.h:24-25). The original keeps
    /// this as its own variable but every site in `MovePlayer` moves it in
    /// lockstep with `playerFaceDir`, so it is derived here instead.
    /// `isPlayerInvincible` (game1.c:6905) - blocks all contact damage.
    pub fn is_invincible(&self) -> bool {
        self.invincible_ticks > 0
    }

    pub fn base_frame(&self) -> usize {
        match self.face_dir {
            FaceDir::West => 0,
            FaceDir::East => 23,
        }
    }

    /// `TryPounce` (game1.c:6844-6895), minus the dizzy/streak bookkeeping
    /// this remake hasn't ported. Succeeds only while descending - either
    /// genuinely falling, or past the jump curve's apex - and refuses to
    /// re-trigger mid-bounce.
    pub fn try_pounce(&mut self, recoil: i32) -> bool {
        // A dizzy player cannot pounce (game1.c:6848).
        if self.dead_timer != 0 || self.dizzy_left != 0 {
            return false;
        }
        let descending = self.is_falling || self.jump_time > 6;
        // `TryPounce` requires the alignment test to have armed it
        // (game1.c:6855). It is *not* cleared here - the alignment pass
        // re-arms it per actor, which is what lets one tick pounce two
        // things stacked on each other (game1.c:7077).
        if !self.pounce_ready {
            return false;
        }
        if (self.is_recoiling && self.recoil_left >= 2) || !descending {
            return false;
        }
        self.recoil_left = recoil + 1;
        self.is_recoiling = true;
        self.is_falling = false;
        self.fall_time = 0;
        // Landing on something shakes off a queued head-shake
        // (game1.c:6859).
        self.clear_dizzy();
        // Only the hardest recoils count as a long jump - a plain creature
        // gives 7, which does not (game1.c:6862-6866).
        self.long_jumping = recoil > 18;
        // `pounceStreak`: ten ordinary pounces in a row is worth a bonus.
        // Anything with a different recoil breaks the run.
        if recoil == 7 {
            self.pounce_streak += 1;
            if self.pounce_streak == 10 {
                self.pounce_streak = 0;
            }
        } else {
            self.pounce_streak = 0;
        }
        true
    }

    /// `ProcessPlayerDizzy` (game1.c:9122-9151). A queued head-shake waits
    /// for the player to be back on the ground before it starts, and
    /// grabbing a wall cancels it outright.
    pub fn process_dizzy(&mut self, grounded: bool) {
        if self.cling_dir.is_some() {
            self.dizzy_queued = false;
            self.dizzy_left = 0;
            return;
        }
        if self.dizzy_queued && grounded {
            self.dizzy_queued = false;
            self.dizzy_left = 8;
        }
        if self.dizzy_left != 0 {
            self.dizzy_left -= 1;
            self.is_falling = false;
        }
    }

    /// `SET_PLAYER_DIZZY` (game1.c:246).
    pub fn queue_dizzy(&mut self) {
        self.dizzy_queued = true;
    }

    /// `ClearPlayerDizzy` (game1.c:307).
    pub fn clear_dizzy(&mut self) {
        self.dizzy_queued = false;
        self.dizzy_left = 0;
    }
}

#[derive(Resource, Default, Clone)]
pub struct PlayerInput {
    pub west: bool,
    pub east: bool,
    pub jump: bool,
    pub look_up: bool,
    pub look_down: bool,
    pub bomb: bool,
    /// "Any key", for dismissing a modal frame. Only ever set by a scripted
    /// run (`k` in `COSMO_INPUT`); live play reads the keyboard directly,
    /// since a real tap can fall between two 18.2Hz ticks.
    pub dismiss: bool,
}

/// A scripted input sequence for `COSMO_INPUT`, so a mechanic can be
/// exercised deterministically instead of hoping a playthrough happens to
/// hit it. The format is comma-separated `<keys><ticks>` steps, where each
/// key is one of `w`/`e` (west/east), `u`/`d` (look up/down), `j` (jump),
/// `b` (bomb), `k` (dismiss an open text frame), or `.` (nothing):
/// `COSMO_INPUT="e40,.10,u60,d60"` walks east, stands still, then looks up
/// and back down.
pub struct InputScript(Vec<(PlayerInput, u32)>);

impl InputScript {
    pub fn parse(spec: &str) -> Self {
        let mut steps = Vec::new();
        for step in spec.split(',').filter(|s| !s.trim().is_empty()) {
            let step = step.trim();
            let split = step.find(|c: char| c.is_ascii_digit()).unwrap_or(step.len());
            let (keys, count) = step.split_at(split);
            let count: u32 = count.parse().unwrap_or(1);
            steps.push((
                PlayerInput {
                    west: keys.contains('w'),
                    east: keys.contains('e'),
                    jump: keys.contains('j'),
                    look_up: keys.contains('u'),
                    look_down: keys.contains('d'),
                    bomb: keys.contains('b'),
                    dismiss: keys.contains('k'),
                },
                count,
            ));
        }
        InputScript(steps)
    }

    /// The state at tick `t`; the final step holds once the script runs out.
    pub fn at(&self, t: u32) -> PlayerInput {
        let mut remaining = t;
        for (state, count) in &self.0 {
            if remaining < *count {
                return state.clone();
            }
            remaining -= count;
        }
        self.0.last().map(|(s, _)| s.clone()).unwrap_or_default()
    }
}

/// Collects this tick's controls.
///
/// The live path just drains `InputAccumulator`, which has been sampling
/// every frame - see `input.rs` for why the gameplay tick can't read the
/// keyboard directly.
/// Where this run's controls come from. Resolved once - probing the
/// environment every tick allocated two `String`s to answer a question
/// fixed at launch.
pub enum InputSource {
    Live,
    Script(InputScript),
    Autoplay,
}

impl InputSource {
    pub fn from_env() -> Self {
        if let Ok(spec) = std::env::var("COSMO_INPUT") {
            InputSource::Script(InputScript::parse(&spec))
        } else if std::env::var("COSMO_AUTOPLAY").is_ok() {
            InputSource::Autoplay
        } else {
            InputSource::Live
        }
    }
}

pub fn read_input(
    mut input: ResMut<PlayerInput>,
    mut accum: ResMut<crate::input::InputAccumulator>,
    mut tick: Local<u32>,
    mut source: Local<Option<InputSource>>,
) {
    let source = source.get_or_insert_with(InputSource::from_env);
    if let InputSource::Script(script) = source {
        *input = script.at(*tick);
        *tick += 1;
        return;
    }
    if matches!(source, InputSource::Autoplay) {
        *input = PlayerInput {
            east: true,
            // Tap rather than hold: the jump latch deliberately blocks a
            // re-jump until the key is released (game1.c:8793-8797), so a
            // held key would mask whether jumping is possible at all.
            jump: *tick % 12 == 0,
            bomb: *tick % 40 == 0,
            ..default()
        };
        *tick += 1;
        return;
    }
    *input = accum.take();
}

fn attr_at(level: &crate::data::LevelJson, data: &GameData, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 {
        return 0;
    }
    data.tile_attr(level.tile_at(x as usize, y as usize))
}

/// Faithful port of TestPlayerMove; `can_cling` is an out-param mirroring
/// the original's side-effecting global write during WEST/EAST checks.
/// Which way a slippery slope under the player's feet pulls them.
///
/// The original hangs this off `TestPlayerMove(DIR4_SOUTH)` as a side
/// effect and then calls that function purely for it (game1.c:8567,
/// "used for side effects"). Keeping it a query of its own says what it
/// means and leaves `test_move` answering one question.
///
/// Only the outer two of the player's three columns are consulted, and a
/// tile counts only if it is sloped, slippery, and *not* solid
/// (game1.c:1062-1073).
fn slide_dir(
    x: i32,
    y: i32,
    level: &crate::data::LevelJson,
    data: &GameData,
) -> Option<FaceDir> {
    let slippery_slope = |col: i32| {
        let a = attr_at(level, data, col, y);
        a & TILE_ATTR_BLOCK_SOUTH == 0
            && a & TILE_ATTR_SLOPED != 0
            && a & crate::data::TILE_ATTR_SLIPPERY != 0
    };
    let east = slippery_slope(x);
    let west = slippery_slope(x + PLAYER_WIDTH - 1);
    // Both at once cancel: the original's guard is
    // `if (!isPlayerSlidingEast || !isPlayerSlidingWest)` (game1.c:8568),
    // so a dip with slippery slopes on each side holds the player still.
    match (east, west) {
        (true, false) => Some(FaceDir::East),
        (false, true) => Some(FaceDir::West),
        _ => None,
    }
}

fn test_move(
    dir: Direction,
    x: i32,
    y: i32,
    level: &crate::data::LevelJson,
    data: &GameData,
    can_cling: &mut bool,
) -> MoveResult {
    match dir {
        Direction::North => {
            if y - 3 == 0 || y - 2 == 0 {
                return MoveResult::Blocked;
            }
            let row = y - 4;
            for i in 0..PLAYER_WIDTH {
                if attr_at(level, data, x + i, row) & TILE_ATTR_BLOCK_NORTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Direction::South => {
            let row = y;
            for i in 0..PLAYER_WIDTH {
                let a = attr_at(level, data, x + i, row);
                if a & TILE_ATTR_SLOPED != 0 {
                    return MoveResult::Sloped;
                }
                if a & TILE_ATTR_BLOCK_SOUTH != 0 {
                    return MoveResult::Blocked;
                }
            }
            MoveResult::Free
        }
        Direction::West => {
            *can_cling = attr_at(level, data, x, y - 2) & TILE_ATTR_CAN_CLING != 0;
            for i in 0..PLAYER_HEIGHT {
                let row = y - i;
                let a = attr_at(level, data, x, row);
                if a & TILE_ATTR_BLOCK_WEST != 0 {
                    return MoveResult::Blocked;
                }
                if i == 0
                    && a & TILE_ATTR_SLOPED != 0
                    && attr_at(level, data, x, row - 1) & TILE_ATTR_BLOCK_WEST == 0
                {
                    return MoveResult::Sloped;
                }
            }
            MoveResult::Free
        }
        Direction::East => {
            *can_cling = attr_at(level, data, x + 2, y - 2) & TILE_ATTR_CAN_CLING != 0;
            for i in 0..PLAYER_HEIGHT {
                let row = y - i;
                let a = attr_at(level, data, x + 2, row);
                if a & TILE_ATTR_BLOCK_EAST != 0 {
                    return MoveResult::Blocked;
                }
                if i == 0
                    && a & TILE_ATTR_SLOPED != 0
                    && attr_at(level, data, x + 2, row - 1) & TILE_ATTR_BLOCK_EAST == 0
                {
                    return MoveResult::Sloped;
                }
            }
            MoveResult::Free
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    North,
    South,
    West,
    East,
}

/// `MovePlayerPush` (game1.c:8375-8437). Carries the player along while a
/// shove is active, one cell at a time up to `speed` cells a tick, and
/// stops early on hitting anything if the shove is blockable.
///
/// The scroll is carried with the player, which is what keeps a shove from
/// throwing them out of the view.
fn move_player_push(
    p: &mut Player,
    scroll: &mut crate::camera::Scroll,
    level: &crate::data::LevelJson,
    data: &GameData,
    jump_pressed: bool,
    map_width: i32,
) -> bool {
    let Some(mut push) = p.push else {
        return false;
    };
    if jump_pressed && push.abortable {
        p.push = None;
        return false;
    }

    let mut dummy = false;
    let mut blocked = false;
    for _ in 0..push.speed {
        if p.x + push.dx > 0 && p.x + push.dx + 2 < map_width {
            p.x += push.dx;
        }
        p.y += push.dy;
        scroll.x += push.dx;
        scroll.y += push.dy;

        if push.blockable
            && [
                Direction::West,
                Direction::East,
                Direction::North,
                Direction::South,
            ]
            .into_iter()
            .any(|d| test_move(d, p.x, p.y, level, data, &mut dummy) != MoveResult::Free)
        {
            blocked = true;
            break;
        }
    }

    if blocked {
        // The original moves into the wall and then backs off a step
        // rather than testing ahead (game1.c:8425-8432).
        p.x -= push.dx;
        p.y -= push.dy;
        scroll.x -= push.dx;
        scroll.y -= push.dy;
        p.push = None;
    } else {
        push.time += 1;
        if push.time >= push.max_time {
            p.push = None;
        } else {
            p.push = Some(push);
        }
    }
    true
}

pub fn move_player_tick(
    mut query: Query<&mut Player>,
    input: Res<PlayerInput>,
    level_data: Res<CurrentLevel>,
    data: Res<GameData>,
    mut scroll: ResMut<crate::camera::Scroll>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer != 0 {
        return; // frozen during the death animation, game1.c:8452
    }
    let level = &level_data.level;
    // A queued head-shake starts once the player is back on the ground,
    // and runs to completion before they get control again
    // (game1.c:9122-9151).
    if p.dizzy_queued || p.dizzy_left != 0 {
        let mut ignored = false;
        let grounded = test_move(Direction::South, p.x, p.y + 1, level, &data, &mut ignored)
            != MoveResult::Free;
        p.process_dizzy(grounded);
    }

    // `blockActionCmds` and the dizzy shake both stop the whole tick, not
    // just movement (game1.c:8452-8454).
    if p.dizzy_left != 0 || p.block_action {
        return;
    }

    // Held fast by a trap: the tick still runs (gravity, damage, death)
    // but the movement commands are ignored (game1.c:7753).
    if p.held_ticks > 0 {
        p.held_ticks -= 1;
        return;
    }
    // A shove overrides the player's own commands for as long as it runs,
    // which is what `blockMovementCmds` does in the original.
    if move_player_push(
        &mut p,
        &mut scroll,
        level,
        &data,
        input.jump,
        level.width as i32,
    ) {
        return;
    }
    let mut dummy_cling = false;
    p.move_count = p.move_count.wrapping_add(1);
    p.cling_slip = false;

    // --- Holding on to a wall (game1.c:8464-8495) ---
    // A cling is not permanent: every tick re-reads the tile actually being
    // held. A wall that is *slippery* as well as clingable lets the player
    // slide down it one row per tick, and running out of clingable wall -
    // by slipping off the bottom of it, or because the tile simply stopped
    // being clingable - drops them. Without this the player stuck to the
    // wall forever, which is what "just bugging against a wall" looked like.
    if let Some(dir) = p.cling_dir {
        let (cx, cy) = match dir {
            FaceDir::West => (p.x - 1, p.y - 2),
            FaceDir::East => (p.x + 3, p.y - 2),
        };
        let target = attr_at(&level, &data, cx, cy);
        let slippery = target & crate::data::TILE_ATTR_SLIPPERY != 0;
        let clingable = target & TILE_ATTR_CAN_CLING != 0;
        if slippery && clingable {
            if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                != MoveResult::Free
            {
                p.cling_dir = None; // reached the floor
            } else {
                p.y += 1;
                p.cling_slip = true;
                let (cx, cy) = match dir {
                    FaceDir::West => (p.x - 1, p.y - 2),
                    FaceDir::East => (p.x + 3, p.y - 2),
                };
                let target = attr_at(&level, &data, cx, cy);
                if target & crate::data::TILE_ATTR_SLIPPERY == 0
                    && target & TILE_ATTR_CAN_CLING == 0
                {
                    p.cling_dir = None;
                    p.cling_slip = false;
                }
            }
        } else if !clingable {
            p.cling_dir = None;
        }
    }

    // --- Horizontal movement (game1.c:8606-8687) ---
    // Transcribed move-first-then-test, as the original does it, rather
    // than the test-first rewrite this used to be. The difference is not
    // cosmetic: testing before the step checks the column the player is
    // *standing in* instead of the one they are moving into, so walls only
    // stopped them a tile late and `canPlayerCling` was read off the
    // player's own column rather than the wall's.
    //
    // The one thing not reproduced is the original's uninitialized
    // `horizmove` when the step is refused at the map edge (flagged "WHOA
    // uninitialized" at game1.c:8637); that reads stack garbage, so this
    // treats it as MOVE_FREE.
    //
    // `southmove` is probed *before* stepping (game1.c:8607, 8648) - it is
    // what distinguishes walking down a slope from walking off a ledge.
    if input.west && p.cling_dir.is_none() && !input.east {
        let southmove =
            test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling);
        if p.face_dir == FaceDir::West {
            p.x -= 1;
        } else {
            p.face_dir = FaceDir::West;
        }
        let mut horizmove = MoveResult::Free;
        if p.x < 1 {
            p.x += 1;
        } else {
            horizmove = test_move(Direction::West, p.x, p.y, &level, &data, &mut p.can_cling);
            if horizmove == MoveResult::Blocked {
                p.x += 1;
                if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                    == MoveResult::Free
                    && p.can_cling
                {
                    sfx.write(PlaySfx(snd::PLAYER_CLING));
                    p.cling_dir = Some(FaceDir::West);
                    p.is_recoiling = false;
                    p.recoil_left = 0;
                    p.is_falling = false;
                    p.jump_time = 0;
                    p.fall_time = 0;
                    p.cmd_jump_latch = input.jump;
                }
            }
        }
        if horizmove == MoveResult::Sloped {
            p.y -= 1; // walking *up* a slope
        } else if southmove == MoveResult::Sloped
            && test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                == MoveResult::Free
        {
            // Walking *down* one (game1.c:8638-8645). Clearing the falling
            // flag here is what keeps jumping possible on a descent.
            p.is_falling = false;
            p.jump_time = 0;
            p.y += 1;
        }
    }
    if input.east && p.cling_dir.is_none() && !input.west {
        let southmove =
            test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling);
        if p.face_dir == FaceDir::East {
            p.x += 1;
        } else {
            p.face_dir = FaceDir::East;
        }
        let mut horizmove = MoveResult::Free;
        if level.width as i32 - 4 < p.x {
            p.x -= 1;
        } else {
            horizmove = test_move(Direction::East, p.x, p.y, &level, &data, &mut p.can_cling);
            if horizmove == MoveResult::Blocked {
                p.x -= 1;
                if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                    == MoveResult::Free
                    && p.can_cling
                {
                    sfx.write(PlaySfx(snd::PLAYER_CLING));
                    p.cling_dir = Some(FaceDir::East);
                    p.is_recoiling = false;
                    p.recoil_left = 0;
                    p.is_falling = false;
                    p.jump_time = 0;
                    p.fall_time = 0;
                    p.cmd_jump_latch = input.jump;
                }
            }
        }
        if horizmove == MoveResult::Sloped {
            p.y -= 1;
        } else if southmove == MoveResult::Sloped
            && test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                == MoveResult::Free
        {
            p.is_falling = false;
            p.fall_time = 0;
            p.y += 1;
        }
    } else if let Some(dir) = slide_dir(p.x, p.y + 1, &level, &data) {
        // --- Ice slide (game1.c:8565-8605) ---
        // Standing still on a slippery slope slides the player down it,
        // and the view follows so they cannot be slid off the screen.
        // Clinging to a wall overrides it - the slide only moves a player
        // whose feet are on the slope.
        if p.cling_dir.is_none() {
            p.x += if dir == FaceDir::East { 1 } else { -1 };
            if test_move(Direction::South, p.x, p.y + 1, &level, &data, &mut dummy_cling)
                == MoveResult::Free
            {
                p.y += 1;
            }
        }
        p.cling_dir = None;
    }

    if p.cling_dir.is_some() && p.cmd_jump_latch && !input.jump {
        p.cmd_jump_latch = false;
    }

    // --- Jump / recoil ---
    // A pounce puts the player into recoil, which drives the same upward
    // branch a jump does - hence the shared condition (game1.c:8691-8695).
    let jumping_now = p.recoil_left != 0
        || (input.jump && !p.is_falling && !p.cmd_jump_latch)
        || (p.cling_dir.is_some() && input.jump && !p.cmd_jump_latch);
    if jumping_now {
        let mut new_jump = true;

        if p.is_recoiling && p.recoil_left > 0 {
            // Recoil bounce (game1.c:8698-8721). It rises faster than a
            // jump while the counter is high, giving a pounce its
            // characteristic springy launch.
            p.recoil_left -= 1;
            // The long-jump flag survives only the first, fastest part of
            // the recoil (game1.c:8700-8712).
            if p.recoil_left < 10 {
                p.long_jumping = false;
            }
            if p.recoil_left > 1 {
                p.y -= 1;
            }
            if p.recoil_left > 13 {
                p.recoil_left -= 1;
                if test_move(Direction::North, p.x, p.y, &level, &data, &mut dummy_cling)
                    == MoveResult::Free
                {
                    p.y -= 1;
                } else {
                    // Hitting a ceiling ends the long jump early
                    // (game1.c:8712).
                    p.long_jumping = false;
                }
            }
            new_jump = false;
            if p.recoil_left == 0 {
                p.jump_time = 0;
                p.is_recoiling = false;
                p.fall_time = 0;
                p.long_jumping = false;
                p.cmd_jump_latch = true;
            }
        } else {
            if p.cling_dir == Some(FaceDir::West) && input.west {
                p.cling_dir = None;
            }
            if p.cling_dir == Some(FaceDir::East) && input.east {
                p.cling_dir = None;
            }
            if p.cling_dir.is_none() {
                let jt = (p.jump_time as usize).min(JUMP_TABLE.len() - 1);
                p.y = (p.y + JUMP_TABLE[jt]).max(0);
            }
            p.is_recoiling = false;
        }
        p.cling_dir = None;

        if test_move(Direction::North, p.x, p.y, &level, &data, &mut dummy_cling)
            != MoveResult::Free
        {
            if p.jump_time > 0 || p.is_recoiling {
                sfx.write(PlaySfx(snd::PLAYER_HIT_HEAD));
            }
            p.recoil_left = 0;
            p.is_recoiling = false;
            p.y += 1;
            p.is_falling = true;
            if input.jump {
                p.cmd_jump_latch = true;
            }
            p.fall_time = 0;
        } else if new_jump && p.jump_time == 0 {
            sfx.write(PlaySfx(snd::PLAYER_JUMP));
        }
        if !p.is_recoiling && p.jump_time + 1 > 6 {
            p.is_falling = true;
            if input.jump {
                p.cmd_jump_latch = true;
            }
            p.fall_time = 0;
        }
        if new_jump || !p.is_recoiling {
            p.jump_time += 1;
        }
    }

    // --- Gravity / falling ---
    if p.cling_dir.is_none() {
        if p.is_falling && input.jump {
            p.cmd_jump_latch = true;
        }
        if (!input.jump || p.cmd_jump_latch) && !p.is_falling {
            p.is_falling = true;
            p.fall_time = 0;
        }
        // Recoil owns vertical motion while it lasts (game1.c:8784).
        if p.is_falling && !p.is_recoiling {
            p.y += 1;
            if test_move(Direction::South, p.x, p.y, &level, &data, &mut dummy_cling)
                != MoveResult::Free
            {
                if p.fall_time != 0 {
                    sfx.write(PlaySfx(snd::PLAYER_LAND));
                }
                p.is_falling = false;
                p.y -= 1;
                p.jump_time = 0;
                p.cmd_jump_latch = input.jump;
                p.fall_time = 0;
            }
            if p.is_falling && p.fall_time > 3 {
                // A long fall covers two rows a tick and drags the view
                // down with it (game1.c:8802-8807), rather than waiting for
                // the follow tail's dead zone to notice.
                p.y += 1;
                scroll.y += 1;
                if test_move(Direction::South, p.x, p.y, &level, &data, &mut dummy_cling)
                    != MoveResult::Free
                {
                    if p.fall_time != 0 {
                        sfx.write(PlaySfx(snd::PLAYER_LAND));
                    }
                    p.is_falling = false;
                    p.y -= 1;
                    scroll.y -= 1;
                    p.jump_time = 0;
                    p.cmd_jump_latch = input.jump;
                    p.fall_time = 0;
                }
            }
            if p.fall_time < 25 {
                p.fall_time += 1;
            }
        }
    }

    // Falling out of the world kills rather than teleports
    // (game1.c:9165-9167): past `maxScrollY + SCROLLH + 3`, which is three
    // rows below the last row the view can ever reach. This used to
    // silently respawn the player at the level start, which skipped the
    // death entirely - no animation, no cost, and the level left as it was.
    let bottom = crate::camera::max_scroll_y(level.width) + crate::camera::SCROLL_H + 3;
    if p.y > bottom && p.dead_timer == 0 {
        p.dead_timer = 1;
        // Falling out of the world is its own death, not the ordinary one:
        // the body does not rise and the sequence runs longer
        // (game1.c:9165-9211).
        p.fall_dead_time = 1;
    }
}

/// Plays the death animation - frozen movement, alternating DEAD frames,
/// the body floating upward - then restarts the level (game1.c:9225-9258).
///
/// The restart is a full `InitializeLevel`, not a teleport: it rebuilds the
/// level from the map, so every enemy the player had already killed is back
/// and the level-entry snapshot's score/stars/bombs/health come back with
/// it. That is raised as an event so it shares one code path with the F1
/// menu's "Restart Level", exactly as the two share `LoadGameState('T');
/// InitializeLevel(levelNum);` in the original.
pub fn update_death(
    mut query: Query<&mut Player>,
    mut sfx: EventWriter<PlaySfx>,
    mut scroll: ResMut<crate::camera::Scroll>,
    mut restart: EventWriter<crate::flow::RestartLevel>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer == 0 {
        return;
    }
    if p.dead_timer == 1 {
        sfx.write(PlaySfx(snd::PLAYER_HURT));
    }
    p.dead_timer += 1;

    if p.fall_dead_time != 0 {
        // The falling death (game1.c:9177-9211). The body is already gone
        // off the bottom, so nothing rises; it is a held beat, the jingle,
        // and a restart at thirty.
        //
        // NOT PORTED: the speech bubble that rises up the screen through
        // it - see the speech bubble issue.
        p.fall_dead_time += 1;
        if p.fall_dead_time == 13 {
            sfx.write(PlaySfx(snd::PLAYER_DEATH));
        }
        if p.fall_dead_time > 30 {
            p.fall_dead_time = 0;
            p.dead_timer = 0;
            restart.write(crate::flow::RestartLevel);
        }
        return;
    }

    // The death jingle lands partway in, once the body starts rising
    // (game1.c:9243-9244).
    if p.dead_timer == 10 {
        sfx.write(PlaySfx(snd::PLAYER_DEATH));
    }
    if p.dead_timer > 10 {
        p.y -= 1;
        // The view drifts up with the body, but only for the first couple
        // of ticks (game1.c:9239-9240).
        if scroll.y > 0 && p.dead_timer < 12 {
            scroll.y -= 1;
        }
    }
    if p.dead_timer > 36 {
        p.dead_timer = 0;
        restart.write(crate::flow::RestartLevel);
    }
}

pub fn sync_transform(mut query: Query<(&Player, &mut Transform)>) {
    for (p, mut t) in &mut query {
        let top_row = p.y - (PLAYER_HEIGHT - 1);
        let world = tile_topleft_to_center(
            p.x as f32,
            top_row as f32,
            PLAYER_WIDTH as f32 * TILE_PX,
            PLAYER_HEIGHT as f32 * TILE_PX,
        );
        t.translation.x = world.x;
        t.translation.y = world.y;
        t.translation.z = 10.0;
    }
}

// Named frame offsets within one facing direction's 23-frame block
// (player.h: PLAYER_BASE_WEST=0, PLAYER_BASE_EAST=23, frames 0..22 per side).
mod frame {
    pub const WALK_1: usize = 0;
    pub const WALK_4: usize = 3;
    pub const STAND: usize = 4;
    pub const LOOK_NORTH: usize = 5;
    pub const LOOK_SOUTH: usize = 6;
    pub const JUMP: usize = 7;
    pub const FALL: usize = 8;
    pub const CLING: usize = 9;
    pub const CLING_OPPOSITE: usize = 10;
    pub const CLING_NORTH: usize = 11;
    pub const CLING_SOUTH: usize = 12;
    pub const FALL_LONG: usize = 13;
    pub const PAIN: usize = 15;
    pub const FALL_SEVERE: usize = 16;
    pub const STAND_BLINK: usize = 18;
    pub const SHAKE_1: usize = 19;
    pub const SHAKE_2: usize = 20;
    pub const SHAKE_3: usize = 21;
    pub const JUMP_LONG: usize = 22;
    /// Shared by both facings, not offset by `base` - PLAYER_DEAD_1/2 = 46/47.
    pub const DEAD_1: usize = 46;
}

/// The tail of `MovePlayer` (game1.c:8824-8949): choose this tick's sprite
/// frame, then move the view.
///
/// These are one system because they are one function in the original, and
/// not incidentally so: the look-up/look-down branch ends in a `return`
/// (game1.c:8853) that deliberately skips the scroll-follow below it. Split
/// them and the follow immediately drags the view back, which is why
/// looking around never appeared to do anything.
///
/// NOT PORTED: the `playerBombDir` crouch pose, which belongs to the bomb
/// path in `MovePlayer` rather than to `combat::place_bomb` where bombs
/// live here.
pub fn update_frame_and_scroll(
    mut query: Query<&mut Player>,
    input: Res<PlayerInput>,
    level_data: Res<CurrentLevel>,
    mut scroll: ResMut<crate::camera::Scroll>,
    near_globe: Res<crate::hints::NearHintGlobe>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Ok(mut p) = query.single_mut() else {
        return;
    };
    if p.dead_timer != 0 {
        // PLAYER_DEAD_1/2 alternate every tick, shared by both facings
        // (game1.c:9236: `PLAYER_DEAD_1 + (playerDeadTime % 2)`).
        p.frame = frame::DEAD_1 + (p.dead_timer as usize % 2);
        return;
    }
    let level = &level_data.level;
    let cling_slip = p.cling_slip;
    let clinging = p.cling_dir.is_some();

    // --- Looking around (game1.c:8827-8852) ---
    // Holding up or down while standing still walks the view through the
    // world a row per tick. That is the whole mechanic: there is no
    // separate "camera offset", just the ordinary scroll position being
    // nudged, which is why it needed the scroll to become stateful first.
    // The reward for doing it is that anything parked off-screen above
    // comes into view and starts running - actors only tick while visible
    // (`ProcessActor`, game1.c:7858-7864) - so hoards of prizes overhead
    // wake up and drop once you look up at them. Hint 5 in episode 1 is
    // literally "In high places look up to find bonus objects."
    if (input.look_up || input.look_down)
        && !input.west
        && !input.east
        && !p.is_falling
        && !input.jump
    {
        p.idle_count = 0;
        // Looking up *at a hint globe* reads the globe instead of panning;
        // `hints::read_hint_globe` handles that half.
        if input.look_up && near_globe.0.is_none() {
            if scroll.y > 0 && p.y - scroll.y < crate::camera::SCROLL_H - 1 {
                scroll.y -= 1;
            }
            if cling_slip {
                scroll.y += 1;
            }
            p.frame = if clinging {
                frame::CLING_NORTH
            } else {
                frame::LOOK_NORTH
            };
        } else if input.look_down {
            if scroll.y + 3 < p.y {
                scroll.y += 1;
                if cling_slip && scroll.y + 3 < p.y {
                    scroll.y += 1;
                }
            }
            p.frame = if clinging {
                frame::CLING_SOUTH
            } else {
                frame::LOOK_SOUTH
            };
        }
        scroll.clamp_to(level.width);
        return; // deliberately skips the scroll-follow below
    }

    if let Some(dir) = p.cling_dir {
        p.idle_count = 0;
        // Pressing away from the wall shows Cosmo reaching off it.
        let pressing_away = match dir {
            FaceDir::West => input.east,
            FaceDir::East => input.west,
        };
        p.frame = if pressing_away {
            frame::CLING_OPPOSITE
        } else {
            frame::CLING
        };
    } else if (p.is_falling && !p.is_recoiling) || (p.jump_time > 6 && !p.is_falling) {
        p.idle_count = 0;
        p.frame = if !p.is_recoiling && !p.is_falling && p.jump_time > 6 {
            frame::FALL
        } else if p.fall_time >= 10 && p.fall_time < 25 {
            frame::FALL_LONG
        } else if p.fall_time == 25 {
            // The original also sets the dizzy state here, which isn't ported.
            frame::FALL_SEVERE
        } else if !p.is_falling {
            frame::JUMP
        } else {
            frame::FALL
        };
    } else if (input.jump && !p.cmd_jump_latch) || p.is_recoiling {
        p.idle_count = 0;
        p.frame = frame::JUMP;
        if p.is_recoiling && p.recoil_left > 13 {
            // `isPlayerLongJumping`, which this approximates by the recoil
            // counter still being in its fast-rise range.
            p.frame = frame::JUMP_LONG;
        }
        if p.is_recoiling && p.recoil_left < 3 {
            p.frame = frame::FALL;
        }
    } else if input.west == input.east {
        // Standing around. Cosmo blinks at random, and if left alone long
        // enough looks up, then down, then shakes his head at you.
        let rnd = p.next_rand(50);
        p.frame = frame::STAND;
        if !input.west && !input.east && !p.is_falling {
            p.idle_count += 1;
            match p.idle_count {
                101..=109 => p.frame = frame::LOOK_NORTH,
                140..=149 => p.frame = frame::LOOK_SOUTH,
                180 => p.frame = frame::SHAKE_1,
                181 => p.frame = frame::SHAKE_2,
                182 => p.frame = frame::SHAKE_3,
                183 => p.frame = frame::SHAKE_2,
                184 => p.frame = frame::SHAKE_1,
                185 => p.idle_count = 0,
                _ => {}
            }
        }
        if p.frame != frame::LOOK_NORTH
            && p.frame != frame::LOOK_SOUTH
            && (rnd == 0 || rnd == 31)
        {
            p.frame = frame::STAND_BLINK;
        }
    } else if !p.is_falling {
        // Walking: one frame every other tick, footstep on the odd ones.
        p.idle_count = 0;
        if p.move_count % 2 != 0 {
            if p.frame % 2 != 0 {
                sfx.write(PlaySfx(snd::PLAYER_FOOTSTEP));
            }
            p.frame += 1;
        }
        if p.frame > frame::WALK_4 {
            p.frame = frame::WALK_1;
        }
    }

    scroll.follow(&p, &level, cling_slip);
}

#[derive(Resource, Default)]
pub struct PlayerFrames(pub Vec<Handle<Image>>);

impl PlayerFrames {
    pub fn load(asset_server: &AssetServer, data: &GameData) -> Self {
        let Some(manifest) = data.load_sprite_manifest("sprites/player") else {
            return Self::default();
        };
        let handles = manifest
            .frames
            .iter()
            .map(|f| asset_server.load(crate::data::asset_path(&format!("sprites/player/{}", f.file))))
            .collect();
        Self(handles)
    }
}

pub fn apply_player_frame(mut query: Query<(&Player, &mut Sprite)>, frames: Res<PlayerFrames>) {
    for (p, mut sprite) in &mut query {
        // The death and "ouch" poses are draw-time overrides in the
        // original rather than branches of the frame chain
        // (`ProcessAndDrawPlayer`, game1.c:9213-9224), so they are applied
        // here: the chain keeps running underneath, which is what lets the
        // view keep scrolling while the player is flinching.
        let index = if p.dead_timer != 0 {
            p.frame // already absolute - PLAYER_DEAD_1/2 aren't per-facing
        } else if p.hurt_cooldown > 40 {
            // Shown for the first 4 of the 44 invincibility ticks. The
            // original also flashes it solid white on the very first tick,
            // which we don't replicate (no white draw mode), just the pose.
            p.base_frame() + frame::PAIN
        } else {
            p.base_frame() + p.frame
        };
        if let Some(h) = frames.0.get(index) {
            sprite.image = h.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queued_head_shake_waits_for_the_ground() {
        let mut p = Player::spawn_at(0, 10);
        p.queue_dizzy();

        // Airborne: it stays queued rather than starting.
        for _ in 0..10 {
            p.process_dizzy(false);
        }
        assert!(p.dizzy_queued, "still waiting");
        assert_eq!(p.dizzy_left, 0);

        // Landing starts it. The original sets 8 and decrements in the
        // same pass, so it reads 7 after the tick that begins it.
        p.process_dizzy(true);
        assert!(!p.dizzy_queued);
        assert_eq!(p.dizzy_left, 7);

        let mut ticks = 1;
        while p.dizzy_left != 0 {
            p.process_dizzy(true);
            ticks += 1;
        }
        assert_eq!(ticks, 8, "the shake lasts eight ticks");
    }

    #[test]
    fn grabbing_a_wall_cancels_a_head_shake() {
        // game1.c:9130 - clinging clears both the queue and the shake.
        let mut p = Player::spawn_at(0, 10);
        p.queue_dizzy();
        p.process_dizzy(true);
        assert_ne!(p.dizzy_left, 0, "shaking");

        p.cling_dir = Some(FaceDir::West);
        p.process_dizzy(true);
        assert_eq!(p.dizzy_left, 0);
        assert!(!p.dizzy_queued);
    }

    #[test]
    fn a_dizzy_player_cannot_pounce() {
        let mut p = Player::spawn_at(0, 10);
        p.pounce_ready = true;
        p.is_falling = true;
        p.dizzy_left = 5;
        assert!(!p.try_pounce(7));

        p.dizzy_left = 0;
        assert!(p.try_pounce(7), "and can again once it passes");
    }

    #[test]
    fn a_pounce_shakes_off_a_queued_head_shake() {
        // game1.c:6859 - landing on something clears it.
        let mut p = Player::spawn_at(0, 10);
        p.pounce_ready = true;
        p.is_falling = true;
        p.queue_dizzy();
        assert!(p.try_pounce(7));
        assert!(!p.dizzy_queued);
    }

    #[test]
    fn only_a_hard_recoil_counts_as_a_long_jump() {
        // A creature gives 7 and a jump pad 40 (game1.c:6862-6866).
        let mut p = Player::spawn_at(0, 10);
        p.pounce_ready = true;
        p.is_falling = true;
        assert!(p.try_pounce(7));
        assert!(!p.long_jumping, "an ordinary pounce is not a long jump");

        let mut q = Player::spawn_at(0, 10);
        q.pounce_ready = true;
        q.is_falling = true;
        assert!(q.try_pounce(40));
        assert!(q.long_jumping, "a jump pad is");
    }

    #[test]
    fn ten_plain_pounces_make_a_streak_and_anything_else_breaks_it() {
        let mut p = Player::spawn_at(0, 10);
        for i in 1..10 {
            p.pounce_ready = true;
            p.is_falling = true;
            p.is_recoiling = false;
            p.recoil_left = 0;
            assert!(p.try_pounce(7));
            assert_eq!(p.pounce_streak, i);
        }
        // The tenth wraps the streak back to zero.
        p.pounce_ready = true;
        p.is_falling = true;
        p.is_recoiling = false;
        p.recoil_left = 0;
        assert!(p.try_pounce(7));
        assert_eq!(p.pounce_streak, 0);

        // A different recoil breaks a run in progress.
        p.pounce_ready = true;
        p.is_falling = true;
        p.is_recoiling = false;
        p.recoil_left = 0;
        assert!(p.try_pounce(7));
        assert_eq!(p.pounce_streak, 1);
        p.pounce_ready = true;
        p.is_falling = true;
        p.is_recoiling = false;
        p.recoil_left = 0;
        assert!(p.try_pounce(40));
        assert_eq!(p.pounce_streak, 0);
    }

    #[test]
    fn a_script_holds_each_step_for_its_tick_count() {
        let s = InputScript::parse("e3,.2,uk1");
        assert!(s.at(0).east && !s.at(0).look_up);
        assert!(s.at(2).east);
        assert!(!s.at(3).east, "the second step has no keys held");
        assert!(!s.at(4).east);
        assert!(s.at(5).look_up && s.at(5).dismiss, "steps can combine keys");
    }

    #[test]
    fn the_last_step_holds_once_the_script_runs_out() {
        let s = InputScript::parse("e2,w4");
        assert!(s.at(99).west);
    }

    #[test]
    fn an_empty_script_holds_nothing_down() {
        let s = InputScript::parse("");
        let at = s.at(0);
        assert!(!at.east && !at.west && !at.jump);
    }

    #[test]
    fn a_step_without_a_count_lasts_one_tick() {
        let s = InputScript::parse("j,e5");
        assert!(s.at(0).jump);
        assert!(s.at(1).east && !s.at(1).jump);
    }
}
