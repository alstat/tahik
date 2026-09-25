//! The archipelago: sea floor, reef shoals, the market island and scenery.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::characters::{spawn_character, CharAssets, CharStyle, Headwear};
use crate::common::*;

pub struct ShoalDef {
    pub name: &'static str,
    pub center: Vec2,
    pub radius: f32,
    /// Optional exposed sandbar offset from the centre.
    pub sandbar: Option<Vec2>,
}

pub const SHOALS: [ShoalDef; 7] = [
    ShoalDef { name: "Sitangkai", center: Vec2::new(-190.0, 40.0), radius: 62.0, sandbar: None },
    ShoalDef { name: "Sibutu", center: Vec2::new(-60.0, -210.0), radius: 56.0, sandbar: Some(Vec2::new(18.0, -22.0)) },
    ShoalDef { name: "Tungkalang", center: Vec2::new(70.0, 150.0), radius: 66.0, sandbar: None },
    ShoalDef { name: "Lapid-Lapid", center: Vec2::new(230.0, -110.0), radius: 52.0, sandbar: None },
    ShoalDef { name: "Bulan", center: Vec2::new(340.0, 250.0), radius: 58.0, sandbar: Some(Vec2::new(-20.0, 16.0)) },
    ShoalDef { name: "Karang Puti", center: Vec2::new(130.0, -390.0), radius: 52.0, sandbar: None },
    ShoalDef { name: "Pasil", center: Vec2::new(-250.0, 360.0), radius: 54.0, sandbar: None },
];

pub const LAND_CENTER: Vec2 = Vec2::new(-440.0, -40.0);
pub const LAND_RADIUS: f32 = 170.0;
/// Where boats tie up at the Bongao market pier.
pub const MARKET_DOCK: Vec2 = Vec2::new(-300.0, -40.0);
pub const PLAYER_START: Vec2 = Vec2::new(-172.0, 30.0);

fn island_height(p: Vec2) -> f32 {
    let d = p.distance(LAND_CENTER) + fbm(p.x * 0.012, p.y * 0.012, 3, 91) * 14.0;
    let t = d / LAND_RADIUS;
    if t < 0.5 {
        1.5 + 9.0 * (1.0 - t / 0.5).powf(1.3)
    } else if t < 0.62 {
        1.5 - 2.0 * smoothstep(0.5, 0.62, t)
    } else {
        -0.5 - 23.0 * smoothstep(0.62, 1.0, t)
    }
}

/// Height of the sea floor (or land) at a point; sea level is 0.
pub fn seabed_height(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let mut h = -24.0 + fbm(x * 0.004, z * 0.004, 3, 7) * 6.0;
    for s in SHOALS.iter() {
        let d = p.distance(s.center) + fbm(x * 0.02, z * 0.02, 2, 33) * 10.0;
        let t = 1.0 - smoothstep(s.radius * 0.55, s.radius * 1.15, d);
        if t > 0.0 {
            let mut top = -1.9 + fbm(x * 0.05, z * 0.05, 3, 51) * 1.3;
            if let Some(off) = s.sandbar {
                let sd = p.distance(s.center + off);
                top += 2.6 * (1.0 - smoothstep(4.0, 16.0, sd));
            }
            h = h + (top - h) * t;
        }
    }
    h.max(island_height(p))
}

/// Water depth at a point (0 on land).
pub fn depth_at(p: Vec2) -> f32 {
    (-seabed_height(p.x, p.y)).max(0.0)
}

/// Which shoal (if any) a point belongs to.
pub fn shoal_at(p: Vec2) -> Option<usize> {
    SHOALS
        .iter()
        .position(|s| p.distance(s.center) < s.radius * 1.05)
}

pub fn is_land(p: Vec2) -> bool {
    seabed_height(p.x, p.y) > -0.45
}

// ------------------------------------------------------------ assets ---

