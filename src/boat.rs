//! The player's boat (banka / katig / balangay), wind sailing and the camera.

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use std::f32::consts::PI;

use crate::characters::{spawn_character, CharAssets, CharStyle, Headwear};
use crate::common::*;
use crate::water::{sea_height, sea_normal, Sea};
use crate::weather::Weather;
use crate::world::{depth_at, is_land, Props, PLAYER_START};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoatTier {
    Banka,
    Katig,
    Balangay,
}

pub struct TierStats {
    pub name: &'static str,
    pub capacity: u32,
    pub sail_speed: f32,
    pub paddle_speed: f32,
    pub turn_rate: f32,
    /// Multiplier on damage taken.
    pub frailty: f32,
    pub price: i32,
    pub length: f32,
    sail_w: f32,
    sail_h: f32,
    outriggers: bool,
}

impl BoatTier {
    pub fn stats(self) -> TierStats {
        match self {
            BoatTier::Banka => TierStats {
                name: "Banka",
                capacity: 20,
                sail_speed: 9.0,
                paddle_speed: 3.2,
                turn_rate: 1.15,
                frailty: 1.0,
                price: 0,
                length: 6.0,
                sail_w: 2.6,
                sail_h: 3.2,
                outriggers: false,
            },
            BoatTier::Katig => TierStats {
                name: "Katig outrigger",
                capacity: 40,
                sail_speed: 13.0,
                paddle_speed: 3.6,
                turn_rate: 1.0,
                frailty: 0.65,
                price: 300,
                length: 8.0,
                sail_w: 3.6,
                sail_h: 4.4,
                outriggers: true,
            },
            BoatTier::Balangay => TierStats {
                name: "Balangay",
                capacity: 80,
                sail_speed: 16.0,
                paddle_speed: 3.0,
                turn_rate: 0.75,
                frailty: 0.4,
                price: 800,
                length: 12.0,
                sail_w: 5.2,
                sail_h: 6.2,
                outriggers: true,
            },
        }
    }
    pub fn next(self) -> Option<BoatTier> {
        match self {
            BoatTier::Banka => Some(BoatTier::Katig),
            BoatTier::Katig => Some(BoatTier::Balangay),
            BoatTier::Balangay => None,
        }
    }
}

#[derive(Resource)]
pub struct BoatState {
    pub pos: Vec2,
    pub heading: f32,
    pub speed: f32,
    pub sail_up: bool,
    pub sail_amount: f32,
    pub hull: f32,
    pub tier: BoatTier,
    pub hidden_player: bool,
    /// Paddle input this frame: 1 forward, -1 backward, 0 none.
    pub paddle_input: f32,
    /// Steering input this frame: 1 left, -1 right.
    pub steer_input: f32,
    rebuild: bool,
}

impl BoatState {
    pub fn forward(&self) -> Vec2 {
        heading_vec(self.heading)
    }
    pub fn depth(&self) -> f32 {
        depth_at(self.pos)
    }
    pub fn request_rebuild(&mut self) {
        self.rebuild = true;
    }
}

#[derive(Resource)]
pub struct OrbitCam {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    focus: Vec3,
    idle: f32,
    /// Where the overseer camera looks.
    pub free: Vec2,
}

#[derive(Component)]
pub struct PlayerBoat;
#[derive(Component)]
struct BoatModel;
#[derive(Component)]
struct SailPivot;
#[derive(Component)]
struct SailCloth;
#[derive(Component)]
struct Pennant;
#[derive(Component)]
pub struct PlayerToon;
#[derive(Component)]
struct PaddlePivot;
#[derive(Component)]
struct PaddleBlade;

/// Stroke cycle of the player's paddle, plus the ripples it leaves.
#[derive(Resource)]
struct PaddleAnim {
    phase: f32,
    side: f32,
    active: f32,
    ripples: Vec<(Vec3, f32, f32)>,
    wake_timer: f32,
}

impl Default for PaddleAnim {
    fn default() -> Self {
        Self {
            phase: 0.0,
            side: 1.0,
            active: 0.0,
            ripples: Vec::new(),
            wake_timer: 0.0,
        }
    }
}

pub struct BoatPlugin;

