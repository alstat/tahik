//! Day/night, seasons, wind, rain, storms (ribut) and the sky that shows them.

use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_4, TAU};

use crate::common::*;
use crate::water::{Sea, SeaMaterials};
use crate::world::Props;

pub const DAY_LENGTH: f32 = 150.0;
pub const SEASON_DAYS: u32 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Season {
    Wet,
    Dry,
}

impl Season {
    pub fn label(self) -> &'static str {
        match self {
            Season::Wet => "Wet season - Habagat monsoon",
            Season::Dry => "Dry season - Amihan winds",
        }
    }
    /// Direction the prevailing wind blows towards.
    fn prevailing(self) -> f32 {
        match self {
            Season::Wet => 3.0 * FRAC_PI_4,
            Season::Dry => -FRAC_PI_4,
        }
    }
}

#[derive(Resource)]
pub struct Clock {
    pub day: u32,
    pub time: f32,
    last_day_announced: u32,
}

impl Clock {
    pub fn season(&self) -> Season {
        if ((self.day - 1) / SEASON_DAYS) % 2 == 0 {
            Season::Wet
        } else {
            Season::Dry
        }
    }
    pub fn day_in_season(&self) -> u32 {
        (self.day - 1) % SEASON_DAYS + 1
    }
    pub fn hour_string(&self) -> String {
        let mins = (self.time * 24.0 * 60.0) as u32;
        format!("{:02}:{:02}", mins / 60, mins % 60)
    }
    pub fn sun_elevation(&self) -> f32 {
        ((self.time - 0.25) * TAU).sin()
    }
    pub fn daylight(&self) -> f32 {
        smoothstep(-0.12, 0.2, self.sun_elevation())
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StormPhase {
    Clear,
    Brewing(f32),
    Raging(f32),
}

#[derive(Resource)]
pub struct Weather {
    pub wind_dir: f32,
    pub wind_strength: f32,
    target_dir: f32,
    target_strength: f32,
    wind_timer: f32,
    pub rain: f32,
    rain_target: f32,
    rain_timer: f32,
    pub storm: f32,
    pub phase: StormPhase,
    storm_at: Option<f32>,
    pub flash: f32,
    next_bolt: f32,
    bolt: Option<(Vec3, f32, u32)>,
}

impl Weather {
    pub fn wind_vec(&self) -> Vec2 {
        heading_vec(self.wind_dir) * self.wind_strength
    }
    pub fn describe(&self) -> &'static str {
        match self.phase {
            StormPhase::Raging(_) => "RIBUT! Storm raging",
            StormPhase::Brewing(_) => "Dark clouds gathering",
            StormPhase::Clear if self.rain > 0.55 => "Heavy rain",
            StormPhase::Clear if self.rain > 0.12 => "Light rain",
            _ => "Fair",
        }
    }
    pub fn raging(&self) -> bool {
        matches!(self.phase, StormPhase::Raging(_))
    }
}

#[derive(Component)]
struct Sun;
#[derive(Component)]
struct SkyDome;
#[derive(Component)]
struct SunDisc;
#[derive(Component)]
struct MoonDisc;
#[derive(Component)]
struct Cloud {
    drift: f32,
}

#[derive(Resource)]
struct SkyAssets {
    dome: Handle<Mesh>,
    cloud_mat: Handle<StandardMaterial>,
    sun_mat: Handle<StandardMaterial>,
    glow_mats: Vec<Handle<StandardMaterial>>,
    star_mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct SunGlow(f32);
#[derive(Component)]
struct StarField;

#[derive(Resource, Default)]
struct RainDrops(Vec<Vec3>);

struct Wisp {
    pos: Vec3,
    life: f32,
    max: f32,
}

#[derive(Resource, Default)]
struct Wisps(Vec<Wisp>);

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Clock {
            day: 1,
            time: 0.27,
            last_day_announced: 0,
        })
        .insert_resource(Weather {
            wind_dir: Season::Wet.prevailing(),
            wind_strength: 0.7,
            target_dir: Season::Wet.prevailing(),
            target_strength: 0.7,
            wind_timer: 20.0,
            rain: 0.0,
            rain_target: 0.0,
            rain_timer: 45.0,
            storm: 0.0,
            phase: StormPhase::Clear,
            storm_at: None,
            flash: 0.0,
            next_bolt: 3.0,
            bolt: None,
        })
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            brightness: 600.0,
            ..default()
        })
        .init_resource::<RainDrops>()
        .init_resource::<Wisps>()
        .add_systems(Startup, setup_sky)
        .add_systems(
            Update,
            (advance_clock, update_weather, lightning).chain().run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (update_sky, move_clouds, draw_rain, draw_wisps));
    }
}