/// Materials shared by buildings and props across modules.
#[derive(Resource)]
pub struct Props {
    pub cube: Handle<Mesh>,
    pub cyl: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub cone4: Handle<Mesh>,
    pub cone8: Handle<Mesh>,
    pub wood: Handle<StandardMaterial>,
    pub wood_dark: Handle<StandardMaterial>,
    pub bamboo: Handle<StandardMaterial>,
    pub thatch: Handle<StandardMaterial>,
    pub clay: Handle<StandardMaterial>,
    pub net: Handle<StandardMaterial>,
    pub seaweed: Handle<StandardMaterial>,
    pub buoy: Handle<StandardMaterial>,
    pub white: Handle<StandardMaterial>,
    pub lantern: Handle<StandardMaterial>,
    pub leaves: Vec<Handle<StandardMaterial>>,
    pub blossoms: Vec<Handle<StandardMaterial>>,
    pub bark: Handle<StandardMaterial>,
    pub trunk: Handle<StandardMaterial>,
    pub coconut: Handle<StandardMaterial>,
    pub fish: Handle<StandardMaterial>,
    pub dark: Handle<StandardMaterial>,
    pub water_in_jar: Handle<StandardMaterial>,
    pub paints: Vec<Handle<StandardMaterial>>,
    pub corals: Vec<Handle<StandardMaterial>>,
}

pub fn matte(materials: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: 0.9,
        reflectance: 0.15,
        ..default()
    })
}

fn setup_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let paints = [
        Color::srgb(0.86, 0.36, 0.30),
        Color::srgb(0.25, 0.62, 0.62),
        Color::srgb(0.96, 0.78, 0.30),
        Color::srgb(0.46, 0.38, 0.72),
        Color::srgb(0.95, 0.60, 0.55),
        Color::srgb(0.35, 0.66, 0.38),
    ]
    .into_iter()
    .map(|c| matte(&mut materials, c))
    .collect();
    let corals = [
        Color::srgb(0.98, 0.45, 0.50),
        Color::srgb(0.99, 0.62, 0.30),
        Color::srgb(0.70, 0.40, 0.85),
        Color::srgb(0.95, 0.85, 0.45),
        Color::srgb(0.40, 0.80, 0.70),
    ]
    .into_iter()
    .map(|c| matte(&mut materials, c))
    .collect();
    let props = Props {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cyl: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(10)),
        sphere: meshes.add(Sphere::new(0.5).mesh().uv(16, 10)),
        cone4: meshes.add(Cone { radius: 0.5, height: 1.0 }.mesh().resolution(4)),
        cone8: meshes.add(Cone { radius: 0.5, height: 1.0 }.mesh().resolution(10)),
        wood: matte(&mut materials, Color::srgb(0.62, 0.45, 0.30)),
        wood_dark: matte(&mut materials, Color::srgb(0.38, 0.27, 0.19)),
        bamboo: matte(&mut materials, Color::srgb(0.80, 0.70, 0.45)),
        thatch: matte(&mut materials, Color::srgb(0.78, 0.62, 0.38)),
        clay: matte(&mut materials, Color::srgb(0.72, 0.40, 0.26)),
        net: materials.add(StandardMaterial {
            base_color: Color::srgba(0.15, 0.2, 0.18, 0.55),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        seaweed: matte(&mut materials, Color::srgb(0.35, 0.62, 0.25)),
        buoy: matte(&mut materials, Color::srgb(0.98, 0.52, 0.18)),
        white: matte(&mut materials, Color::srgb(0.95, 0.94, 0.90)),
        lantern: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.45),
            emissive: LinearRgba::rgb(0.0, 0.0, 0.0),
            ..default()
        }),
        leaves: [
            Color::srgb(0.26, 0.58, 0.28),
            Color::srgb(0.36, 0.68, 0.30),
            Color::srgb(0.20, 0.48, 0.30),
            Color::srgb(0.46, 0.72, 0.34),
        ]
        .into_iter()
        .map(|c| matte(&mut materials, c))
        .collect(),
        blossoms: [
            Color::srgb(0.95, 0.36, 0.62), // bougainvillea
            Color::srgb(0.98, 0.38, 0.20), // flame tree
            Color::srgb(0.99, 0.82, 0.30), // golden shower
            Color::srgb(0.82, 0.50, 0.90), // banaba
        ]
        .into_iter()
        .map(|c| matte(&mut materials, c))
        .collect(),
        bark: matte(&mut materials, Color::srgb(0.42, 0.32, 0.24)),
        trunk: matte(&mut materials, Color::srgb(0.55, 0.43, 0.32)),
        coconut: matte(&mut materials, Color::srgb(0.42, 0.30, 0.16)),
        fish: materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.84, 0.90),
            metallic: 0.6,
            perceptual_roughness: 0.3,
            ..default()
        }),
        dark: matte(&mut materials, Color::srgb(0.12, 0.1, 0.1)),
        water_in_jar: matte(&mut materials, Color::srgb(0.3, 0.6, 0.75)),
        paints,
        corals,
    };
    commands.insert_resource(props);
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_props)
            .add_systems(Startup, (spawn_terrain, spawn_scenery));
    }
}

