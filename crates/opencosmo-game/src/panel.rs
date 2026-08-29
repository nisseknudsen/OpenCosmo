//! The game's text frames - the bordered gray boxes the original opens over
//! the paused game for hints, the help menu and so on.
//!
//! Geometry is expressed in the original's own terms so call sites can be
//! transcribed from the source unchanged: `UnfoldTextFrame(top, height,
//! width, title, bottom)` (game2.c:1017-1042) centres a `width`-tile frame
//! horizontally on the 40x25 tile screen at row `top`, and returns
//! `left + 1` as the x for the `DrawTextLine(x, row, text)` calls that fill
//! it. Rows passed to those calls are absolute screen rows, not offsets
//! into the frame.
//!
//! Laying out against the window rather than against a parent node is
//! deliberate: the aspect-ratio-sized node the title screen uses derives
//! its size from its parent's height, which collapses to nothing inside the
//! status bar's UI camera. Since the window shows exactly the original's
//! 320x200 screen (the game view plus the status bar, 25 tiles tall), tile
//! coordinates convert straight to window percentages.

use crate::hud::{HudAssets, HUD_RENDER_LAYER};
use crate::screen::{font_image, font_tile_for_char, FONT_BACKGROUND_GRAY};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

/// The original's virtual screen, in 8px tiles.
const SCREEN_W: f32 = 40.0;
const SCREEN_H: f32 = 25.0;

pub struct TextFrame {
    pub top: i32,
    pub height: i32,
    pub width: i32,
    /// Drawn centred on the screen at `top + 1`.
    pub title: String,
    /// Drawn centred at `top + height - 2`.
    pub bottom: String,
    /// `(x, absolute_row, text)`, as passed to `DrawTextLine`.
    lines: Vec<(i32, i32, String)>,
}

impl TextFrame {
    pub fn new(top: i32, height: i32, width: i32, title: &str, bottom: &str) -> Self {
        TextFrame {
            top,
            height,
            width,
            title: title.to_string(),
            bottom: bottom.to_string(),
            lines: Vec::new(),
        }
    }

    /// `left` (game2.c:1020). Note the original's own comment that the
    /// centred title ignores this entirely and centres on column 20.
    pub fn left(&self) -> i32 {
        20 - (self.width >> 1)
    }

    /// The x `UnfoldTextFrame` hands back for `DrawTextLine`.
    pub fn text_x(&self) -> i32 {
        self.left() + 1
    }

    #[cfg(test)]
    pub fn lines_for_test(&self) -> &[(i32, i32, String)] {
        &self.lines
    }

    pub fn line(mut self, x: i32, row: i32, text: &str) -> Self {
        self.lines.push((x, row, text.to_string()));
        self
    }

    /// Convenience for the common `DrawTextLine(x, row, ...)` at the
    /// frame's own text column.
    pub fn text(self, row: i32, text: &str) -> Self {
        let x = self.text_x();
        self.line(x, row, text)
    }

    pub fn spawn(&self, commands: &mut Commands, hud: &HudAssets, ui_camera: Entity, marker: impl Bundle) -> Entity {
        let mut all: Vec<(i32, i32, &str)> = Vec::new();
        if !self.title.is_empty() {
            all.push((
                20 - (self.title.chars().count() as i32 / 2),
                self.top + 1,
                &self.title,
            ));
        }
        if !self.bottom.is_empty() {
            all.push((
                20 - (self.bottom.chars().count() as i32 / 2),
                self.top + self.height - 2,
                &self.bottom,
            ));
        }
        for (x, row, text) in &self.lines {
            all.push((*x, *row, text));
        }

        commands
            .spawn((
                marker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(self.left() as f32 / SCREEN_W * 100.0),
                    top: Val::Percent(self.top as f32 / SCREEN_H * 100.0),
                    width: Val::Percent(self.width as f32 / SCREEN_W * 100.0),
                    height: Val::Percent(self.height as f32 / SCREEN_H * 100.0),
                    ..default()
                },
                UiTargetCamera(ui_camera),
                RenderLayers::layer(HUD_RENDER_LAYER),
            ))
            .with_children(|panel| {
                // One stretched copy of the font's gray tile rather than a
                // grid of them: it is a flat colour, so stretching is
                // identical to tiling and avoids seams from per-tile
                // percentage rounding.
                panel.spawn((
                    font_image(hud, FONT_BACKGROUND_GRAY),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
                for (x, row, text) in all {
                    for (i, c) in text.chars().enumerate() {
                        let Some(tile) = font_tile_for_char(c) else {
                            continue;
                        };
                        // Back into frame-local tiles, then into percentages
                        // of the frame's own box.
                        let col = (x - self.left()) as f32 + i as f32;
                        let line = (row - self.top) as f32;
                        panel.spawn((
                            font_image(hud, tile),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(col / self.width as f32 * 100.0),
                                top: Val::Percent(line / self.height as f32 * 100.0),
                                width: Val::Percent(100.0 / self.width as f32),
                                height: Val::Percent(100.0 / self.height as f32),
                                ..default()
                            },
                        ));
                    }
                }
            })
            .id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_centre_the_way_the_original_does() {
        // UnfoldTextFrame(2, 9, 28, ...) - episode 1's hint frame.
        let f = TextFrame::new(2, 9, 28, "COSMIC HINT!", "Press any key to exit.");
        assert_eq!(f.left(), 6);
        assert_eq!(f.text_x(), 7);
    }

    #[test]
    fn odd_widths_round_the_same_way_as_the_integer_shift() {
        let f = TextFrame::new(2, 9, 30, "", "");
        assert_eq!(f.left(), 5);
    }
}