impl Plugin for BoatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BoatState {
            pos: PLAYER_START,
            heading: 2.2,
            speed: 0.0,
            sail_up: false,
            sail_amount: 0.0,
            hull: 100.0,
            tier: BoatTier::Banka,
            hidden_player: false,
            paddle_input: 0.0,
            steer_input: 0.0,
            rebuild: true,
        })
        .insert_resource(OrbitCam {
            yaw: 2.2 + PI,
            pitch: 1.0,
            dist: 150.0,
            focus: Vec3::new(PLAYER_START.x, 1.0, PLAYER_START.y),
            idle: 10.0,
            free: crate::world::SHOALS[0].center,
        })
        .init_resource::<PaddleAnim>()
        .add_systems(Startup, spawn_boat_and_camera)
        .add_systems(
            Update,
            (
                sail_boat.run_if(in_state(GameState::Playing).and_then(resource_equals(ViewMode::Captain))),
                toggle_view.run_if(in_state(GameState::Playing)),
                rebuild_model,
            ),
        )
        .add_systems(
            PostUpdate,
            (place_boat, animate_paddle, orbit_camera).chain().before(TransformSystems::Propagate),
        );
    }
}

fn spawn_boat_and_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Tonemapping::AcesFitted,
        Projection::Perspective(PerspectiveProjection {
            fov: 50f32.to_radians(),
            far: 3000.0,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.8, 0.88, 0.95),
            falloff: FogFalloff::Linear { start: 60.0, end: 520.0 },
            ..default()
        },
        Transform::from_xyz(PLAYER_START.x, 12.0, PLAYER_START.y + 25.0)
            .looking_at(Vec3::new(PLAYER_START.x, 0.0, PLAYER_START.y), Vec3::Y),
        MainCamera,
    ));
    commands.spawn((
        Transform::from_xyz(PLAYER_START.x, 0.0, PLAYER_START.y),
        Visibility::default(),
        PlayerBoat,
    ));
}

fn part(
    commands: &mut Commands,
    parent: Entity,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    t: Transform,
) -> Entity {
    commands
        .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), t, ChildOf(parent)))
        .id()
}