// ----------------------------------------------------------- terrain ---

fn terrain_color(x: f32, z: f32, h: f32) -> [f32; 4] {
    let n = fbm(x * 0.08, z * 0.08, 2, 5) * 0.5 + 0.5;
    let c = if h > 1.2 {
        // Land: grass and scrub, sandier near the beach.
        let g = Vec3::new(0.33, 0.58, 0.28).lerp(Vec3::new(0.22, 0.45, 0.22), n);
        Vec3::new(0.93, 0.86, 0.66).lerp(g, smoothstep(1.2, 2.6, h))
    } else if h > -1.0 {
        Vec3::new(0.98, 0.93, 0.78)
    } else if h > -7.0 {
        // Reef flats: bright sand with coral and seagrass patches.
        let sand = Vec3::new(0.95, 0.88, 0.70);
        let coral = fbm(x * 0.11, z * 0.11, 2, 77);
        let grass = fbm(x * 0.06, z * 0.06, 2, 13);
        let mut c = sand;
        if coral > 0.25 {
            c = c.lerp(Vec3::new(0.92, 0.55, 0.52), smoothstep(0.25, 0.45, coral));
        }
        if grass > 0.3 {
            c = c.lerp(Vec3::new(0.45, 0.62, 0.35), smoothstep(0.3, 0.5, grass) * 0.8);
        }
        c.lerp(Vec3::new(0.55, 0.62, 0.58), smoothstep(-3.0, -7.0, h))
    } else {
        Vec3::new(0.50, 0.58, 0.58).lerp(Vec3::new(0.10, 0.16, 0.24), smoothstep(-7.0, -20.0, h))
    };
    [c.x, c.y, c.z, 1.0]
}

fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const N: usize = 280;
    const SIZE: f32 = 1500.0;
    let step = SIZE / N as f32;
    let mut positions = Vec::with_capacity((N + 1) * (N + 1));
    let mut colors = Vec::with_capacity((N + 1) * (N + 1));
    for j in 0..=N {
        for i in 0..=N {
            let x = -SIZE / 2.0 + i as f32 * step;
            let z = -SIZE / 2.0 + j as f32 * step;
            let h = seabed_height(x, z);
            positions.push([x, h, z]);
            colors.push(terrain_color(x, z, h));
        }
    }
    let mut indices = Vec::with_capacity(N * N * 6);
    for j in 0..N {
        for i in 0..N {
            let a = (j * (N + 1) + i) as u32;
            let b = a + 1;
            let c = a + (N + 1) as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.1,
            ..default()
        })),
        Transform::default(),
    ));
}

// ---------------------------------------------------------- scenery ---

