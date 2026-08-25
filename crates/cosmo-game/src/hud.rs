use crate::flow::Score;
use crate::level::CurrentLevel;
use crate::player::Player;
use bevy::prelude::*;

#[derive(Component)]
pub struct LevelLabel;

pub fn spawn_hud(commands: &mut Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(6.0),
            left: Val::Px(8.0),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Text::new("Level: -  Score: 0  Health: 4"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                LevelLabel,
            ));
        });
}

pub fn update_hud(
    level: Res<CurrentLevel>,
    score: Res<Score>,
    player_q: Query<&Player>,
    mut query: Query<&mut Text, With<LevelLabel>>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    for mut text in &mut query {
        text.0 = format!(
            "Level: {}  Score: {}  Health: {}",
            level.name, score.0, player.health
        );
    }
}