fn setup_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 220.0,
            first_cascade_far_bound: 30.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(100.0, 200.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        Sun,
    ));

    let mut dome = Sphere::new(1400.0).mesh().uv(32, 18);
    let n = dome.count_vertices();
    dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 1.0, 1.0, 1.0]; n]);
    let dome = meshes.add(dome);
    commands.spawn((
        Mesh3d(dome.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        SkyDome,
    ));
    let disc = meshes.add(Sphere::new(1.0).mesh().uv(16, 10));
    let sun_mat = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(6.0, 4.6, 2.8)),
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    commands.spawn((
        Mesh3d(disc.clone()),
        MeshMaterial3d(sun_mat.clone()),
        Transform::from_scale(Vec3::splat(38.0)),
        SunDisc,
    ));
    // Soft additive halos around the sun, strongest at dawn and dusk.
    let mut glow_mats = Vec::new();
    for size in [80.0, 150.0, 280.0] {
        let m = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            alpha_mode: AlphaMode::Add,
            unlit: true,
            fog_enabled: false,
            ..default()
        });
        commands.spawn((Mesh3d(disc.clone()), MeshMaterial3d(m.clone()), Transform::from_scale(Vec3::splat(size)), SunGlow(size)));
        glow_mats.push(m);
    }
    // Stars for clear nights.
    let star_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        alpha_mode: AlphaMode::Add,
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let field = commands.spawn((Transform::default(), Visibility::default(), StarField)).id();
    for _ in 0..520 {
        let y = rng.range(0.08, 1.0);
        let a = rng.range(0.0, TAU);
        let r = (1.0 - y * y).sqrt();
        let d = Vec3::new(a.cos() * r, y, a.sin() * r);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::from_translation(d * 1250.0).with_scale(Vec3::splat(rng.range(1.2, 3.2))),
            ChildOf(field),
        ));
    }
    commands.spawn((
        Mesh3d(disc),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::rgb(1.6, 1.7, 2.0)),
            unlit: true,
            fog_enabled: false,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(26.0)),
        MoonDisc,
    ));

    // Puffy cumulus built from overlapping blobs.
    let cloud_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let blob = meshes.add(Sphere::new(1.0).mesh().uv(14, 9));
    for _ in 0..48 {
        let pos = Vec3::new(rng.range(-1000.0, 1000.0), rng.range(110.0, 170.0), rng.range(-1000.0, 1000.0));
        let root = commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                Cloud { drift: rng.range(0.7, 1.3) },
            ))
            .id();
        let size = rng.range(14.0, 30.0);
        for _ in 0..rng.int(4, 7) {
            let off = Vec3::new(rng.range(-2.0, 2.0), rng.range(-0.2, 0.6), rng.range(-1.2, 1.2)) * size;
            let s = size * rng.range(0.55, 1.0);
            commands.spawn((
                Mesh3d(blob.clone()),
                MeshMaterial3d(cloud_mat.clone()),
                Transform::from_translation(off).with_scale(Vec3::new(s * 1.2, s * 0.62, s)),
                ChildOf(root),
            ));
        }
    }
    commands.insert_resource(SkyAssets {
        dome,
        cloud_mat,
        sun_mat,
        glow_mats,
        star_mat,
    });
}

