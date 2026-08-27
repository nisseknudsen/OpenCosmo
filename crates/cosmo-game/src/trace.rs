//! Per-tick state dump for verifying mechanics without a human at the
//! keyboard. `COSMO_TRACE=<n>` prints every nth tick; combined with
//! `COSMO_INPUT` (a scripted key sequence) and `COSMO_QUIT_AFTER=<ticks>` a
//! behaviour can be exercised and checked from a single command.

use crate::camera::Scroll;
use crate::enemy_ai::Enemy;
use crate::player::{FaceDir, Player};
use bevy::prelude::*;

pub fn trace_enabled() -> bool {
    std::env::var("COSMO_TRACE").is_ok()
}

fn interval() -> u32 {
    std::env::var("COSMO_TRACE")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1)
}

pub fn trace_tick(
    player_q: Query<&Player>,
    enemies: Query<&Enemy>,
    containers: Query<&crate::actors::Container>,
    scroll: Res<Scroll>,
    near_globe: Res<crate::hints::NearHintGlobe>,
    paused: Res<crate::help::Paused>,
    stars: Res<crate::flow::Stars>,
    score: Res<crate::flow::Score>,
    mut tick: Local<u32>,
) {
    *tick += 1;
    if *tick % interval() != 0 {
        return;
    }
    let Ok(p) = player_q.single() else {
        return;
    };
    let alive = enemies.iter().filter(|e| !e.dead).count();
    // COSMO_WATCH=<x> lists the y of every live actor in that column, for
    // watching a specific one move.
    if let Ok(col) = std::env::var("COSMO_WATCH") {
        if let Ok(col) = col.trim().parse::<i32>() {
            let mut ys: Vec<i32> = enemies
                .iter()
                .filter(|e| !e.dead && (e.x - col).abs() <= 2)
                .map(|e| e.y)
                .collect();
            ys.sort_unstable();
            println!("  watch x={col}: ys={ys:?}");
        }
    }
    println!(
        "t={t} pos=({x},{y}) face={face} frame={frame} scroll=({sx},{sy}) \
         rel_row={rel} fall={fall}/{ft} jump={jt} cling={cling}{slip} \
         health={hp} dead={dead} enemies={alive} barrels={barrels} globe={globe} paused={paused} \
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
    mut ticks: Local<u32>,
    mut taken: Local<bool>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let Ok(path) = std::env::var("COSMO_SHOT") else {
        return;
    };
    let at: u32 = std::env::var("COSMO_SHOT_AT")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(30);
    *ticks += 1;
    if *taken || *ticks < at {
        return;
    }
    *taken = true;
    let shot = if std::env::var("COSMO_SHOT_RAW").is_ok() {
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
    mut frames: Local<u32>,
    mut elapsed: Local<f32>,
    mut worst: Local<f32>,
) {
    if std::env::var("COSMO_FPS").is_err() {
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
    mut frames: Local<u32>,
) {
    if std::env::var("COSMO_MOTION").is_err() {
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
pub fn quit_after(mut ticks: Local<u32>, mut exit: EventWriter<AppExit>) {
    let Ok(limit) = std::env::var("COSMO_QUIT_AFTER") else {
        return;
    };
    let Ok(limit) = limit.trim().parse::<u32>() else {
        return;
    };
    *ticks += 1;
    if *ticks >= limit {
        exit.write(AppExit::Success);
    }
}