/// A coconut palm with a curving, ringed trunk and arching fronds.
pub fn spawn_palm(commands: &mut Commands, props: &Props, base: Vec3, rng: &mut Rng) {
    let root = commands
        .spawn((
            Transform::from_translation(base).with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU))),
            Visibility::default(),
        ))
        .id();
    let lean = rng.range(0.06, 0.2);
    let height = rng.range(6.5, 10.5);
    let segs = 8;
    let mut top = Vec3::ZERO;
    for s in 0..segs {
        let t0 = s as f32 / segs as f32;
        let t1 = (s + 1) as f32 / segs as f32;
        let p0 = Vec3::new(lean * height * t0 * t0 * 2.0, height * t0, 0.0);
        let p1 = Vec3::new(lean * height * t1 * t1 * 2.0, height * t1, 0.0);
        let dir = (p1 - p0).normalize();
        let w = 0.34 - 0.13 * t0;
        let m = if s % 2 == 0 { &props.trunk } else { &props.bark };
        commands.spawn((
            Mesh3d(props.cyl.clone()),
            MeshMaterial3d(m.clone()),
            Transform::from_translation((p0 + p1) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
                .with_scale(Vec3::new(w, p0.distance(p1) * 1.05, w)),
            ChildOf(root),
        ));
        top = p1;
    }
    let leaf = props.leaves[rng.index(props.leaves.len())].clone();
    let leaf2 = props.leaves[rng.index(props.leaves.len())].clone();
    let fronds = 10;
    for k in 0..fronds {
        let yaw = k as f32 / fronds as f32 * TAU + rng.range(-0.2, 0.2);
        let lift = rng.range(-0.35, 0.1);
        let length = rng.range(3.6, 4.8);
        // Each frond arches up and then droops, in four tapering pieces.
        let mut p = top;
        let pieces = 4;
        for i in 0..pieces {
            let t = i as f32 / pieces as f32;
            let pitch = lift + t * 1.3;
            let d = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Vec3::Z;
            let seg = length / pieces as f32;
            let mid = p + d * seg * 0.5;
            let width = 1.05 * (1.0 - t * 0.55);
            commands.spawn((
                Mesh3d(props.sphere.clone()),
                MeshMaterial3d(if i % 2 == 0 { leaf.clone() } else { leaf2.clone() }),
                Transform::from_translation(mid)
                    .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch))
                    .with_scale(Vec3::new(width, 0.1, seg * 1.25)),
                ChildOf(root),
            ));
            p += d * seg;
        }
    }
    for k in 0..rng.int(2, 5) {
        let a = k as f32 * 1.7;
        commands.spawn((
            Mesh3d(props.sphere.clone()),
            MeshMaterial3d(props.coconut.clone()),
            Transform::from_translation(top + Vec3::new(a.cos() * 0.35, -0.4, a.sin() * 0.35)).with_scale(Vec3::splat(0.42)),
            ChildOf(root),
        ));
    }
}

/// A lush, rounded canopy tree (talisay / narra style).
pub fn spawn_canopy_tree(commands: &mut Commands, props: &Props, base: Vec3, rng: &mut Rng, blossom: Option<Handle<StandardMaterial>>) {
    let root = commands
        .spawn((
            Transform::from_translation(base).with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU))),
            Visibility::default(),
        ))
        .id();
    let height = rng.range(3.5, 6.0);
    let girth = rng.range(0.35, 0.55);
    commands.spawn((
        Mesh3d(props.cyl.clone()),
        MeshMaterial3d(props.bark.clone()),
        Transform::from_xyz(0.0, height / 2.0, 0.0).with_scale(Vec3::new(girth, height, girth)),
        ChildOf(root),
    ));
    // A couple of branches reaching into the crown.
    for k in 0..2 {
        let a = k as f32 * PI + rng.range(-0.5, 0.5);
        let dir = Vec3::new(a.cos() * 0.8, 1.0, a.sin() * 0.8).normalize();
        commands.spawn((
            Mesh3d(props.cyl.clone()),
            MeshMaterial3d(props.bark.clone()),
            Transform::from_translation(Vec3::new(0.0, height * 0.85, 0.0) + dir * 0.9)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
                .with_scale(Vec3::new(girth * 0.5, 2.0, girth * 0.5)),
            ChildOf(root),
        ));
    }
    let size = rng.range(2.6, 3.8);
    let crown = Vec3::new(0.0, height + size * 0.6, 0.0);
    let leaf = props.leaves[rng.index(props.leaves.len())].clone();
    let leaf2 = props.leaves[rng.index(props.leaves.len())].clone();
    for k in 0..rng.int(6, 9) {
        let a = rng.range(0.0, TAU);
        let r = rng.range(0.4, 1.0) * size;
        let off = Vec3::new(a.cos() * r, rng.range(-0.4, 0.7) * size, a.sin() * r);
        let s = size * rng.range(0.7, 1.05);
        let m = match (&blossom, k % 3) {
            (Some(b), 0) => b.clone(),
            _ if k % 2 == 0 => leaf.clone(),
            _ => leaf2.clone(),
        };
        commands.spawn((
            Mesh3d(props.sphere.clone()),
            MeshMaterial3d(m),
            Transform::from_translation(crown + off).with_scale(Vec3::new(s * 1.8, s * 1.4, s * 1.8)),
            ChildOf(root),
        ));
    }
    if let Some(b) = blossom {
        // Petals dotted over the crown.
        for _ in 0..8 {
            let a = rng.range(0.0, TAU);
            let off = Vec3::new(a.cos() * size * 1.3, rng.range(0.2, 1.1) * size, a.sin() * size * 1.3);
            commands.spawn((
                Mesh3d(props.sphere.clone()),
                MeshMaterial3d(b.clone()),
                Transform::from_translation(crown + off).with_scale(Vec3::splat(size * 0.55)),
                ChildOf(root),
            ));
        }
    }
}