fn rebuild_model(
    mut commands: Commands,
    mut boat: ResMut<BoatState>,
    props: Res<Props>,
    chars: Res<CharAssets>,
    boat_root: Query<Entity, With<PlayerBoat>>,
    old: Query<Entity, With<BoatModel>>,
    mut rng: ResMut<Rng>,
) {
    if !boat.rebuild {
        return;
    }
    let Ok(root) = boat_root.single() else { return };
    boat.rebuild = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let s = boat.tier.stats();
    let l = s.length;
    let w = 0.9 + l * 0.06;
    let model = commands
        .spawn((Transform::default(), Visibility::default(), BoatModel, ChildOf(root)))
        .id();
    let p = &*props;
    let hull_paint = &p.paints[match boat.tier {
        BoatTier::Banka => 1,
        BoatTier::Katig => 0,
        BoatTier::Balangay => 3,
    }];

    // Hull: a stretched half-sunk ellipsoid with a painted gunwale and deck.
    part(&mut commands, model, &p.sphere, &p.wood, Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(w, 1.0, l)));
    part(&mut commands, model, &p.sphere, hull_paint, Transform::from_xyz(0.0, 0.28, 0.0).with_scale(Vec3::new(w * 1.04, 0.16, l * 1.01)));
    part(&mut commands, model, &p.cube, &p.wood_dark, Transform::from_xyz(0.0, 0.42, 0.0).with_scale(Vec3::new(w * 0.72, 0.08, l * 0.78)));
    // Carved prow and stern (like the jungal of a lepa).
    for (z, tilt) in [(l * 0.47, -0.7), (-l * 0.47, 0.7)] {
        part(
            &mut commands,
            model,
            &p.cube,
            hull_paint,
            Transform::from_xyz(0.0, 0.62, z)
                .with_rotation(Quat::from_rotation_x(tilt))
                .with_scale(Vec3::new(0.18, 0.9, 0.3)),
        );
    }
    // Katig: bamboo outrigger floats and booms.
    if s.outriggers {
        let span = w * 0.5 + 1.8 + l * 0.08;
        for side in [-1.0, 1.0] {
            part(&mut commands, model, &p.sphere, &p.bamboo, Transform::from_xyz(span * side, 0.1, 0.0).with_scale(Vec3::new(0.35, 0.35, l * 0.75)));
        }
        for z in [l * 0.22, -l * 0.22] {
            part(
                &mut commands,
                model,
                &p.cyl,
                &p.bamboo,
                Transform::from_xyz(0.0, 0.62, z)
                    .with_rotation(Quat::from_rotation_z(PI / 2.0))
                    .with_scale(Vec3::new(0.12, span * 2.1, 0.12)),
            );
        }
    }
    if boat.tier == BoatTier::Balangay {
        // A little thatched shelter amidships.
        part(&mut commands, model, &p.cube, &p.wood, Transform::from_xyz(0.0, 1.0, -l * 0.18).with_scale(Vec3::new(w * 0.7, 1.1, 2.6)));
        part(
            &mut commands,
            model,
            &p.cone4,
            &p.thatch,
            Transform::from_xyz(0.0, 1.9, -l * 0.18)
                .with_rotation(Quat::from_rotation_y(PI / 4.0))
                .with_scale(Vec3::new(w * 1.3, 0.9, 4.0)),
        );
    }

    // Mast, sail and pennant.
    let mast_z = l * 0.12;
    let mast_h = s.sail_h + 1.6;
    part(&mut commands, model, &p.cyl, &p.wood_dark, Transform::from_xyz(0.0, mast_h / 2.0 + 0.3, mast_z).with_scale(Vec3::new(0.12, mast_h, 0.12)));
    let pivot = commands
        .spawn((Transform::from_xyz(0.0, 1.1, mast_z), Visibility::default(), SailPivot, ChildOf(model)))
        .id();
    let cloth = commands
        .spawn((Transform::default(), Visibility::default(), SailCloth, ChildOf(pivot)))
        .id();
    let stripes = 5;
    for i in 0..stripes {
        let m = &p.paints[(i + boat.tier as usize * 2) % p.paints.len()];
        let band = s.sail_h / stripes as f32;
        // Slight taper to the top like a tanja sail.
        let width = s.sail_w * (1.0 - i as f32 * 0.08);
        part(
            &mut commands,
            cloth,
            &p.cube,
            m,
            Transform::from_xyz(0.0, band * (i as f32 + 0.5), -width / 2.0 - 0.1).with_scale(Vec3::new(0.05, band * 1.02, width)),
        );
    }
    part(&mut commands, pivot, &p.cyl, &p.bamboo, Transform::from_xyz(0.0, 0.0, -s.sail_w / 2.0).with_rotation(Quat::from_rotation_x(PI / 2.0)).with_scale(Vec3::new(0.08, s.sail_w + 0.4, 0.08)));
    let pennant = commands
        .spawn((Transform::from_xyz(0.0, mast_h + 0.3, mast_z), Visibility::default(), Pennant, ChildOf(model)))
        .id();
    part(&mut commands, pennant, &p.cube, &p.paints[2], Transform::from_xyz(0.0, 0.0, 0.7).with_scale(Vec3::new(0.03, 0.35, 1.4)));

    // The player: a Sama fisher perched at the stern with a paddle.
    let style = CharStyle {
        skin: 0,
        shirt: 1,
        sarong: 2,
        headwear: Headwear::Scarf,
        scale: 1.05,
    };
    let toon = spawn_character(&mut commands, &chars, style);
    commands.entity(toon).insert((
        Transform::from_xyz(0.0, 0.42, -l * 0.3).with_scale(Vec3::splat(1.05)),
        PlayerToon,
        ChildOf(model),
    ));
    // A bugsay paddle with a leaf-shaped blade, hanging from the grip hand.
    let pivot = commands
        .spawn((Transform::from_xyz(-0.9, 0.62, 0.35), Visibility::default(), PaddlePivot, ChildOf(toon)))
        .id();
    part(&mut commands, pivot, &p.cyl, &p.wood, Transform::from_xyz(0.0, -1.0, 0.0).with_scale(Vec3::new(0.07, 2.0, 0.07)));
    let blade = part(
        &mut commands,
        pivot,
        &p.sphere,
        &p.wood,
        Transform::from_xyz(0.0, -2.1, 0.0).with_scale(Vec3::new(0.34, 0.8, 0.07)),
    );
    commands.entity(blade).insert(PaddleBlade);
    // Extra crew on bigger boats.
    let crew = match boat.tier {
        BoatTier::Banka => 0,
        BoatTier::Katig => 1,
        BoatTier::Balangay => 3,
    };
    for k in 0..crew {
        let c = spawn_character(&mut commands, &chars, CharStyle::random(&mut rng, k == 2));
        commands.entity(c).insert((
            Transform::from_xyz(if k % 2 == 0 { 0.2 } else { -0.2 }, 0.42, l * (0.3 - 0.18 * k as f32)),
            ChildOf(model),
        ));
    }
}