fn advance_clock(
    time: Res<Time>,
    mut clock: ResMut<Clock>,
    mut weather: ResMut<Weather>,
    mut log: ResMut<GameLog>,
    mut rng: ResMut<Rng>,
) {
    clock.time += time.delta_secs() / DAY_LENGTH;
    if clock.time >= 1.0 {
        clock.time -= 1.0;
        clock.day += 1;
    }
    if clock.last_day_announced != clock.day {
        clock.last_day_announced = clock.day;
        let season = clock.season();
        if clock.day_in_season() == 1 {
            match season {
                Season::Wet => log.push(format!(
                    "Day {}: The Habagat monsoon arrives. Rain fills the jars, but storms will come.",
                    clock.day
                )),
                Season::Dry => log.push(format!(
                    "Day {}: The Amihan dry season begins. Rain will be rare - guard your water.",
                    clock.day
                )),
            }
        }
        // Storms roll in during the wet season (not on the very first day).
        if season == Season::Wet && clock.day >= 2 && rng.chance(0.55) {
            weather.storm_at = Some(rng.range(0.15, 0.8));
        }
    }
}

fn update_weather(
    time: Res<Time>,
    clock: Res<Clock>,
    mut weather: ResMut<Weather>,
    mut sea: ResMut<Sea>,
    mut log: ResMut<GameLog>,
    mut rng: ResMut<Rng>,
) {
    let dt = time.delta_secs();
    let season = clock.season();
    let w = &mut *weather;

    // Storm lifecycle.
    if let Some(at) = w.storm_at
        && clock.time >= at
        && w.phase == StormPhase::Clear
    {
        w.storm_at = None;
        w.phase = StormPhase::Brewing(35.0);
        log.push("Dark clouds build in the southwest - a ribut (storm) is coming! Shelter in the shallows.");
    }
    w.phase = match w.phase {
        StormPhase::Brewing(t) if t - dt <= 0.0 => {
            log.push("The storm breaks! Deep water is deadly now.");
            StormPhase::Raging(rng.range(45.0, 65.0))
        }
        StormPhase::Brewing(t) => StormPhase::Brewing(t - dt),
        StormPhase::Raging(t) if t - dt <= 0.0 => {
            log.push("The storm passes. Check your stilt houses for damage.");
            w.rain_timer = 20.0;
            StormPhase::Clear
        }
        StormPhase::Raging(t) => StormPhase::Raging(t - dt),
        StormPhase::Clear => StormPhase::Clear,
    };
    let storm_target = match w.phase {
        StormPhase::Clear => 0.0,
        StormPhase::Brewing(t) => 0.25 + 0.2 * (1.0 - t / 35.0),
        StormPhase::Raging(_) => 1.0,
    };
    w.storm += (storm_target - w.storm) * (1.0 - (-dt * 0.25).exp());

    // Rain showers.
    w.rain_timer -= dt;
    if w.rain_timer <= 0.0 {
        let raining = w.rain_target > 0.0;
        match (season, raining) {
            (_, true) => {
                w.rain_target = 0.0;
                w.rain_timer = match season {
                    Season::Wet => rng.range(15.0, 45.0),
                    Season::Dry => rng.range(90.0, 220.0),
                };
            }
            (Season::Wet, false) => {
                w.rain_target = rng.range(0.4, 1.0);
                w.rain_timer = rng.range(20.0, 50.0);
            }
            (Season::Dry, false) => {
                w.rain_target = if rng.chance(0.35) { rng.range(0.2, 0.45) } else { 0.0 };
                w.rain_timer = rng.range(8.0, 16.0);
            }
        }
    }
    let rain_goal = match w.phase {
        StormPhase::Raging(_) => 1.0,
        StormPhase::Brewing(t) if t < 15.0 => w.rain_target.max(0.5),
        _ => w.rain_target,
    };
    w.rain += (rain_goal - w.rain) * (1.0 - (-dt * 0.4).exp());

    // Wind: wanders around the prevailing monsoon direction.
    w.wind_timer -= dt;
    if w.wind_timer <= 0.0 {
        w.wind_timer = rng.range(18.0, 40.0);
        w.target_dir = season.prevailing() + rng.range(-0.8, 0.8);
        w.target_strength = rng.range(0.45, 1.0);
    }
    let (goal_dir, goal_strength) = if w.storm > 0.3 {
        (w.wind_dir + 0.04, 1.2 + 0.8 * w.storm)
    } else {
        (w.target_dir, w.target_strength)
    };
    w.wind_dir = wrap_angle(w.wind_dir + wrap_angle(goal_dir - w.wind_dir) * (1.0 - (-dt * 0.08).exp()));
    w.wind_strength += (goal_strength - w.wind_strength) * (1.0 - (-dt * 0.2).exp());

    sea.roughness = (w.storm * 0.9 + (w.wind_strength - 0.5).max(0.0) * 0.12).min(1.0);
}

