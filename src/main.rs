use bevy::prelude::*;

#[derive(Component)]
struct Player;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, move_player)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite::from_color(Color::srgb(0.3, 0.7, 1.0), Vec2::splat(50.0)),
        Transform::default(),
        Player,
    ));
}

fn move_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowUp)    { dir.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown)  { dir.y -= 1.0; }
    
    for mut transform in &mut query {
        transform.translation += (
            dir.normalize_or_zero() * 300.0 * time.delta_secs())
        .extend(0.0);
    }
}