fn spawn_bush(commands: &mut Commands, props: &Props, base: Vec3, rng: &mut Rng) {
    let leaf = props.leaves[rng.index(props.leaves.len())].clone();
    for _ in 0..rng.int(2, 4) {
        let s = rng.range(0.9, 1.7);
        let off = Vec3::new(rng.range(-0.9, 0.9), s * 0.35, rng.range(-0.9, 0.9));
        commands.spawn((
            Mesh3d(props.sphere.clone()),
            MeshMaterial3d(leaf.clone()),
            Transform::from_translation(base + off).with_scale(Vec3::new(s * 1.4, s, s * 1.4)),
        ));
    }
    if rng.chance(0.3) {
        let b = props.blossoms[rng.index(props.blossoms.len())].clone();
        for _ in 0..3 {
            commands.spawn((
                Mesh3d(props.sphere.clone()),
                MeshMaterial3d(b.clone()),
                Transform::from_translation(base + Vec3::new(rng.range(-0.8, 0.8), rng.range(0.6, 1.2), rng.range(-0.8, 0.8)))
                    .with_scale(Vec3::splat(0.45)),
            ));
        }
    }
}

fn boxy(
    commands: &mut Commands,
    props: &Props,
    m: &Handle<StandardMaterial>,
    pos: Vec3,
    size: Vec3,
) -> Entity {
    commands
        .spawn((
            Mesh3d(props.cube.clone()),
            MeshMaterial3d(m.clone()),
            Transform::from_translation(pos).with_scale(size),
        ))
        .id()
}

fn post(commands: &mut Commands, props: &Props, x: f32, z: f32, top: f32, r: f32) {
    let bottom = seabed_height(x, z).min(top - 0.5) - 0.5;
    let h = top - bottom;
    commands.spawn((
        Mesh3d(props.cyl.clone()),
        MeshMaterial3d(props.wood_dark.clone()),
        Transform::from_xyz(x, bottom + h / 2.0, z).with_scale(Vec3::new(r, h, r)),
    ));
}

