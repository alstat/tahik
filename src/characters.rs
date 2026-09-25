//! Chunky, big-headed toon characters built from primitives.

use bevy::prelude::*;

use crate::common::Rng;

#[derive(Resource)]
pub struct CharAssets {
    head: Handle<Mesh>,
    eye: Handle<Mesh>,
    pupil: Handle<Mesh>,
    nose: Handle<Mesh>,
    torso: Handle<Mesh>,
    limb: Handle<Mesh>,
    leg: Handle<Mesh>,
    skirt: Handle<Mesh>,
    hair: Handle<Mesh>,
    hat: Handle<Mesh>,
    pub skins: Vec<Handle<StandardMaterial>>,
    pub cloths: Vec<Handle<StandardMaterial>>,
    white: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    blush: Handle<StandardMaterial>,
    hair_mat: Handle<StandardMaterial>,
    straw: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Headwear {
    Hair,
    Scarf,
    Salakot,
}

#[derive(Clone, Copy)]
pub struct CharStyle {
    pub skin: usize,
    pub shirt: usize,
    pub sarong: usize,
    pub headwear: Headwear,
    pub scale: f32,
}

impl CharStyle {
    pub fn random(rng: &mut Rng, child: bool) -> Self {
        let headwear = match rng.index(5) {
            0 | 1 => Headwear::Hair,
            2 | 3 => Headwear::Scarf,
            _ => Headwear::Salakot,
        };
        Self {
            skin: rng.index(3),
            shirt: rng.index(8),
            sarong: rng.index(8),
            headwear,
            scale: if child { 0.68 } else { rng.range(0.95, 1.08) },
        }
    }
}

/// Root of an animated toon. `rig` is the child that bobs and squashes.
#[derive(Component)]
pub struct Toon {
    pub rig: Entity,
    pub eyes: [Entity; 2],
    pub phase: f32,
    pub moving: bool,
    pub blink_in: f32,
}

#[derive(Component)]
pub struct ToonRig;

#[derive(Component)]
pub struct ToonEye;

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_char_assets)
            .add_systems(Update, animate_toons);
    }
}

fn mat(materials: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: 0.85,
        reflectance: 0.2,
        ..default()
    })
}

fn setup_char_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let skins = [
        Color::srgb(0.55, 0.36, 0.24),
        Color::srgb(0.66, 0.45, 0.30),
        Color::srgb(0.45, 0.29, 0.19),
    ]
    .into_iter()
    .map(|c| mat(&mut materials, c))
    .collect();
    // Bright, sun-faded Sama cloth colours.
    let cloths = [
        Color::srgb(0.90, 0.30, 0.28),
        Color::srgb(0.98, 0.76, 0.22),
        Color::srgb(0.20, 0.62, 0.52),
        Color::srgb(0.55, 0.32, 0.72),
        Color::srgb(0.95, 0.52, 0.66),
        Color::srgb(0.22, 0.45, 0.80),
        Color::srgb(0.96, 0.58, 0.20),
        Color::srgb(0.40, 0.72, 0.30),
    ]
    .into_iter()
    .map(|c| mat(&mut materials, c))
    .collect();

    commands.insert_resource(CharAssets {
        head: meshes.add(Sphere::new(0.34).mesh().uv(24, 16)),
        eye: meshes.add(Sphere::new(0.1).mesh().uv(12, 8)),
        pupil: meshes.add(Sphere::new(0.05).mesh().uv(10, 6)),
        nose: meshes.add(Sphere::new(0.05).mesh().uv(8, 6)),
        torso: meshes.add(Capsule3d::new(0.2, 0.18)),
        limb: meshes.add(Capsule3d::new(0.06, 0.24)),
        leg: meshes.add(Capsule3d::new(0.075, 0.16)),
        skirt: meshes.add(ConicalFrustum {
            radius_top: 0.2,
            radius_bottom: 0.27,
            height: 0.3,
        }),
        hair: meshes.add(Sphere::new(0.36).mesh().uv(20, 12)),
        hat: meshes.add(Cone {
            radius: 0.55,
            height: 0.3,
        }),
        skins,
        cloths,
        white: mat(&mut materials, Color::srgb(0.98, 0.98, 0.96)),
        black: mat(&mut materials, Color::srgb(0.05, 0.04, 0.05)),
        blush: mat(&mut materials, Color::srgb(0.93, 0.52, 0.50)),
        hair_mat: mat(&mut materials, Color::srgb(0.08, 0.06, 0.06)),
        straw: mat(&mut materials, Color::srgb(0.86, 0.72, 0.42)),
    });
}

