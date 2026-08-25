use crate::flow::Score;
use crate::level::CurrentLevel;
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
                Text::new("Level: -  Score: 0"),
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
    mut query: Query<&mut Text, With<LevelLabel>>,
) {
    if !level.is_changed() && !score.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!("Level: {}  Score: {}", level.name, score.0);
    }
}
