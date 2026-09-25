//! Animated sea surface: a CPU-displaced patch that follows the camera,
//! surrounded by a flat ring reaching to the horizon.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::common::*;
use crate::world::seabed_height;

const PATCH_SIZE: f32 = 480.0;
const PATCH_SEG: usize = 160;

/// Global wave parameters, driven by the weather.
#[derive(Resource, Default)]
pub struct Sea {
    pub time: f32,
    /// 0 = calm, 1 = full storm.
    pub roughness: f32,
}

struct Wave {
    dir: Vec2,
    k: f32,
    speed: f32,
    amp: f32,
}

const WAVES: [(f32, f32, f32, f32); 5] = [
    // (direction angle, wavelength, speed, relative amplitude)
    (0.4, 38.0, 1.1, 1.0),
    (1.3, 23.0, 1.5, 0.6),
    (-0.6, 15.0, 1.9, 0.35),
    (2.4, 9.0, 2.4, 0.2),
    (0.9, 5.5, 3.1, 0.1),
];

fn waves() -> impl Iterator<Item = Wave> {
    WAVES.iter().map(|&(a, l, s, amp)| Wave {
        dir: Vec2::new(a.cos(), a.sin()),
        k: std::f32::consts::TAU / l,
        speed: s,
        amp,
    })
}

/// Shallow reef flats damp the swell.
pub fn depth_damping(depth: f32) -> f32 {
    0.35 + 0.65 * smoothstep(0.5, 12.0, depth)
}

fn base_amp(roughness: f32) -> f32 {
    0.28 + roughness * 1.5
}

/// Height and slope (dh/dx, dh/dz) of the sea surface.
pub fn wave_sample(p: Vec2, t: f32, roughness: f32, damping: f32) -> (f32, Vec2) {
    let a0 = base_amp(roughness) * damping;
    let mut h = 0.0;
    let mut grad = Vec2::ZERO;
    for w in waves() {
        let phase = w.k * w.dir.dot(p) + t * w.speed * (1.0 + roughness * 0.6);
        let amp = a0 * w.amp;
        h += amp * phase.sin();
        grad += w.dir * (amp * w.k * phase.cos());
    }
    (h, grad)
}

/// Sea surface height at a world point, including shallow-water damping.
pub fn sea_height(sea: &Sea, p: Vec2) -> f32 {
    let depth = (-seabed_height(p.x, p.y)).max(0.0);
    wave_sample(p, sea.time, sea.roughness, depth_damping(depth)).0
}

pub fn sea_normal(sea: &Sea, p: Vec2) -> Vec3 {
    let depth = (-seabed_height(p.x, p.y)).max(0.0);
    let (_, g) = wave_sample(p, sea.time, sea.roughness, depth_damping(depth));
    Vec3::new(-g.x, 1.0, -g.y).normalize()
}

#[derive(Resource)]
struct Patch {
    mesh: Handle<Mesh>,
    ring: Entity,
    center: Vec2,
    local: Vec<Vec2>,
    damping: Vec<f32>,
    base_colors: Vec<[f32; 4]>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    /// Per-lattice-point (damping, colour), filled lazily as the patch moves.
    cache: std::collections::HashMap<(i32, i32), (f32, [f32; 4])>,
}

#[derive(Component)]
struct WaterPatch;

/// Tint applied to all sea materials (storms turn it grey-green).
#[derive(Resource)]
pub struct SeaMaterials {
    pub patch: Handle<StandardMaterial>,
    pub ring: Handle<StandardMaterial>,
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sea>()
            .add_systems(Startup, spawn_water)
            .add_systems(PostUpdate, update_water.before(TransformSystems::Propagate));
    }
}

fn water_color(depth: f32) -> [f32; 4] {
    let shallow = Vec4::new(0.25, 0.95, 0.85, 0.32);
    let mid = Vec4::new(0.04, 0.62, 0.74, 0.7);
    let deep = Vec4::new(0.02, 0.22, 0.48, 0.94);
    let c = if depth < 6.0 {
        shallow.lerp(mid, smoothstep(0.5, 6.0, depth))
    } else {
        mid.lerp(deep, smoothstep(6.0, 18.0, depth))
    };
    // Foamy lip where the reef meets the sand.
    let foam = 1.0 - smoothstep(0.0, 0.7, depth);
    let c = c.lerp(Vec4::new(1.0, 1.0, 1.0, 0.85), foam * 0.8);
    c.to_array()
}