/// Spawns a toon at the origin of its own root entity and returns the root.
pub fn spawn_character(commands: &mut Commands, a: &CharAssets, style: CharStyle) -> Entity {
    let skin = a.skins[style.skin % a.skins.len()].clone();
    let shirt = a.cloths[style.shirt % a.cloths.len()].clone();
    let sarong = a.cloths[style.sarong % a.cloths.len()].clone();

    let root = commands
        .spawn((Transform::from_scale(Vec3::splat(style.scale)), Visibility::default()))
        .id();
    let rig = commands
        .spawn((Transform::default(), Visibility::default(), ToonRig, ChildOf(root)))
        .id();

    let part = |commands: &mut Commands, mesh: &Handle<Mesh>, m: &Handle<StandardMaterial>, t: Transform| {
        commands
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), t, ChildOf(rig)))
            .id()
    };

    for side in [-1.0, 1.0] {
        part(commands, &a.leg, &skin, Transform::from_xyz(0.09 * side, 0.16, 0.0));
        part(
            commands,
            &a.limb,
            &skin,
            Transform::from_xyz(0.27 * side, 0.6, 0.02)
                .with_rotation(Quat::from_rotation_z(0.35 * side)),
        );
    }
    part(commands, &a.skirt, &sarong, Transform::from_xyz(0.0, 0.38, 0.0));
    part(
        commands,
        &a.torso,
        &shirt,
        Transform::from_xyz(0.0, 0.64, 0.0).with_scale(Vec3::new(1.0, 1.0, 0.85)),
    );
    part(commands, &a.head, &skin, Transform::from_xyz(0.0, 1.1, 0.0));
    part(commands, &a.nose, &skin, Transform::from_xyz(0.0, 1.06, 0.34));

    let mut eyes = [Entity::PLACEHOLDER; 2];
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let eye = part(
            commands,
            &a.eye,
            &a.white,
            Transform::from_xyz(0.13 * side, 1.15, 0.27).with_scale(Vec3::new(1.0, 1.15, 0.6)),
        );
        commands.spawn((
            Mesh3d(a.pupil.clone()),
            MeshMaterial3d(a.black.clone()),
            Transform::from_xyz(0.0, -0.01, 0.075).with_scale(Vec3::new(1.0, 1.0, 1.4)),
            ChildOf(eye),
        ));
        eyes[i] = eye;
        part(
            commands,
            &a.nose,
            &a.blush,
            Transform::from_xyz(0.21 * side, 1.02, 0.25).with_scale(Vec3::new(1.2, 0.7, 0.6)),
        );
    }

    match style.headwear {
        Headwear::Hair => {
            part(
                commands,
                &a.hair,
                &a.hair_mat,
                Transform::from_xyz(0.0, 1.27, -0.07).with_scale(Vec3::new(1.02, 0.66, 1.0)),
            );
        }
        Headwear::Scarf => {
            part(
                commands,
                &a.hair,
                &sarong,
                Transform::from_xyz(0.0, 1.26, -0.08).with_scale(Vec3::new(1.04, 0.7, 1.02)),
            );
            part(
                commands,
                &a.eye,
                &sarong,
                Transform::from_xyz(0.0, 1.18, -0.4).with_scale(Vec3::new(1.4, 1.2, 1.2)),
            );
        }
        Headwear::Salakot => {
            part(
                commands,
                &a.hair,
                &a.hair_mat,
                Transform::from_xyz(0.0, 1.25, -0.07).with_scale(Vec3::new(1.0, 0.6, 1.0)),
            );
            part(commands, &a.hat, &a.straw, Transform::from_xyz(0.0, 1.52, 0.0));
        }
    }

    commands.entity(root).insert(Toon {
        rig,
        eyes,
        phase: (root.index_u32() as f32 * 1.37) % 6.28,
        moving: false,
        blink_in: 1.0 + (root.index_u32() % 7) as f32 * 0.6,
    });
    root
}

fn animate_toons(
    time: Res<Time>,
    mut toons: Query<&mut Toon>,
    mut rigs: Query<&mut Transform, (With<ToonRig>, Without<ToonEye>)>,
    mut eyes: Query<&mut Transform, Without<ToonRig>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for mut toon in &mut toons {
        let speed = if toon.moving { 11.0 } else { 2.2 };
        toon.phase += dt * speed;
        let p = toon.phase;
        if let Ok(mut rig) = rigs.get_mut(toon.rig) {
            if toon.moving {
                // Bouncy little hop with squash on landing.
                let hop = p.sin().abs();
                rig.translation.y = hop * 0.12;
                let s = (1.0 - hop) * 0.1;
                rig.scale = Vec3::new(1.0 + s, 1.0 - s, 1.0 + s);
                rig.rotation = Quat::from_rotation_z((p * 0.5).sin() * 0.08);
            } else {
                let b = p.sin() * 0.035;
                rig.translation.y = 0.0;
                rig.scale = Vec3::new(1.0 - b * 0.5, 1.0 + b, 1.0 - b * 0.5);
                rig.rotation = Quat::from_rotation_z((p * 0.35).sin() * 0.03);
            }
        }
        toon.blink_in -= dt;
        let closed = toon.blink_in < 0.0;
        if toon.blink_in < -0.12 {
            toon.blink_in = 2.0 + ((t * 13.7 + p).sin() * 0.5 + 0.5) * 3.5;
        }
        for e in toon.eyes {
            if let Ok(mut tr) = eyes.get_mut(e) {
                tr.scale.y = if closed { 0.12 } else { 1.15 };
            }
        }
    }
}
