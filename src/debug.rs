//! Developer hooks driven by environment variables:
//! - `TAHIK_AUTOSTART`  skip the title screen
//! - `TAHIK_DEMO`       pre-build a village at Sitangkai
//! - `TAHIK_TIME=0.5`   start at a time of day (0..1)
//! - `TAHIK_STORM`      start with a storm raging
//! - `TAHIK_SAIL`       start with the sail raised
//! - `TAHIK_AT=x,z`     start the boat at a world position
//! - `TAHIK_KEYS=3:B,4:Enter`  simulate key taps at given seconds
//! - `TAHIK_HOLD_W`     hold W (paddle forward) after two seconds
//! - `TAHIK_TASKS=2,4`  give the starting families these tasks (1-7) at start
//! - `TAHIK_BLUEPRINT`  place a stilt house blueprint at Sitangkai at start
//! - `TAHIK_SPEED=4`    start at a game speed
//! - `TAHIK_ECHO`       print game log messages to the terminal
//! - `TAHIK_CAM=x,z,dist,pitch,yaw`  point the overseer camera
//! - `TAHIK_WORKSITES`  place a fishing ground, woodcutter and nipa camp at start
//! - `TAHIK_CAT=1`      open a build-bar category at start
//! - `TAHIK_SHOT=path`  save a screenshot after `TAHIK_SHOT_AT` seconds (default 6) and quit

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::boat::BoatState;
use crate::weather::{Clock, StormPhase, Weather};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_time)
            .add_systems(Update, (screenshot, echo_log))
            .add_systems(OnEnter(crate::common::GameState::Playing), setup_scenario)
            .add_systems(PreUpdate, fake_keys.after(bevy::input::InputSystems));
    }
}

fn set_time(
    mut clock: ResMut<Clock>,
    mut weather: ResMut<Weather>,
    mut boat: ResMut<BoatState>,
    mut orbit: ResMut<crate::boat::OrbitCam>,
) {
    if let Ok(v) = std::env::var("TAHIK_CAM") {
        let n: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        if n.len() == 5 {
            orbit.free = Vec2::new(n[0], n[1]);
            orbit.dist = n[2];
            orbit.pitch = n[3];
            orbit.yaw = n[4];
        }
    }
    if std::env::var("TAHIK_STORM").is_ok() {
        weather.phase = StormPhase::Raging(90.0);
        weather.storm = 1.0;
        weather.rain = 1.0;
    }
    if let Some((x, z)) = std::env::var("TAHIK_AT").ok().and_then(|v| {
        let (x, z) = v.split_once(',')?;
        Some((x.parse::<f32>().ok()?, z.parse::<f32>().ok()?))
    }) {
        boat.pos = Vec2::new(x, z);
    }
    if std::env::var("TAHIK_SAIL").is_ok() {
        boat.sail_up = true;
    }
    if let Some(t) = std::env::var("TAHIK_TIME").ok().and_then(|v| v.parse::<f32>().ok()) {
        clock.time = t;
    }
}

fn screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<u8>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TAHIK_SHOT") else { return };
    let at: f32 = std::env::var("TAHIK_SHOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let t = time.elapsed_secs();
    if *done == 0 && t > at {
        *done = 1;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    } else if *done == 1 && t > at + 1.5 {
        *done = 2;
        exit.write(AppExit::Success);
    }
}


