//! Per-tick state dump for verifying mechanics without a human at the
//! keyboard. `COSMO_TRACE=<n>` prints every nth tick; combined with
//! `COSMO_INPUT` (a scripted key sequence) and `COSMO_QUIT_AFTER=<ticks>` a
//! behaviour can be exercised and checked from a single command.

use crate::camera::Scroll;
use crate::enemy_ai::Enemy;
use crate::player::{FaceDir, Player};
use bevy::prelude::*;

/// Every `COSMO_*` debug hook, resolved once at startup.
///
/// These used to be read with `std::env::var` inside the systems - and two
/// of them inside *run conditions*, which Bevy evaluates every frame. Each
/// call allocates a `String` and asks the OS for the environment, to answer
/// a question whose answer cannot change while the process is alive.
#[derive(Resource, Default)]
pub struct Hooks {
    pub trace_every: Option<u32>,
    pub watch_column: Option<i32>,
    pub shot_path: Option<String>,
    pub shot_at: u32,
    pub shot_raw: bool,
    pub fps: bool,
    pub motion_log: bool,
    pub quit_after: Option<u32>,
}

impl Hooks {
    pub fn from_env() -> Self {
        fn var(name: &str) -> Option<String> {
            std::env::var(name).ok()
        }
        fn num<T: std::str::FromStr>(name: &str) -> Option<T> {
            var(name).and_then(|v| v.trim().parse().ok())
        }
        Hooks {
            trace_every: var("COSMO_TRACE").map(|v| v.trim().parse().unwrap_or(1).max(1)),
            watch_column: num::<i32>("COSMO_WATCH"),
            shot_path: var("COSMO_SHOT"),
            shot_at: num::<u32>("COSMO_SHOT_AT").unwrap_or(30),
            shot_raw: var("COSMO_SHOT_RAW").is_some(),
            fps: var("COSMO_FPS").is_some(),
            motion_log: var("COSMO_MOTION").is_some(),
            quit_after: num::<u32>("COSMO_QUIT_AFTER"),
        }
    }
}

pub fn trace_enabled(hooks: Res<Hooks>) -> bool {
    hooks.trace_every.is_some()
}

pub fn trace_tick(
    player_q: Query<&Player>,
    enemies: Query<&Enemy>,
    containers: Query<&crate::actors::Container>,
    pedestals: Query<&crate::actors::Pedestal>,
    scroll: Res<Scroll>,
    near_globe: Res<crate::hints::NearHintGlobe>,
    paused: Res<crate::help::Paused>,
    stars: Res<crate::flow::Stars>,
    score: Res<crate::flow::Score>,
    hooks: Res<Hooks>,
    mut tick: Local<u32>,
) {
    *tick += 1;
    if *tick % hooks.trace_every.unwrap_or(1) != 0 {
        return;
    }
    let Ok(p) = player_q.single() else {
        return;
    };
    let alive = enemies.iter().filter(|e| !e.dead).count();
    // COSMO_WATCH=<x> lists the y of every live actor in that column, for
    // watching a specific one move.
    if let Some(col) = hooks.watch_column {
        let mut ys: Vec<i32> = enemies
            .iter()
            .filter(|e| !e.dead && (e.x - col).abs() <= 2)
            .map(|e| e.y)
            .collect();
        ys.sort_unstable();
        println!("  watch x={col}: ys={ys:?}");
    }
    println!(
        "t={t} pos=({x},{y}) face={face} frame={frame} scroll=({sx},{sy}) \
         rel_row={rel} fall={fall}/{ft} jump={jt} cling={cling}{slip} \
         health={hp} dead={dead} enemies={alive} barrels={barrels} peds={peds} globe={globe} paused={paused} \
         stars={stars} score={score}",
        t = *tick,
        x = p.x,
        y = p.y,
        face = match p.face_dir {
            FaceDir::West => "W",
            FaceDir::East => "E",
        },
        frame = p.frame,
        sx = scroll.x,
        sy = scroll.y,
        rel = p.y - scroll.y,
        fall = p.is_falling as u8,
        ft = p.fall_time,
        jt = p.jump_time,
        cling = match p.cling_dir {
            Some(FaceDir::West) => "W",
            Some(FaceDir::East) => "E",
            None => "-",
        },
        slip = if p.cling_slip { "+slip" } else { "" },
        hp = p.health,
        dead = p.dead_timer,
        alive = alive,
        barrels = containers.iter().count(),
        peds = pedestals.iter().map(|p| p.height).sum::<i32>(),
        globe = near_globe
            .0
            .map(|h| h.to_string())
            .unwrap_or_else(|| "-".into()),
        paused = paused.0 as u8,
        stars = stars.0,
        score = score.0,
    );
}

