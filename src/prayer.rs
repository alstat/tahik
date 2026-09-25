//! The five daily prayers. At each prayer time the azan sounds from the
//! langgal and Bongao's masjid, and everyone stops work to pray.

use bevy::audio::Volume;
use bevy::prelude::*;
use std::sync::Arc;

use crate::audio::{azan_sound, wav};
use crate::common::*;
use crate::weather::Clock;

/// Prayer names and their time of day (fraction of the day).
pub const PRAYERS: [(&str, f32); 5] = [
    ("Fajr", 4.9 / 24.0),
    ("Dhuhr", 12.2 / 24.0),
    ("Asr", 15.5 / 24.0),
    ("Maghrib", 18.2 / 24.0),
    ("Isha", 19.6 / 24.0),
];

/// Game seconds that work stops for each prayer.
const PRAYER_LENGTH: f32 = 7.0;

#[derive(Resource, Default)]
pub struct Prayer {
    pub active: Option<&'static str>,
    remaining: f32,
    last_time: Option<f32>,
    azan: Option<Handle<AudioSource>>,
}

impl Prayer {
    /// The next prayer and how many game hours until it.
    pub fn next(&self, time: f32) -> (&'static str, f32) {
        PRAYERS
            .iter()
            .map(|(n, t)| (*n, (t - time).rem_euclid(1.0)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(n, d)| (n, d * 24.0))
            .unwrap_or(("Fajr", 0.0))
    }
}

pub struct PrayerPlugin;

impl Plugin for PrayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Prayer>()
            .add_systems(Startup, load_azan)
            .add_systems(Update, prayer_times.run_if(in_state(GameState::Playing)));
    }
}

fn load_azan(mut prayer: ResMut<Prayer>, server: Res<AssetServer>, mut sources: ResMut<Assets<AudioSource>>) {
    prayer.azan = Some(if std::path::Path::new("assets/azan.ogg").exists() {
        server.load("azan.ogg")
    } else {
        sources.add(AudioSource {
            bytes: Arc::from(wav(&azan_sound())),
        })
    });
}

fn prayer_times(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<Clock>,
    mut prayer: ResMut<Prayer>,
    mut log: ResMut<GameLog>,
) {
    let now = clock.time;
    let last = prayer.last_time.unwrap_or(now);
    prayer.last_time = Some(now);
    // Did the clock pass a prayer time this frame (allowing for midnight)?
    let crossed = PRAYERS.iter().find(|(_, t)| {
        if now >= last { last < *t && *t <= now } else { *t > last || *t <= now }
    });
    if let Some((name, _)) = crossed {
        prayer.active = Some(name);
        prayer.remaining = PRAYER_LENGTH;
        log.push(format!("Allahu akbar - the azan calls the faithful to {name} prayer. Work stops."));
        if let Some(h) = prayer.azan.clone() {
            commands.spawn((
                AudioPlayer(h),
                PlaybackSettings {
                    volume: Volume::Linear(0.75),
                    ..PlaybackSettings::DESPAWN
                },
            ));
        }
    }
    if let Some(name) = prayer.active {
        prayer.remaining -= time.delta_secs();
        if prayer.remaining <= 0.0 {
            prayer.active = None;
            log.push(format!("{name} prayer is over. Everyone returns to work."));
        }
    }
}