fn spawn_scenery(
    mut commands: Commands,
    props: Res<Props>,
    chars: Res<CharAssets>,
    mut rng: ResMut<Rng>,
) {
    let props = props.into_inner();
    // Bongao's greenery: palms along the beaches, leafy and flowering trees
    // inland, and bushes everywhere - keeping the town and market clear.
    let clearings = [
        (Vec2::new(-352.0, -40.0), 26.0),
        (Vec2::new(-455.0, -30.0), 16.0),
        (Vec2::new(-392.0, -70.0), 8.0),
        (Vec2::new(-398.0, -12.0), 8.0),
        (Vec2::new(-420.0, 20.0), 8.0),
        (Vec2::new(-415.0, -95.0), 8.0),
        (Vec2::new(-440.0, -60.0), 8.0),
        (Vec2::new(-380.0, 30.0), 8.0),
    ];
    let mut plant = |rng: &mut Rng, count: usize, hmin: f32, hmax: f32, f: &mut dyn FnMut(&mut Commands, &mut Rng, Vec3)| {
        let mut placed = 0;
        let mut tries = 0;
        while placed < count && tries < 6000 {
            tries += 1;
            let a = rng.range(0.0, TAU);
            let r = rng.range(0.0, LAND_RADIUS * 0.66);
            let p = LAND_CENTER + Vec2::new(a.cos(), a.sin()) * r;
            let h = seabed_height(p.x, p.y);
            if h > hmin && h < hmax && clearings.iter().all(|(c, cr)| p.distance(*c) > *cr) {
                f(&mut commands, rng, Vec3::new(p.x, h - 0.2, p.y));
                placed += 1;
            }
        }
    };
    plant(&mut rng, 70, 0.9, 4.5, &mut |c, rng, at| spawn_palm(c, props, at, rng));
    plant(&mut rng, 22, 4.5, 11.0, &mut |c, rng, at| spawn_palm(c, props, at, rng));
    plant(&mut rng, 55, 3.0, 11.0, &mut |c, rng, at| spawn_canopy_tree(c, props, at, rng, None));
    plant(&mut rng, 16, 2.0, 9.0, &mut |c, rng, at| {
        let b = props.blossoms[rng.index(props.blossoms.len())].clone();
        spawn_canopy_tree(c, props, at, rng, Some(b));
    });
    plant(&mut rng, 110, 1.3, 11.0, &mut |c, rng, at| spawn_bush(c, props, at, rng));
    for s in SHOALS.iter() {
        if let Some(off) = s.sandbar {
            for k in 0..3 {
                let p = s.center + off + Vec2::new(k as f32 * 2.5 - 2.5, (k as f32 * 1.7).sin() * 2.0);
                let h = seabed_height(p.x, p.y);
                if h > 0.0 {
                    spawn_palm(&mut commands, props, Vec3::new(p.x, h - 0.2, p.y), &mut rng);
                }
            }
        }
    }

    // Coral heads on the reef flats (visible through clear shallow water).
    for s in SHOALS.iter() {
        let mut n = 0;
        let mut tries = 0;
        while n < 22 && tries < 400 {
            tries += 1;
            let a = rng.range(0.0, 6.28);
            let r = rng.range(0.0, s.radius * 1.1);
            let p = s.center + Vec2::new(a.cos(), a.sin()) * r;
            let h = seabed_height(p.x, p.y);
            if !(-5.0..-0.9).contains(&h) {
                continue;
            }
            n += 1;
            for _ in 0..rng.int(2, 4) {
                let m = props.corals[rng.index(props.corals.len())].clone();
                let off = Vec3::new(rng.range(-0.8, 0.8), 0.0, rng.range(-0.8, 0.8));
                let sz = rng.range(0.5, 1.1);
                let mesh = if rng.chance(0.5) { props.sphere.clone() } else { props.cone8.clone() };
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(m),
                    Transform::from_translation(Vec3::new(p.x, h + sz * 0.3, p.y) + off)
                        .with_scale(Vec3::new(sz, sz * rng.range(0.6, 1.3), sz)),
                ));
            }
        }
    }

    // --- Bongao market: a pier, stalls, vendors and a small town. ---
    let deck = 2.2;
    let (x0, x1, z) = (-350.0, -304.0, MARKET_DOCK.y);
    boxy(&mut commands, props, &props.wood, Vec3::new((x0 + x1) / 2.0, deck, z), Vec3::new(x1 - x0, 0.25, 4.0));
    boxy(&mut commands, props, &props.wood, Vec3::new(x1 - 4.0, deck, z), Vec3::new(10.0, 0.25, 12.0));
    let mut x = x0 + 3.0;
    while x < x1 {
        for dz in [-1.8, 1.8] {
            post(&mut commands, props, x, z + dz, deck, 0.25);
        }
        x += 5.0;
    }
    for (dx, dz) in [(-8.0, -5.5), (-8.0, 5.5), (0.0, -5.5), (0.0, 5.5)] {
        post(&mut commands, props, x1 - 4.0 + dx * 0.5, z + dz, deck, 0.3);
    }
    // Stalls with striped awnings on the landward end.
    for (k, (sx, sz)) in [(-356.0, -50.0), (-358.0, -30.0), (-368.0, -44.0), (-366.0, -58.0)]
        .into_iter()
        .enumerate()
    {
        let g = seabed_height(sx, sz);
        let paint = props.paints[k % props.paints.len()].clone();
        boxy(&mut commands, props, &props.wood, Vec3::new(sx, g + 0.5, sz), Vec3::new(4.0, 1.0, 2.5));
        for (px, pz) in [(-1.9, -1.2), (1.9, -1.2), (-1.9, 1.2), (1.9, 1.2)] {
            boxy(&mut commands, props, &props.wood_dark, Vec3::new(sx + px, g + 1.5, sz + pz), Vec3::new(0.15, 3.0, 0.15));
        }
        for stripe in 0..4 {
            let m = if stripe % 2 == 0 { paint.clone() } else { props.white.clone() };
            commands.spawn((
                Mesh3d(props.cube.clone()),
                MeshMaterial3d(m),
                Transform::from_xyz(sx - 1.5 + stripe as f32, g + 3.1, sz)
                    .with_rotation(Quat::from_rotation_z(0.0))
                    .with_scale(Vec3::new(1.0, 0.12, 3.2)),
            ));
        }
        // Baskets of goods.
        for b in 0..3 {
            commands.spawn((
                Mesh3d(props.cyl.clone()),
                MeshMaterial3d(props.thatch.clone()),
                Transform::from_xyz(sx - 1.2 + b as f32 * 1.2, g + 1.25, sz + 0.3)
                    .with_scale(Vec3::new(0.8, 0.5, 0.8)),
            ));
        }
        let v = spawn_character(&mut commands, &chars, CharStyle::random(&mut rng, false));
        commands.entity(v).insert(
            Transform::from_xyz(sx, g, sz + 2.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        );
    }
    // Market folk strolling on the pier.
    for k in 0..3 {
        let mut style = CharStyle::random(&mut rng, k == 2);
        style.headwear = Headwear::Salakot;
        let v = spawn_character(&mut commands, &chars, style);
        commands.entity(v).insert(
            Transform::from_xyz(-330.0 + k as f32 * 7.0, deck + 0.12, z + 1.0)
                .with_rotation(Quat::from_rotation_y(1.2 + k as f32))
                .with_scale(Vec3::splat(if k == 2 { 0.7 } else { 1.0 })),
        );
    }
    // Town houses and a masjid on the hill.
    for (k, (hx, hz)) in [(-392.0, -70.0), (-398.0, -12.0), (-420.0, 20.0), (-415.0, -95.0), (-440.0, -60.0), (-380.0, 30.0)]
        .into_iter()
        .enumerate()
    {
        let g = seabed_height(hx, hz);
        let paint = props.paints[(k + 2) % props.paints.len()].clone();
        boxy(&mut commands, props, &paint, Vec3::new(hx, g + 2.0, hz), Vec3::new(7.0, 4.0, 6.0));
        commands.spawn((
            Mesh3d(props.cone4.clone()),
            MeshMaterial3d(props.clay.clone()),
            Transform::from_xyz(hx, g + 5.3, hz)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                .with_scale(Vec3::new(10.5, 2.6, 9.5)),
        ));
    }
    let (mx, mz) = (-455.0, -30.0);
    let g = seabed_height(mx, mz);
    boxy(&mut commands, props, &props.white, Vec3::new(mx, g + 3.0, mz), Vec3::new(12.0, 6.0, 12.0));
    let green = props.paints[5].clone();
    commands.spawn((
        Mesh3d(props.sphere.clone()),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(mx, g + 6.5, mz).with_scale(Vec3::new(8.0, 7.0, 8.0)),
    ));
    boxy(&mut commands, props, &props.white, Vec3::new(mx + 8.0, g + 7.0, mz + 8.0), Vec3::new(1.8, 14.0, 1.8));
    commands.spawn((
        Mesh3d(props.cone8.clone()),
        MeshMaterial3d(green),
        Transform::from_xyz(mx + 8.0, g + 15.2, mz + 8.0).with_scale(Vec3::new(2.4, 2.5, 2.4)),
    ));
}
