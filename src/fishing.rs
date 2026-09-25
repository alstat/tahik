//! Sea life and livelihoods: fish schools, sea urchin beds, net fishing,
//! diving, and the sharks that prowl the deep.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::boat::BoatState;
use crate::common::*;
use crate::water::{sea_height, Sea};
use crate::world::{depth_at, Props, SHOALS};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ActivityKind {
    #[default]
    None,
    Fishing,
    Diving,
}

#[derive(Resource, Default)]
pub struct Activity {
    pub kind: ActivityKind,
    pub t: f32,
    pub dur: f32,
    target: Option<Entity>,
}

impl Activity {
    pub fn busy(&self) -> bool {
        self.kind != ActivityKind::None
    }
    pub fn progress(&self) -> f32 {
        if self.dur > 0.0 { (self.t / self.dur).min(1.0) } else { 0.0 }
    }
    pub fn cancel(&mut self) {
        *self = Activity::default();
    }
}

/// Context-sensitive key hints collected every frame for the HUD.
#[derive(Resource, Default)]
pub struct Hints(pub Vec<String>);

#[derive(Component)]
pub struct FishSchool {
    pub pos: Vec2,
    heading: f32,
    pub stock: f32,
}

#[derive(Component)]
struct Jumper {
    phase: f32,
    offset: Vec2,
    dir: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SharkState {
    Wander,
    Stalk(f32),
    Charge,
    Retreat(f32),
}

#[derive(Component)]
pub struct Shark {
    pub pos: Vec2,
    heading: f32,
    home: Vec2,
    target: Vec2,
    state: SharkState,
    cooldown: f32,
}

impl Shark {
    pub fn hunting(&self) -> bool {
        matches!(self.state, SharkState::Stalk(_) | SharkState::Charge)
    }
}

#[derive(Component)]
pub struct UrchinBed {
    pub pos: Vec2,
    pub stock: f32,
    urchins: Vec<Entity>,
}

const URCHIN_MAX: f32 = 8.0;
const SCHOOL_MAX: f32 = 24.0;

pub struct FishingPlugin;

impl Plugin for FishingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Activity>()
            .init_resource::<Hints>()
            .add_systems(First, |mut h: ResMut<Hints>| h.0.clear())
            .add_systems(Startup, spawn_sea_life)
            .add_systems(Update, (move_schools, animate_jumpers, place_sharks, show_urchins))
            .add_systems(
                Update,
                (
                    regrow,
                    shark_ai.run_if(resource_equals(ViewMode::Captain)),
                    start_activity.run_if(resource_equals(ViewMode::Captain)),
                    progress_activity,
                    draw_activity,
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn urchin_mesh() -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut tri = |a: Vec3, b: Vec3, c: Vec3| {
        let n = (b - a).cross(c - a);
        let (b, c) = if n.dot(a + b + c) < 0.0 { (c, b) } else { (b, c) };
        pos.extend_from_slice(&[a.to_array(), b.to_array(), c.to_array()]);
    };
    // Octahedron core.
    let r = 0.17;
    let axes = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
    for &x in &axes[0..2] {
        for &y in &axes[2..4] {
            for &z in &axes[4..6] {
                tri(x * r, y * r, z * r);
            }
        }
    }
    // Spines on a Fibonacci sphere.
    let n = 34;
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let rr = (1.0 - y * y).sqrt();
        let phi = i as f32 * 2.399_963;
        let d = Vec3::new(phi.cos() * rr, y, phi.sin() * rr);
        let t1 = d.any_orthonormal_vector();
        let t2 = d.cross(t1);
        let base: Vec<Vec3> = (0..3)
            .map(|k| {
                let a = k as f32 * TAU / 3.0;
                d * 0.13 + (t1 * a.cos() + t2 * a.sin()) * 0.035
            })
            .collect();
        let tip = d * 0.5;
        for k in 0..3 {
            tri(base[k], base[(k + 1) % 3], tip);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.compute_flat_normals();
    mesh
}

fn random_deep_point(rng: &mut Rng, min_depth: f32) -> Vec2 {
    for _ in 0..200 {
        let p = Vec2::new(rng.range(-520.0, 520.0), rng.range(-520.0, 520.0));
        if depth_at(p) > min_depth {
            return p;
        }
    }
    Vec2::new(0.0, 0.0)
}

fn spawn_sea_life(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    props: Res<Props>,
    mut rng: ResMut<Rng>,
) {
    let shadow = materials.add(StandardMaterial {
        base_color: Color::srgba(0.02, 0.08, 0.14, 0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let fish_mesh = meshes.add(Sphere::new(0.5).mesh().uv(10, 6));
    for _ in 0..16 {
        let p = random_deep_point(&mut rng, 12.0);
        let root = commands
            .spawn((
                Transform::from_xyz(p.x, 0.0, p.y),
                Visibility::default(),
                FishSchool {
                    pos: p,
                    heading: rng.range(0.0, TAU),
                    stock: rng.range(12.0, SCHOOL_MAX),
                },
            ))
            .id();
        commands.spawn((
            Mesh3d(props.sphere.clone()),
            MeshMaterial3d(shadow.clone()),
            Transform::from_xyz(0.0, -1.2, 0.0).with_scale(Vec3::new(16.0, 0.4, 11.0)),
            ChildOf(root),
        ));
        for _ in 0..10 {
            commands.spawn((
                Mesh3d(fish_mesh.clone()),
                MeshMaterial3d(props.fish.clone()),
                Transform::from_xyz(0.0, -5.0, 0.0).with_scale(Vec3::new(0.25, 0.32, 0.8)),
                Jumper {
                    phase: rng.range(0.0, 4.0),
                    offset: Vec2::new(rng.range(-7.0, 7.0), rng.range(-5.0, 5.0)),
                    dir: rng.range(0.0, TAU),
                },
                ChildOf(root),
            ));
        }
    }

    // Sharks: a dorsal fin and tail cutting the surface, a dark body below.
    let grey = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.40, 0.46),
        perceptual_roughness: 0.6,
        ..default()
    });
    for _ in 0..10 {
        let p = random_deep_point(&mut rng, 14.0);
        let root = commands
            .spawn((
                Transform::from_xyz(p.x, 0.0, p.y),
                Visibility::default(),
                Shark {
                    pos: p,
                    heading: rng.range(0.0, TAU),
                    home: p,
                    target: p,
                    state: SharkState::Wander,
                    cooldown: 0.0,
                },
            ))
            .id();
        commands.spawn((
            Mesh3d(props.cone4.clone()),
            MeshMaterial3d(grey.clone()),
            Transform::from_xyz(0.0, 0.45, 0.0)
                .with_rotation(Quat::from_rotation_x(-0.25))
                .with_scale(Vec3::new(0.12, 1.3, 1.0)),
            ChildOf(root),
        ));
        commands.spawn((
            Mesh3d(props.cone4.clone()),
            MeshMaterial3d(grey.clone()),
            Transform::from_xyz(0.0, 0.2, -2.6)
                .with_rotation(Quat::from_rotation_x(-0.5))
                .with_scale(Vec3::new(0.08, 0.8, 0.5)),
            ChildOf(root),
        ));
        commands.spawn((
            Mesh3d(props.sphere.clone()),
            MeshMaterial3d(grey.clone()),
            Transform::from_xyz(0.0, -0.75, -0.4).with_scale(Vec3::new(1.0, 0.8, 4.6)),
            ChildOf(root),
        ));
    }

    // Sea urchin (tayum) beds on the reef flats.
    let urchin = meshes.add(urchin_mesh());
    let purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.07, 0.26),
        perceptual_roughness: 0.5,
        ..default()
    });
    for s in SHOALS.iter() {
        let mut placed = 0;
        let mut tries = 0;
        while placed < 5 && tries < 300 {
            tries += 1;
            let a = rng.range(0.0, TAU);
            let r = rng.range(s.radius * 0.35, s.radius * 1.0);
            let p = s.center + Vec2::new(a.cos(), a.sin()) * r;
            let d = depth_at(p);
            if !(1.4..6.0).contains(&d) {
                continue;
            }
            placed += 1;
            let mut urchins = Vec::new();
            for _ in 0..URCHIN_MAX as usize {
                let q = p + Vec2::new(rng.range(-2.5, 2.5), rng.range(-2.5, 2.5));
                let g = -depth_at(q);
                let e = commands
                    .spawn((
                        Mesh3d(urchin.clone()),
                        MeshMaterial3d(purple.clone()),
                        Transform::from_xyz(q.x, g + 0.15, q.y)
                            .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                            .with_scale(Vec3::splat(rng.range(0.8, 1.3))),
                    ))
                    .id();
                urchins.push(e);
            }
            commands.spawn((
                Transform::from_xyz(p.x, 0.0, p.y),
                UrchinBed {
                    pos: p,
                    stock: URCHIN_MAX,
                    urchins,
                },
            ));
        }
    }
}

