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
    scroll: Res<Scroll>,
    near_globe: Res<crate::hints::NearHintGlobe>,
    paused: Res<crate::help::Paused>,
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
    println!(
        "t={t} pos=({x},{y}) face={face} frame={frame} scroll=({sx},{sy}) \
         rel_row={rel} fall={fall}/{ft} jump={jt} cling={cling}{slip} \
         health={hp} dead={dead} enemies={alive} globe={globe} paused={paused}",
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
        globe = near_globe
            .0
            .map(|h| h.to_string())
            .unwrap_or_else(|| "-".into()),
        paused = paused.0 as u8,
    );
}

/// `COSMO_SHOT=<path>` captures the window once, at tick
/// `COSMO_SHOT_AT` (default 30 - late enough for assets to have loaded).
/// Going through the engine rather than an external screen grab keeps the
/// capture deterministic and off whatever display the user is looking at.
pub fn screenshot_at(mut commands: Commands, mut ticks: Local<u32>, mut taken: Local<bool>) {
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
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
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