fn spawn_water(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let n = PATCH_SEG + 1;
    let step = PATCH_SIZE / PATCH_SEG as f32;
    let mut local = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            local.push(Vec2::new(
                -PATCH_SIZE / 2.0 + i as f32 * step,
                -PATCH_SIZE / 2.0 + j as f32 * step,
            ));
        }
    }
    let mut indices = Vec::with_capacity(PATCH_SEG * PATCH_SEG * 6);
    for j in 0..PATCH_SEG {
        for i in 0..PATCH_SEG {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let positions: Vec<[f32; 3]> = local.iter().map(|p| [p.x, 0.0, p.y]).collect();
    let normals = vec![[0.0, 1.0, 0.0]; local.len()];
    let colors = vec![[0.1, 0.5, 0.7, 0.7]; local.len()];
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors.clone());
    mesh.insert_indices(Indices::U32(indices));
    let mesh = meshes.add(mesh);

    let patch_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.12,
        reflectance: 0.6,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.035, 0.25, 0.46),
        perceptual_roughness: 0.15,
        reflectance: 0.5,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(patch_mat.clone()),
        Transform::default(),
        WaterPatch,
    ));

    // Flat ring with a square hole the size of the patch.
    let inner = PATCH_SIZE / 2.0 - 2.0;
    let outer = 3000.0;
    let mut ring_pos = Vec::new();
    let mut ring_idx: Vec<u32> = Vec::new();
    let quads = [
        (-outer, -outer, outer, -inner),
        (-outer, inner, outer, outer),
        (-outer, -inner, -inner, inner),
        (inner, -inner, outer, inner),
    ];
    for (x0, z0, x1, z1) in quads {
        let b = ring_pos.len() as u32;
        ring_pos.extend_from_slice(&[[x0, -0.25, z0], [x1, -0.25, z0], [x0, -0.25, z1], [x1, -0.25, z1]]);
        ring_idx.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
    }
    let ring_normals = vec![[0.0, 1.0, 0.0]; ring_pos.len()];
    let mut ring_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    ring_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, ring_pos);
    ring_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, ring_normals);
    ring_mesh.insert_indices(Indices::U32(ring_idx));
    let ring = commands
        .spawn((
            Mesh3d(meshes.add(ring_mesh)),
            MeshMaterial3d(ring_mat.clone()),
            Transform::default(),
        ))
        .id();

    commands.insert_resource(SeaMaterials {
        patch: patch_mat,
        ring: ring_mat,
    });
    let count = local.len();
    commands.insert_resource(Patch {
        mesh,
        ring,
        center: Vec2::splat(f32::MAX),
        local,
        damping: vec![1.0; count],
        base_colors: vec![[0.0; 4]; count],
        positions,
        normals,
        colors,
        cache: default(),
    });
}

fn update_water(
    time: Res<Time>,
    mut sea: ResMut<Sea>,
    patch: Option<ResMut<Patch>>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&Transform, With<MainCamera>>,
    mut water: Query<&mut Transform, (With<WaterPatch>, Without<MainCamera>)>,
    mut rings: Query<&mut Transform, (Without<WaterPatch>, Without<MainCamera>)>,
) {
    let Some(mut patch) = patch else { return };
    sea.time += time.delta_secs();
    let Ok(cam) = camera.single() else { return };

    // Snap the patch to its grid so vertices don't swim.
    let step = PATCH_SIZE / PATCH_SEG as f32 * 4.0;
    let fwd = cam.forward().as_vec3();
    let focus = Vec2::new(cam.translation.x, cam.translation.z) + Vec2::new(fwd.x, fwd.z) * 90.0;
    let center = (focus / step).round() * step;
    let patch = &mut *patch;
    if center != patch.center {
        patch.center = center;
        let cell = PATCH_SIZE / PATCH_SEG as f32;
        for i in 0..patch.local.len() {
            let w = center + patch.local[i];
            let key = ((w.x / cell).round() as i32, (w.y / cell).round() as i32);
            let (damp, color) = *patch.cache.entry(key).or_insert_with(|| {
                let depth = (-seabed_height(w.x, w.y)).max(0.0);
                (depth_damping(depth), water_color(depth))
            });
            patch.damping[i] = damp;
            patch.base_colors[i] = color;
        }
        if let Ok(mut t) = water.single_mut() {
            t.translation = Vec3::new(center.x, 0.0, center.y);
        }
        if let Ok(mut t) = rings.get_mut(patch.ring) {
            t.translation = Vec3::new(center.x, 0.0, center.y);
        }
    }

    let t = sea.time;
    let rough = sea.roughness;
    let foam_level = base_amp(rough) * 0.9;
    for i in 0..patch.local.len() {
        let l = patch.local[i];
        let (h, g) = wave_sample(center + l, t, rough, patch.damping[i]);
        patch.positions[i] = [l.x, h, l.y];
        let n = Vec3::new(-g.x, 1.0, -g.y).normalize();
        patch.normals[i] = n.to_array();
        let mut c = patch.base_colors[i];
        // Whitecaps on the crests when the sea gets up.
        let crest = smoothstep(foam_level * 0.55, foam_level * 1.1, h) * (0.25 + rough * 0.75);
        for k in 0..3 {
            c[k] += (1.0 - c[k]) * crest * 0.8;
        }
        c[3] += (0.95 - c[3]) * crest * 0.6;
        patch.colors[i] = c;
    }
    if let Some(mut mesh) = meshes.get_mut(&patch.mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, patch.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, patch.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, patch.colors.clone());
    }
}