/// `COSMO_SHOT=<path>` captures the window once, at tick
/// `COSMO_SHOT_AT` (default 30 - late enough for assets to have loaded).
/// Going through the engine rather than an external screen grab keeps the
/// capture deterministic and off whatever display the user is looking at.
///
/// `COSMO_SHOT_RAW=1` captures the 320x200 virtual screen instead of the
/// window. That is the one that can answer pixel-alignment questions: the
/// window shot has been through the present shader and the display's scale
/// factor, so "is this UI element on an exact pixel" is unanswerable from
/// it, while the raw buffer is the game's own framebuffer.
pub fn screenshot_at(
    mut commands: Commands,
    screen: Res<crate::presentation::VirtualScreen>,
    hooks: Res<Hooks>,
    mut ticks: Local<u32>,
    mut taken: Local<bool>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let Some(path) = hooks.shot_path.clone() else {
        return;
    };
    *ticks += 1;
    if *taken || *ticks < hooks.shot_at {
        return;
    }
    *taken = true;
    let shot = if hooks.shot_raw {
        Screenshot::image(screen.0.clone())
    } else {
        Screenshot::primary_window()
    };
    commands.spawn(shot).observe(save_to_disk(path));
}

/// `COSMO_FPS=1` reports frame pacing once a second: the average rate and
/// the worst frame in the window. The worst frame is the one that matters
/// for "feels laggy" - an average of 60 with occasional 100ms spikes feels
/// far worse than a steady 45.
pub fn report_frame_rate(
    time: Res<Time>,
    windows: Query<&Window>,
    hooks: Res<Hooks>,
    mut frames: Local<u32>,
    mut elapsed: Local<f32>,
    mut worst: Local<f32>,
) {
    if !hooks.fps {
        return;
    }
    let dt = time.delta_secs();
    *frames += 1;
    *elapsed += dt;
    *worst = worst.max(dt);
    if *elapsed >= 1.0 {
        // The window size has to be part of the report: a window the
        // compositor decided to maximise is several times the pixel count
        // of one it left alone, and comparing two runs without it is
        // comparing nothing.
        let size = windows
            .single()
            .map(|w| w.resolution.physical_size())
            .unwrap_or_default();
        println!(
            "fps={:.1} avg_frame={:.2}ms worst_frame={:.2}ms window={}x{}",
            *frames as f32 / *elapsed,
            *elapsed * 1000.0 / *frames as f32,
            *worst * 1000.0,
            size.x,
            size.y
        );
        *frames = 0;
        *elapsed = 0.0;
        *worst = 0.0;
    }
}

/// `COSMO_MOTION=1` logs the player's *drawn* position every frame, which
/// is the only way to see whether interpolation is doing anything - the
/// tick trace shows the simulation, which moves a whole tile at a time by
/// definition.
pub fn trace_drawn_position(
    query: Query<(&Transform, &crate::motion::PrevPos, &crate::player::Player)>,
    fixed: Res<Time<bevy::time::Fixed>>,
    hooks: Res<Hooks>,
    mut frames: Local<u32>,
) {
    if !hooks.motion_log {
        return;
    }
    *frames += 1;
    if let Ok((t, prev, p)) = query.single() {
        println!(
            "frame={} drawn_x={:.1} now={} prev={} alpha={:.3}",
            *frames,
            t.translation.x,
            p.x,
            prev.x,
            fixed.overstep_fraction()
        );
    }
}

/// `COSMO_QUIT_AFTER=<ticks>` ends the run on its own, so a verification
/// command terminates instead of needing to be killed.
pub fn quit_after(hooks: Res<Hooks>, mut ticks: Local<u32>, mut exit: EventWriter<AppExit>) {
    let Some(limit) = hooks.quit_after else {
        return;
    };
    *ticks += 1;
    if *ticks >= limit {
        exit.write(AppExit::Success);
    }
}