/// Fraction of the wind's push a sail gets at a relative angle
/// (0 = running downwind, PI = straight into the wind).
pub fn sail_efficiency(rel: f32) -> f32 {
    let deg = rel.abs().to_degrees();
    if deg < 100.0 {
        0.65 + 0.35 * (deg / 100.0)
    } else if deg < 145.0 {
        (145.0 - deg) / 45.0
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments)]
fn sail_boat(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    weather: Res<Weather>,
    mut boat: ResMut<BoatState>,
    mut inv: ResMut<Inventory>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    harbors: Res<crate::village::Communities>,
) {
    let dt = time.delta_secs();
    let s = boat.tier.stats();
    if keys.just_pressed(KeyCode::Space) {
        boat.sail_up = !boat.sail_up;
    }
    let sail_goal = if boat.sail_up { 1.0 } else { 0.0 };
    boat.sail_amount += (sail_goal - boat.sail_amount) * (1.0 - (-dt * 2.5).exp());

    let mut steer: f32 = 0.0;
    if keys.pressed(KeyCode::KeyA) {
        steer += 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        steer -= 1.0;
    }
    let mut paddle: f32 = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        paddle += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        paddle -= 0.6;
    }

    let turn = s.turn_rate * (0.55 + 0.45 * (boat.speed.abs() / 5.0).min(1.0));
    boat.heading = wrap_angle(boat.heading + steer * turn * dt);
    boat.paddle_input = paddle.clamp(-1.0, 1.0);
    boat.steer_input = steer;

    let rel = wrap_angle(boat.heading - weather.wind_dir);
    let wind = weather.wind_strength.min(1.4);
    let target = boat.sail_amount * sail_efficiency(rel) * wind * s.sail_speed + paddle * s.paddle_speed;
    let rate = if target.abs() < boat.speed.abs() { 0.9 } else { 0.45 };
    boat.speed += (target - boat.speed) * (1.0 - (-dt * rate).exp());

    // Storm winds shove the boat around.
    let drift = weather.wind_vec() * weather.storm * 1.6;
    let step = boat.forward() * boat.speed * dt + drift * dt;
    let next = boat.pos + step;
    if is_land(next) || next.x.abs() > WORLD_HALF || next.y.abs() > WORLD_HALF {
        if boat.speed.abs() > 3.0 {
            sfx.play(Sfx::Thud, 0.6);
        }
        boat.speed *= -0.25;
    } else {
        boat.pos = next;
    }

    // Storm damage: deep water is where the big waves are.
    if weather.raging() {
        let depth = boat.depth();
        let rate = if depth > 8.0 { 2.4 } else if depth > 4.0 { 0.8 } else { 0.15 };
        boat.hull -= rate * weather.storm * s.frailty * dt;
    }
    if boat.hull <= 0.0 {
        inv.fish = 0;
        inv.urchin = 0;
        inv.seaweed = 0;
        inv.dried_fish = 0;
        inv.water_jars = 0;
        boat.hull = 35.0;
        boat.speed = 0.0;
        boat.sail_up = false;
        let harbor = harbors.nearest_center(boat.pos).unwrap_or(PLAYER_START);
        boat.pos = harbor + Vec2::new(8.0, 8.0);
        sfx.play(Sfx::Splash, 1.0);
        log.push("Your boat capsized! Kin fished you out of the sea, but the cargo is lost.");
    }
}