fn lightning(
    time: Res<Time>,
    mut weather: ResMut<Weather>,
    mut sfx: ResMut<SfxQueue>,
    mut rng: ResMut<Rng>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs();
    weather.flash = (weather.flash - dt * 3.5).max(0.0);
    if let Some((p, t, seed)) = weather.bolt {
        weather.bolt = if t - dt > 0.0 { Some((p, t - dt, seed)) } else { None };
    }
    if !weather.raging() || weather.storm < 0.6 {
        return;
    }
    weather.next_bolt -= dt;
    if weather.next_bolt <= 0.0 {
        weather.next_bolt = rng.range(2.5, 8.0);
        weather.flash = 1.0;
        let Ok(cam) = camera.single() else { return };
        let a = rng.range(0.0, TAU);
        let dist = rng.range(90.0, 320.0);
        let p = Vec3::new(cam.translation.x + a.cos() * dist, 0.0, cam.translation.z + a.sin() * dist);
        weather.bolt = Some((p, 0.25, rng.next_u32()));
        sfx.play_delayed(Sfx::Thunder, dist / 340.0 * 2.0, (1.4 - dist / 400.0).clamp(0.3, 1.0));
    }
}

#[allow(clippy::too_many_arguments)]
fn update_sky(
    clock: Res<Clock>,
    weather: Res<Weather>,
    sky: Option<Res<SkyAssets>>,
    props: Option<Res<Props>>,
    sea_mats: Option<Res<SeaMaterials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    camera: Query<&Transform, With<MainCamera>>,
    mut fog: Query<&mut DistanceFog>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), (With<Sun>, Without<MainCamera>)>,
    mut discs: Query<
        (&mut Transform, Has<SunDisc>, Has<SkyDome>),
        (Or<(With<SunDisc>, With<MoonDisc>, With<SkyDome>)>, Without<Sun>, Without<MainCamera>),
    >,
    mut moon_vis: Query<&mut Visibility, With<MoonDisc>>,
    mut glows: Query<(&mut Transform, &SunGlow), (Without<Sun>, Without<MainCamera>, Without<SunDisc>, Without<MoonDisc>, Without<SkyDome>)>,
    mut stars: Query<&mut Transform, (With<StarField>, Without<Sun>, Without<MainCamera>, Without<SunDisc>, Without<MoonDisc>, Without<SkyDome>, Without<SunGlow>)>,
) {
    let (Some(sky), Some(props), Some(sea_mats)) = (sky, props, sea_mats) else { return };
    let Ok(cam) = camera.single() else { return };

    let e = clock.sun_elevation();
    let day = clock.daylight();
    let golden = (1.0 - smoothstep(0.0, 0.5, e)) * smoothstep(-0.25, 0.0, e);
    let overcast = weather.storm.max(weather.rain * 0.6);

    let c = |r: f32, g: f32, b: f32| Color::srgb(r, g, b);
    let mut zen = lerp_color(c(0.02, 0.03, 0.09), c(0.22, 0.52, 0.95), day);
    let mut hor = lerp_color(c(0.07, 0.09, 0.18), c(0.70, 0.86, 0.97), day);
    zen = lerp_color(zen, c(0.30, 0.32, 0.68), golden * 0.75);
    hor = lerp_color(hor, c(1.0, 0.60, 0.30), golden * 0.9);
    // Opposite the sun the horizon turns rose and lilac at dawn and dusk.
    let mut hor_away = lerp_color(hor, c(0.92, 0.58, 0.66), golden * 0.85);
    let grey = 0.15 + 0.85 * day;
    zen = lerp_color(zen, c(0.26 * grey, 0.29 * grey, 0.33 * grey), overcast * 0.9);
    hor = lerp_color(hor, c(0.44 * grey, 0.48 * grey, 0.52 * grey), overcast * 0.85);
    hor_away = lerp_color(hor_away, c(0.44 * grey, 0.48 * grey, 0.52 * grey), overcast * 0.85);
    zen = lerp_color(zen, c(0.85, 0.88, 1.0), weather.flash * 0.6);
    hor = lerp_color(hor, c(0.85, 0.88, 1.0), weather.flash * 0.6);
    hor_away = lerp_color(hor_away, c(0.85, 0.88, 1.0), weather.flash * 0.6);
    let fog_col = lerp_color(hor_away, hor, 0.55);
    clear.0 = fog_col;
    let a = (clock.time - 0.25) * TAU;
    let sun_dir = Vec3::new(a.cos() * 0.8, a.sin(), 0.45).normalize();

    // Sky dome gradient.
    if let Some(mut mesh) = meshes.get_mut(&sky.dome) {
        let dirs: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| Vec3::from_array(*v) / 1400.0).collect(),
            _ => Vec::new(),
        };
        let (zl, hl, al) = (zen.to_linear().to_vec3(), hor.to_linear().to_vec3(), hor_away.to_linear().to_vec3());
        let glow_col = Vec3::new(1.0, 0.62, 0.32);
        let glow_amt = (0.2 * day + 1.3 * golden) * (1.0 - overcast);
        let colors: Vec<[f32; 4]> = dirs
            .iter()
            .map(|d| {
                let toward = d.dot(sun_dir);
                let side = ((toward + 1.0) * 0.5).powf(2.5);
                let horizon = al.lerp(hl, side);
                let t = smoothstep(-0.02, 0.5, d.y);
                let mut col = horizon.lerp(zl, t);
                // A warm bloom of light around the sun.
                col += glow_col * toward.max(0.0).powf(10.0) * glow_amt * (1.0 - t * 0.6);
                [col.x, col.y, col.z, 1.0]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }

    // Sun & moon.
    let moon_dir = -sun_dir + Vec3::new(0.0, 0.25, 0.0);
    let moon_dir = moon_dir.normalize();
    for (mut t, is_sun, is_dome) in &mut discs {
        if is_dome {
            t.translation = cam.translation;
            continue;
        }
        let d = if is_sun { sun_dir } else { moon_dir };
        t.translation = cam.translation + d * 1300.0;
        if is_sun {
            t.scale = Vec3::splat(38.0 * (1.0 + golden * 0.6));
        }
    }
    for mut v in &mut moon_vis {
        *v = if day < 0.6 && overcast < 0.7 { Visibility::Inherited } else { Visibility::Hidden };
    }
    let sun_col = lerp_color(c(1.0, 0.95, 0.86), c(1.0, 0.55, 0.30), golden);
    if let Some(mut m) = materials.get_mut(&sky.sun_mat) {
        let hot = LinearRgba::rgb(6.0, 4.6, 2.8);
        let red = LinearRgba::rgb(7.0, 2.4, 0.8);
        let k = golden;
        m.base_color = Color::LinearRgba(LinearRgba::rgb(
            hot.red + (red.red - hot.red) * k,
            hot.green + (red.green - hot.green) * k,
            hot.blue + (red.blue - hot.blue) * k,
        ) * (1.0 - overcast * 0.9));
    }
    let glow_strength = (0.25 * day + 1.0 * golden) * (1.0 - overcast) * smoothstep(-0.12, 0.0, e);
    for (i, h) in sky.glow_mats.iter().enumerate() {
        if let Some(mut m) = materials.get_mut(h) {
            let k = [0.22, 0.10, 0.045][i] * glow_strength;
            m.base_color = Color::LinearRgba(LinearRgba::rgb(1.0 * k, 0.55 * k, 0.25 * k));
        }
    }
    if let Some(mut m) = materials.get_mut(&sky.star_mat) {
        // Stars come out only once the sun is well below the horizon.
        let k = smoothstep(-0.05, -0.25, e) * (1.0 - overcast).powi(2) * 2.5;
        m.base_color = Color::LinearRgba(LinearRgba::rgb(k, k, k * 1.1));
    }
    for (mut t, g) in &mut glows {
        t.translation = cam.translation + sun_dir * 1290.0;
        t.scale = Vec3::splat(g.0 * (1.0 + golden * 0.4));
    }
    for mut t in &mut stars {
        t.translation = cam.translation;
    }
    if let Ok((mut light, mut lt)) = sun.single_mut() {
        let (dir, col, lux) = if e > -0.05 {
            (sun_dir, sun_col, 10500.0 * day * (1.0 - 0.72 * overcast))
        } else {
            (moon_dir, c(0.6, 0.7, 1.0), 900.0 * (1.0 - 0.8 * overcast))
        };
        light.illuminance = lux.max(250.0);
        light.color = col;
        *lt = Transform::from_translation(cam.translation + dir * 300.0).looking_at(cam.translation, Vec3::Y);
    }
    // Golden hour bathes everything in a warm, rosy light.
    ambient.color = lerp_color(lerp_color(c(0.45, 0.55, 0.9), c(0.78, 0.86, 1.0), day), c(1.0, 0.72, 0.62), golden * 0.7);
    ambient.brightness =
        320.0 + 520.0 * day * (1.0 - 0.3 * overcast) + 380.0 * golden * (1.0 - overcast) + weather.flash * 5000.0;

    for mut f in &mut fog {
        f.color = fog_col;
        f.directional_light_color = lerp_color(Color::NONE, sun_col.with_alpha(0.5), day * (1.0 - overcast));
        f.directional_light_exponent = 24.0;
        let (start, end) = (150.0 - 120.0 * overcast, 720.0 - 500.0 * overcast * overcast);
        f.falloff = FogFalloff::Linear { start, end };
    }

    // Clouds take on the mood of the sky.
    if let Some(mut m) = materials.get_mut(&sky.cloud_mat) {
        let base = lerp_color(c(0.98, 0.97, 0.95), c(1.0, 0.62, 0.52), golden * 0.9);
        let dark = c(0.34, 0.36, 0.40);
        let col = lerp_color(base, dark, overcast * 0.85);
        let lum = 0.12 + 0.88 * day;
        let l = col.to_linear();
        m.base_color = Color::LinearRgba(LinearRgba::rgb(l.red * lum, l.green * lum, l.blue * lum));
    }
    if let Some(mut m) = materials.get_mut(&props.lantern) {
        let n = 1.0 - smoothstep(0.1, 0.5, day) + overcast * 0.4;
        m.emissive = LinearRgba::rgb(9.0, 5.0, 1.8) * n.min(1.0);
    }
    if let Some(mut m) = materials.get_mut(&sea_mats.patch) {
        m.base_color = lerp_color(Color::WHITE, c(0.55, 0.62, 0.62), overcast * 0.7);
    }
    if let Some(mut m) = materials.get_mut(&sea_mats.ring) {
        m.base_color = lerp_color(c(0.035, 0.25, 0.46), c(0.12, 0.18, 0.22), overcast * 0.7);
    }
}