fn move_schools(
    time: Res<Time>,
    sea: Res<Sea>,
    mut rng: ResMut<Rng>,
    mut schools: Query<(&mut FishSchool, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut s, mut t) in &mut schools {
        s.heading += rng.range(-0.4, 0.4) * dt;
        let next = s.pos + heading_vec(s.heading) * 1.4 * dt;
        if depth_at(next) < 11.0 || next.length() > 540.0 {
            s.heading += PI * 0.5;
        } else {
            s.pos = next;
        }
        t.translation = Vec3::new(s.pos.x, sea_height(&sea, s.pos), s.pos.y);
        t.rotation = Quat::from_rotation_y(s.heading);
    }
}

fn animate_jumpers(
    time: Res<Time>,
    schools: Query<&FishSchool>,
    mut jumpers: Query<(&mut Jumper, &mut Transform, &ChildOf)>,
) {
    let dt = time.delta_secs();
    for (mut j, mut t, parent) in &mut jumpers {
        let lively = schools.get(parent.parent()).map(|s| s.stock / SCHOOL_MAX).unwrap_or(0.5);
        j.phase += dt;
        let cycle = 4.5 - lively * 2.0;
        if j.phase > cycle {
            j.phase -= cycle;
        }
        let u = j.phase / 0.7;
        if u < 1.0 {
            let d = heading_vec(j.dir);
            let along = (u - 0.5) * 2.0;
            t.translation = Vec3::new(j.offset.x + d.x * along, (u * PI).sin() * 1.1 - 0.1, j.offset.y + d.y * along);
            t.rotation = Quat::from_rotation_y(j.dir) * Quat::from_rotation_x((u - 0.5) * 2.2);
        } else {
            t.translation.y = -5.0;
        }
    }
}