fn fake_keys(time: Res<Time>, mut keys: ResMut<ButtonInput<KeyCode>>, mut fired: Local<Vec<bool>>) {
    if std::env::var("TAHIK_HOLD_W").is_ok() && time.elapsed_secs() > 2.0 {
        keys.press(KeyCode::KeyW);
    }
    let Ok(spec) = std::env::var("TAHIK_KEYS") else { return };
    let taps: Vec<(f32, KeyCode)> = spec
        .split(',')
        .filter_map(|t| {
            let (at, key) = t.split_once(':')?;
            let code = match key {
                "B" => KeyCode::KeyB,
                "G" => KeyCode::KeyG,
                "W" => KeyCode::KeyW,
                "Enter" => KeyCode::Enter,
                "1" => KeyCode::Digit1,
                "2" => KeyCode::Digit2,
                "3" => KeyCode::Digit3,
                _ => return None,
            };
            Some((at.parse().ok()?, code))
        })
        .collect();
    fired.resize(taps.len(), false);
    for (i, (at, code)) in taps.iter().enumerate() {
        if !fired[i] && time.elapsed_secs() > *at {
            fired[i] = true;
            keys.press(*code);
        } else if fired[i] {
            keys.release(*code);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn setup_scenario(
    mut commands: Commands,
    props: Res<crate::world::Props>,
    mut rng: ResMut<crate::common::Rng>,
    mut communities: ResMut<crate::village::Communities>,
    mut log: ResMut<crate::common::GameLog>,
    mut families: Query<&mut crate::families::Family>,
    mut vtime: ResMut<Time<Virtual>>,
    chars: Res<crate::characters::CharAssets>,
    mut build_mode: ResMut<crate::village::BuildMode>,
) {
    if let Some(c) = std::env::var("TAHIK_CAT").ok().and_then(|v| v.parse::<usize>().ok()) {
        build_mode.category = Some(c);
    }
    if std::env::var("TAHIK_WORKSITES").is_ok() {
        use crate::village::Kind;
        let home = crate::world::SHOALS[0].center;
        let mut place = |kind: Kind, ok: &dyn Fn(Vec2) -> bool, from: Vec2| {
            for r in (10..400).step_by(6) {
                for k in 0..24 {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    let p = from + Vec2::new(a.cos(), a.sin()) * r as f32;
                    if ok(p) {
                        crate::village::place_worksite(&mut commands, &props, &chars, &mut rng, &communities, &mut log, kind, p);
                        return;
                    }
                }
            }
        };
        place(Kind::FishingGround, &|p| crate::world::depth_at(p) > 12.0 && crate::world::shoal_at(p).is_none(), home);
        let land_ok = |p: Vec2| {
            let h = crate::world::seabed_height(p.x, p.y);
            (1.0..6.0).contains(&h) && p.distance(crate::world::MARKET_DOCK) > 75.0
        };
        place(Kind::Woodcutter, &land_ok, crate::world::MARKET_DOCK + Vec2::new(0.0, 80.0));
        place(Kind::NipaGrove, &land_ok, crate::world::MARKET_DOCK + Vec2::new(0.0, -85.0));
    }
    if let Some(speed) = std::env::var("TAHIK_SPEED").ok().and_then(|v| v.parse::<f32>().ok()) {
        vtime.set_relative_speed(speed);
    }
    if let Ok(spec) = std::env::var("TAHIK_TASKS") {
        let tasks: Vec<usize> = spec.split(',').filter_map(|t| t.parse().ok()).collect();
        let mut fams: Vec<_> = families.iter_mut().collect();
        fams.sort_by_key(|f| f.name);
        fams.reverse(); // Jalani, Hadja
        for (f, t) in fams.iter_mut().zip(tasks) {
            if (1..=7).contains(&t) {
                crate::families::set_task(f, crate::families::TASKS[t - 1]);
            }
        }
    }
    if std::env::var("TAHIK_BLUEPRINT").is_ok() {
        let c = crate::world::SHOALS[0].center;
        let p = c + Vec2::new(-8.0, 6.0);
        crate::village::place_blueprint(
            &mut commands,
            &props,
            &build_mode,
            &mut rng,
            &mut communities,
            &mut log,
            crate::village::Kind::House,
            0,
            p,
        );
    }
}

fn echo_log(log: Res<crate::common::GameLog>, mut seen: Local<usize>, time: Res<Time<Virtual>>) {
    if std::env::var("TAHIK_ECHO").is_err() {
        return;
    }
    let fresh: Vec<_> = log.entries.iter().filter(|e| e.age == 0.0).collect();
    for e in &fresh {
        info!("[{:.0}s] {}", time.elapsed_secs(), e.text);
    }
    *seen += fresh.len();
}