fn move_clouds(
    time: Res<Time>,
    weather: Res<Weather>,
    camera: Query<&Transform, With<MainCamera>>,
    mut clouds: Query<(&mut Transform, &Cloud), Without<MainCamera>>,
) {
    let Ok(cam) = camera.single() else { return };
    let wind = weather.wind_vec();
    let speed = 3.0 + weather.storm * 12.0;
    for (mut t, cloud) in &mut clouds {
        t.translation.x += wind.x * speed * cloud.drift * time.delta_secs();
        t.translation.z += wind.y * speed * cloud.drift * time.delta_secs();
        for (axis, c) in [(0, cam.translation.x), (2, cam.translation.z)] {
            let v = &mut t.translation[axis];
            if *v - c > 1000.0 {
                *v -= 2000.0;
            } else if *v - c < -1000.0 {
                *v += 2000.0;
            }
        }
    }
}

fn draw_rain(
    time: Res<Time>,
    weather: Res<Weather>,
    mut drops: ResMut<RainDrops>,
    mut rng: ResMut<Rng>,
    camera: Query<&Transform, With<MainCamera>>,
    mut gizmos: Gizmos,
) {
    let Ok(cam) = camera.single() else { return };
    const MAX: usize = 1800;
    if drops.0.is_empty() {
        drops.0 = (0..MAX).map(|_| Vec3::new(0.0, -100.0, 0.0)).collect();
    }
    let count = (weather.rain * MAX as f32) as usize;
    let wind = weather.wind_vec() * (4.0 + weather.storm * 10.0);
    let vel = Vec3::new(wind.x, -30.0, wind.y);
    let fwd = cam.forward().as_vec3();
    let center = cam.translation + Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero() * 25.0;
    let col = Color::srgba(0.78, 0.84, 0.92, 0.45);
    let dt = time.delta_secs();
    for p in drops.0.iter_mut().take(count) {
        *p += vel * dt;
        if p.y < 0.0 || (p.x - center.x).abs() > 60.0 || (p.z - center.z).abs() > 60.0 {
            *p = Vec3::new(
                center.x + rng.range(-60.0, 60.0),
                rng.range(2.0, 40.0),
                center.z + rng.range(-60.0, 60.0),
            );
        }
        gizmos.line(*p, *p - vel * 0.045, col);
    }

    if let Some((p, _, seed)) = weather.bolt {
        let mut r = Rng::new(seed as u64 + 1);
        let mut pts = vec![p + Vec3::new(0.0, 150.0, 0.0)];
        let mut cur = pts[0];
        while cur.y > 0.0 {
            cur += Vec3::new(r.range(-9.0, 9.0), -r.range(10.0, 20.0), r.range(-9.0, 9.0));
            pts.push(cur.with_y(cur.y.max(0.0)));
        }
        gizmos.linestrip(pts, Color::LinearRgba(LinearRgba::rgb(4.0, 4.0, 6.0)));
    }
}