fn regrow(
    time: Res<Time>,
    mut schools: Query<&mut FishSchool>,
    mut beds: Query<&mut UrchinBed>,
) {
    let dt = time.delta_secs();
    for mut s in &mut schools {
        s.stock = (s.stock + dt * 0.08).min(SCHOOL_MAX);
    }
    for mut b in &mut beds {
        b.stock = (b.stock + dt / 25.0).min(URCHIN_MAX);
    }
}

fn show_urchins(beds: Query<&UrchinBed, Changed<UrchinBed>>, mut vis: Query<&mut Visibility>) {
    for b in &beds {
        let shown = b.stock.floor() as usize;
        for (i, e) in b.urchins.iter().enumerate() {
            if let Ok(mut v) = vis.get_mut(*e) {
                *v = if i < shown { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn shark_ai(
    time: Res<Time>,
    mut boat: ResMut<BoatState>,
    mut activity: ResMut<Activity>,
    mut inv: ResMut<Inventory>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut rng: ResMut<Rng>,
    mut sharks: Query<&mut Shark>,
) {
    let dt = time.delta_secs();
    let boat_depth = boat.depth();
    let fishing = activity.kind == ActivityKind::Fishing;
    let mut warned = false;
    for mut s in &mut sharks {
        s.cooldown -= dt;
        let to_boat = boat.pos - s.pos;
        let d = to_boat.length();
        let interested = boat_depth > 5.0
            && s.cooldown <= 0.0
            && ((fishing && d < 80.0) || (boat.speed.abs() < 2.5 && d < 45.0) || (inv.fish > 12 && d < 35.0));

        let (goal, speed) = match s.state {
            SharkState::Wander => {
                if interested {
                    s.state = SharkState::Stalk(rng.range(6.0, 10.0));
                    if !warned {
                        warned = true;
                        log.push("A pating (shark) fin circles closer - sail away or lose your catch!");
                    }
                }
                if s.pos.distance(s.target) < 6.0 {
                    let a = rng.range(0.0, TAU);
                    let cand = s.home + Vec2::new(a.cos(), a.sin()) * rng.range(20.0, 120.0);
                    if depth_at(cand) > 10.0 {
                        s.target = cand;
                    }
                }
                (s.target, 3.2)
            }
            SharkState::Stalk(t) => {
                // Circle in, tightening.
                let ang = (s.pos - boat.pos).to_angle() + dt * 0.6;
                let radius = (d - dt * 2.5).max(10.0);
                s.state = if t - dt <= 0.0 { SharkState::Charge } else { SharkState::Stalk(t - dt) };
                (boat.pos + Vec2::from_angle(ang) * radius, 6.0)
            }
            SharkState::Charge => {
                if d < 3.8 {
                    let lost = inv.fish.div_ceil(2);
                    inv.fish -= lost;
                    boat.hull -= 18.0 * boat.tier.stats().frailty;
                    boat.speed *= 0.3;
                    activity.cancel();
                    sfx.play(Sfx::Splash, 1.0);
                    sfx.play(Sfx::Thud, 0.9);
                    log.push(format!("The shark rams your boat! {lost} fish lost and the hull is damaged."));
                    s.state = SharkState::Retreat(18.0);
                    s.cooldown = 35.0;
                }
                (boat.pos, 8.5)
            }
            SharkState::Retreat(t) => {
                s.state = if t - dt <= 0.0 { SharkState::Wander } else { SharkState::Retreat(t - dt) };
                (s.pos - to_boat.normalize_or_zero() * 20.0, 6.0)
            }
        };
        // Sharks give up in the shallows or when outrun.
        if s.hunting() && (boat_depth < 4.0 || d > 95.0) {
            s.state = SharkState::Wander;
            s.cooldown = 12.0;
        }

        let want = (goal - s.pos).to_angle();
        let want_heading = PI / 2.0 - want; // convert atan2(y,x) to our heading convention
        let diff = wrap_angle(want_heading - s.heading);
        s.heading = wrap_angle(s.heading + diff.clamp(-1.8 * dt, 1.8 * dt));
        let next = s.pos + heading_vec(s.heading) * speed * dt;
        if depth_at(next) > 5.5 {
            s.pos = next;
        } else {
            s.heading += 1.5 * dt * 4.0;
            if s.hunting() {
                s.state = SharkState::Wander;
            }
        }
    }
}

fn place_sharks(time: Res<Time>, sea: Res<Sea>, mut sharks: Query<(&Shark, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tr) in &mut sharks {
        let sway = (t * 3.0 + s.home.x).sin() * 0.12;
        tr.translation = Vec3::new(s.pos.x, sea_height(&sea, s.pos) - 0.1, s.pos.y);
        tr.rotation = Quat::from_rotation_y(s.heading + sway);
    }
}

#[allow(clippy::too_many_arguments)]
fn start_activity(
    keys: Res<ButtonInput<KeyCode>>,
    boat: Res<BoatState>,
    inv: Res<Inventory>,
    mut activity: ResMut<Activity>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut hints: ResMut<Hints>,
    schools: Query<&FishSchool>,
    beds: Query<(Entity, &UrchinBed)>,
    build: Res<crate::village::BuildMode>,
) {
    if activity.busy() || build.active {
        return;
    }
    let depth = boat.depth();
    let slow = boat.speed.abs() < 1.6;
    let full = inv.cargo() >= boat.tier.stats().capacity;
    let near_school = schools.iter().any(|s| s.pos.distance(boat.pos) < 32.0 && s.stock >= 1.0);
    let bed = beds
        .iter()
        .filter(|(_, b)| b.pos.distance(boat.pos) < 11.0 && b.stock >= 1.0)
        .min_by(|a, b| a.1.pos.distance(boat.pos).total_cmp(&b.1.pos.distance(boat.pos)));

    if depth > 1.2 {
        let hint = if near_school {
            "[F] Cast net - a school of fish is here!"
        } else if depth > 10.0 {
            "[F] Cast net (deep water)"
        } else {
            "[F] Cast net (shallows - small catch)"
        };
        hints.0.push(hint.to_string());
    }
    if bed.is_some() && depth < 7.0 {
        hints.0.push("[E] Dive for tayum (sea urchins)".to_string());
    }

    if keys.just_pressed(KeyCode::KeyF) && depth > 1.2 {
        if full {
            log.push("Your boat is full. Sell at Bongao market or unload at a village.");
        } else if !slow {
            log.push("Slow down (lower the sail with Space) before casting the net.");
        } else {
            *activity = Activity {
                kind: ActivityKind::Fishing,
                t: 0.0,
                dur: 4.0,
                target: None,
            };
            sfx.play(Sfx::Splash, 0.55);
        }
    }
    if keys.just_pressed(KeyCode::KeyE) {
        match bed {
            Some((e, _)) if depth < 7.0 => {
                if full {
                    log.push("Your boat is full.");
                } else if !slow {
                    log.push("Stop the boat before diving.");
                } else {
                    *activity = Activity {
                        kind: ActivityKind::Diving,
                        t: 0.0,
                        dur: 3.5,
                        target: Some(e),
                    };
                    sfx.play(Sfx::Splash, 0.7);
                    sfx.play_delayed(Sfx::Bubbles, 0.3, 0.6);
                }
            }
            _ => log.push("No tayum here. Look for dark purple clusters on the reef flats."),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn progress_activity(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut boat: ResMut<BoatState>,
    mut inv: ResMut<Inventory>,
    mut activity: ResMut<Activity>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut rng: ResMut<Rng>,
    mut schools: Query<&mut FishSchool>,
    mut beds: Query<&mut UrchinBed>,
) {
    boat.hidden_player = activity.kind == ActivityKind::Diving;
    if !activity.busy() {
        return;
    }
    let moving = boat.speed.abs() > 2.0
        || [KeyCode::KeyW, KeyCode::KeyS].iter().any(|k| keys.pressed(*k));
    if moving {
        log.push("You stopped before finishing.");
        activity.cancel();
        boat.hidden_player = false;
        return;
    }
    activity.t += time.delta_secs();
    if activity.t < activity.dur {
        return;
    }
    let room = boat.tier.stats().capacity.saturating_sub(inv.cargo());
    match activity.kind {
        ActivityKind::Fishing => {
            let depth = boat.depth();
            let bonus = match boat.tier {
                crate::boat::BoatTier::Banka => 0,
                crate::boat::BoatTier::Katig => 1,
                crate::boat::BoatTier::Balangay => 3,
            };
            let school = schools
                .iter_mut()
                .filter(|s| s.pos.distance(boat.pos) < 32.0 && s.stock >= 1.0)
                .min_by(|a, b| a.pos.distance(boat.pos).total_cmp(&b.pos.distance(boat.pos)));
            let caught = if let Some(mut s) = school {
                let n = (rng.int(3, 7) + bonus).min(s.stock as i32).max(0) as u32;
                s.stock -= n as f32;
                n
            } else if depth > 10.0 {
                rng.int(0, 2) as u32
            } else {
                rng.int(0, 1) as u32
            };
            let caught = caught.min(room);
            inv.fish += caught;
            sfx.play(Sfx::Splash, 0.4);
            log.push(match caught {
                0 => "The net comes up empty. Look for jumping fish in deep water.".to_string(),
                1 => "You haul in 1 fish.".to_string(),
                n => format!("You haul in {n} fish!"),
            });
        }
        ActivityKind::Diving => {
            if let Some(mut bed) = activity.target.and_then(|e| beds.get_mut(e).ok()) {
                let n = (rng.int(2, 4) as u32).min(bed.stock as u32).min(room);
                bed.stock -= n as f32;
                inv.urchin += n;
                log.push(format!("You surface with {n} tayum (sea urchins)."));
            }
            sfx.play(Sfx::Splash, 0.6);
        }
        ActivityKind::None => {}
    }
    activity.cancel();
    boat.hidden_player = false;
}

fn draw_activity(
    time: Res<Time>,
    boat: Res<BoatState>,
    activity: Res<Activity>,
    sea: Res<Sea>,
    mut gizmos: Gizmos,
) {
    let t = time.elapsed_secs();
    match activity.kind {
        ActivityKind::Fishing => {
            let side = Vec2::new(-boat.forward().y, boat.forward().x);
            let c = boat.pos + side * 6.0;
            let h = sea_height(&sea, c) + 0.05;
            let r = 1.5 + activity.progress().min(0.3) * 10.0;
            let iso = Isometry3d::new(Vec3::new(c.x, h, c.y), Quat::from_rotation_x(PI / 2.0));
            gizmos.circle(iso, r, Color::srgba(0.95, 0.95, 0.85, 0.8));
            gizmos.circle(iso, r * 0.6, Color::srgba(0.95, 0.95, 0.85, 0.5));
            let hand = Vec3::new(boat.pos.x, sea_height(&sea, boat.pos) + 1.4, boat.pos.y);
            gizmos.line(hand, Vec3::new(c.x, h, c.y), Color::srgba(0.9, 0.9, 0.8, 0.9));
        }
        ActivityKind::Diving => {
            let fwd = boat.forward();
            let side = Vec2::new(-fwd.y, fwd.x);
            for k in 0..6 {
                let ph = (t * 1.3 + k as f32 * 0.37) % 1.0;
                let p = boat.pos + side * 2.2 + Vec2::new((k as f32 * 2.1).sin(), (k as f32 * 1.3).cos()) * 0.8;
                let h = sea_height(&sea, p) + 0.05;
                let iso = Isometry3d::new(Vec3::new(p.x, h, p.y), Quat::from_rotation_x(PI / 2.0));
                gizmos.circle(iso, 0.2 + ph * 1.2, Color::srgba(1.0, 1.0, 1.0, 0.7 * (1.0 - ph)));
            }
        }
        ActivityKind::None => {}
    }
}