fn place_boat(
    time: Res<Time>,
    sea: Res<Sea>,
    weather: Res<Weather>,
    boat: Res<BoatState>,
    mut root: Query<&mut Transform, With<PlayerBoat>>,
    mut pivots: Query<&mut Transform, (With<SailPivot>, Without<PlayerBoat>)>,
    mut cloths: Query<&mut Transform, (With<SailCloth>, Without<PlayerBoat>, Without<SailPivot>)>,
    mut pennants: Query<&mut Transform, (With<Pennant>, Without<PlayerBoat>, Without<SailPivot>, Without<SailCloth>)>,
    mut toons: Query<&mut Visibility, With<PlayerToon>>,
) {
    let Ok(mut t) = root.single_mut() else { return };
    let h = sea_height(&sea, boat.pos);
    let n = sea_normal(&sea, boat.pos).lerp(Vec3::Y, 0.35).normalize();
    let yaw = Quat::from_rotation_y(boat.heading);
    let rel = wrap_angle(weather.wind_dir - boat.heading);
    let heel = -rel.sin() * boat.sail_amount * weather.wind_strength.min(1.5) * 0.1;
    t.translation = Vec3::new(boat.pos.x, h - 0.05, boat.pos.y);
    t.rotation = Quat::from_rotation_arc(Vec3::Y, n) * yaw * Quat::from_rotation_z(heel);

    // The boom swings out to leeward, roughly half the relative wind angle.
    let boom = (-rel * 0.5).clamp(-1.35, 1.35);
    let flutter = (time.elapsed_secs() * 9.0).sin() * 0.03 * (1.0 - boat.sail_amount * 0.7);
    for mut p in &mut pivots {
        p.rotation = Quat::from_rotation_y(boom + flutter);
    }
    for mut c in &mut cloths {
        c.scale = Vec3::new(1.0, 0.06 + 0.94 * boat.sail_amount, 1.0);
    }
    for mut p in &mut pennants {
        let wiggle = (time.elapsed_secs() * 14.0).sin() * 0.15;
        p.rotation = Quat::from_rotation_y(rel + wiggle);
    }
    for mut v in &mut toons {
        *v = if boat.hidden_player { Visibility::Hidden } else { Visibility::Inherited };
    }
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Local transform of a paddle's grip pivot (paddle hanging along -Y) for a
/// stroke `phase` in 0..1 on `side` (+1 left, -1 right), blended with the
/// resting pose across the lap by `active`.
pub fn paddle_pose(phase: f32, side: f32, backwards: bool, active: f32) -> Transform {
    // The blade sweeps from ahead to behind in the water, then lifts out and
    // swings forward again.
    let u = phase;
    let (mut z, y) = if u < 0.55 {
        let t = ease(u / 0.55);
        (0.95 + (-0.75 - 0.95) * t, -1.0)
    } else {
        let t = (u - 0.55) / 0.45;
        (-0.75 + (0.95 + 0.75) * ease(t), -1.0 + 0.7 * (t * PI).sin())
    };
    if backwards {
        z = -z;
    }
    let stroke_dir = Vec3::new(side * 0.5, y, z).normalize();
    let stroke_pos = Vec3::new(side * 0.4, 0.95, 0.15 + z * 0.15);
    let rest_dir = Vec3::new(1.0, -0.12, 0.25).normalize();
    let rest_pos = Vec3::new(-0.9, 0.62, 0.35);
    let d = rest_dir.lerp(stroke_dir, active).normalize();
    Transform::from_translation(rest_pos.lerp(stroke_pos, active)).with_rotation(Quat::from_rotation_arc(Vec3::NEG_Y, d))
}

/// Alternating-side paddle strokes while the player paddles (or steers with
/// the sail down); otherwise the paddle rests across the lap.
#[allow(clippy::too_many_arguments)]
fn animate_paddle(
    time: Res<Time>,
    sea: Res<Sea>,
    boat: Res<BoatState>,
    state: Res<State<GameState>>,
    mut anim: ResMut<PaddleAnim>,
    mut sfx: ResMut<SfxQueue>,
    mut pivots: Query<&mut Transform, With<PaddlePivot>>,
    mut toons: Query<&mut Transform, (With<PlayerToon>, Without<PaddlePivot>)>,
    blades: Query<&GlobalTransform, With<PaddleBlade>>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    let playing = *state.get() == GameState::Playing;
    let dir = boat.paddle_input;
    let steer = boat.steer_input;
    let wants = playing && !boat.hidden_player && (dir != 0.0 || (steer != 0.0 && boat.sail_amount < 0.5));
    let goal = if wants { 1.0 } else { 0.0 };
    anim.active += (goal - anim.active) * (1.0 - (-dt * 6.0).exp());

    let blade_pos = blades.iter().next().map(|g| g.translation());
    if wants {
        let prev = anim.phase;
        anim.phase += dt * 1.4;
        if anim.phase >= 1.0 {
            anim.phase -= 1.0;
            // Steer by paddling on the outside of the turn; otherwise alternate sides.
            anim.side = if steer != 0.0 { -steer.signum() } else { -anim.side };
        }
        let catch = prev < 0.08 && anim.phase >= 0.08;
        let release = prev < 0.55 && anim.phase >= 0.55;
        if let Some(p) = blade_pos {
            if catch {
                sfx.play(Sfx::Paddle, 0.35);
                anim.ripples.push((p, 0.0, 1.0));
            } else if release {
                anim.ripples.push((p, 0.0, 0.7));
            }
        }
    }

    let a = anim.active;
    for mut t in &mut pivots {
        *t = paddle_pose(anim.phase, anim.side, dir < 0.0, a);
    }
    let (s, u) = (anim.side, anim.phase);
    // Lean into the catch and twist toward the paddling side.
    for mut t in &mut toons {
        let lean = a * 0.2 * (u * std::f32::consts::TAU).cos();
        t.rotation = Quat::from_rotation_y(a * s * 0.3) * Quat::from_rotation_x(lean);
    }

    // A small wake behind the stern when moving.
    anim.wake_timer -= dt;
    if boat.speed.abs() > 0.8 && anim.wake_timer <= 0.0 {
        anim.wake_timer = 0.3;
        let stern = boat.pos - boat.forward() * boat.tier.stats().length * 0.5 * boat.speed.signum();
        anim.ripples.push((Vec3::new(stern.x, 0.0, stern.y), 0.0, 0.6 + (boat.speed.abs() / 10.0).min(0.6)));
    }

    for r in anim.ripples.iter_mut() {
        r.1 += dt;
    }
    anim.ripples.retain(|r| r.1 < 1.4);
    let flat = Quat::from_rotation_x(PI / 2.0);
    for (p, age, size) in &anim.ripples {
        let h = sea_height(&sea, Vec2::new(p.x, p.z)) + 0.04;
        let k = age / 1.4;
        let col = Color::srgba(1.0, 1.0, 1.0, 0.55 * (1.0 - k));
        let iso = Isometry3d::new(Vec3::new(p.x, h, p.z), flat);
        gizmos.circle(iso, 0.2 + k * 1.6 * size, col);
        gizmos.circle(iso, 0.1 + k * 0.9 * size, col.with_alpha(0.35 * (1.0 - k)));
    }
}

/// Tab switches between overseeing families and steering your own boat.
fn toggle_view(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ViewMode>,
    mut orbit: ResMut<OrbitCam>,
    boat: Res<BoatState>,
    mut log: ResMut<GameLog>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    *mode = match *mode {
        ViewMode::Overseer => {
            orbit.pitch = 0.38;
            orbit.dist = 26.0;
            orbit.yaw = boat.heading + PI;
            log.push("Captain view: you steer your own boat (W/S paddle, A/D steer, Space sail). Tab to go back.");
            ViewMode::Captain
        }
        ViewMode::Captain => {
            orbit.pitch = 1.0;
            orbit.dist = 150.0;
            orbit.free = boat.pos;
            log.push("Overseer view: click families and give them work. Tab to steer your own boat.");
            ViewMode::Overseer
        }
    };
}

#[allow(clippy::too_many_arguments)]
fn orbit_camera(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    sea: Res<Sea>,
    boat: Res<BoatState>,
    state: Res<State<GameState>>,
    mode: Res<ViewMode>,
    mut goto: ResMut<CameraGoto>,
    mut orbit: ResMut<OrbitCam>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs();
    let Ok(mut cam) = camera.single_mut() else { return };
    orbit.idle += dt;
    let title = *state.get() == GameState::Title;
    let overseer = *mode == ViewMode::Overseer && !title;
    if title {
        // Slow establishing orbit on the title screen.
        orbit.yaw += dt * 0.05;
    } else {
        if mouse.pressed(MouseButton::Right) {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.004).clamp(0.05, 1.45);
            orbit.idle = 0.0;
        }
        if scroll.delta.y != 0.0 {
            let amount = match scroll.unit {
                bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y * 0.004,
            };
            let (lo, hi) = if overseer { (25.0, 450.0) } else { (8.0, 180.0) };
            orbit.dist = (orbit.dist * (1.0 - amount)).clamp(lo, hi);
        }
        if overseer {
            // Pan across the sea with WASD / arrows; Q/E rotate.
            let fwd = -Vec2::new(orbit.yaw.sin(), orbit.yaw.cos());
            let right = Vec2::new(-fwd.y, fwd.x);
            let mut pan = Vec2::ZERO;
            for (k, v) in [
                (KeyCode::KeyW, fwd),
                (KeyCode::ArrowUp, fwd),
                (KeyCode::KeyS, -fwd),
                (KeyCode::ArrowDown, -fwd),
                (KeyCode::KeyD, right),
                (KeyCode::ArrowRight, right),
                (KeyCode::KeyA, -right),
                (KeyCode::ArrowLeft, -right),
            ] {
                if keys.pressed(k) {
                    pan += v;
                }
            }
            if pan != Vec2::ZERO {
                goto.0 = None;
                let speed = 40.0 + orbit.dist * 0.9;
                orbit.free += pan.normalize() * speed * dt;
            }
            if keys.pressed(KeyCode::KeyQ) {
                orbit.yaw += dt * 1.4;
            }
            if keys.pressed(KeyCode::KeyE) {
                orbit.yaw -= dt * 1.4;
            }
            if let Some(p) = goto.0 {
                let f = orbit.free;
                orbit.free = f + (p - f) * (1.0 - (-dt * 4.0).exp());
                if orbit.free.distance(p) < 1.0 {
                    goto.0 = None;
                }
            }
            orbit.free = orbit.free.clamp(Vec2::splat(-650.0), Vec2::splat(650.0));
        } else {
            let mut k = 0.0;
            if keys.pressed(KeyCode::ArrowLeft) {
                k -= 1.0;
            }
            if keys.pressed(KeyCode::ArrowRight) {
                k += 1.0;
            }
            if k != 0.0 {
                orbit.yaw += k * dt * 1.6;
                orbit.idle = 0.0;
            }
            if keys.pressed(KeyCode::ArrowUp) {
                orbit.pitch = (orbit.pitch + dt).min(1.4);
            }
            if keys.pressed(KeyCode::ArrowDown) {
                orbit.pitch = (orbit.pitch - dt).max(0.05);
            }
            // Drift back behind the boat when sailing and the player isn't looking around.
            if orbit.idle > 2.5 && boat.speed > 1.5 {
                let behind = boat.heading + PI;
                let diff = wrap_angle(behind - orbit.yaw);
                orbit.yaw += diff * (1.0 - (-dt * 0.8).exp());
            }
        }
    }
    let target = if overseer {
        Vec3::new(orbit.free.x, 0.0, orbit.free.y)
    } else {
        Vec3::new(boat.pos.x, 1.6, boat.pos.y)
    };
    let f = orbit.focus;
    orbit.focus = f + (target - f) * (1.0 - (-dt * 6.0).exp());
    let (pitch, dist) = if title { (0.3, 60.0) } else { (orbit.pitch, orbit.dist) };
    let dir = Vec3::new(orbit.yaw.sin() * pitch.cos(), pitch.sin(), orbit.yaw.cos() * pitch.cos());
    let mut pos = orbit.focus + dir * dist;
    let floor = sea_height(&sea, Vec2::new(pos.x, pos.z)) + 1.2;
    pos.y = pos.y.max(floor);
    *cam = Transform::from_translation(pos).looking_at(orbit.focus, Vec3::Y);
}