fn draw_wisps(
    time: Res<Time>,
    weather: Res<Weather>,
    mut wisps: ResMut<Wisps>,
    mut rng: ResMut<Rng>,
    camera: Query<&Transform, With<MainCamera>>,
    mut gizmos: Gizmos,
) {
    let Ok(cam) = camera.single() else { return };
    let dt = time.delta_secs();
    let dir = heading_vec(weather.wind_dir);
    let dir3 = Vec3::new(dir.x, 0.0, dir.y);
    let side = Vec3::new(-dir.y, 0.0, dir.x);
    let speed = 6.0 + weather.wind_strength * 10.0;
    let wanted = (10.0 + weather.wind_strength * 22.0) as usize;
    let fwd = cam.forward().as_vec3();
    let center = cam.translation + Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero() * 30.0;
    while wisps.0.len() < wanted {
        wisps.0.push(Wisp {
            pos: Vec3::new(
                center.x + rng.range(-45.0, 45.0),
                rng.range(1.0, 7.0),
                center.z + rng.range(-45.0, 45.0),
            ),
            life: 0.0,
            max: rng.range(1.5, 3.5),
        });
    }
    let t = time.elapsed_secs();
    wisps.0.retain_mut(|w| {
        w.life += dt;
        w.pos += dir3 * speed * dt;
        w.life < w.max
    });
    for (i, w) in wisps.0.iter().enumerate() {
        let fade = (w.life / w.max * std::f32::consts::PI).sin();
        let col = Color::srgba(1.0, 1.0, 1.0, 0.35 * fade);
        let pts = (0..6).map(|k| {
            let s = k as f32 * 1.1;
            w.pos + dir3 * s + side * ((t * 3.0 + s * 0.9 + i as f32).sin() * 0.25)
        });
        gizmos.linestrip(pts, col);
    }
}
