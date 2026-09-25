//! Tahik - a Sama Dilaut sea-village builder.

mod audio;
mod boat;
mod characters;
mod common;
mod debug;
mod families;
mod fishing;
mod icons;
mod market;
mod prayer;
mod ui;
mod village;
mod water;
mod weather;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.8, 0.88, 0.95)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tahik - Sama Dilaut".into(),
                resolution: (1600u32, 900u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            common::CommonPlugin,
            icons::IconsPlugin,
            characters::CharacterPlugin,
            world::WorldPlugin,
            water::WaterPlugin,
            weather::WeatherPlugin,
            boat::BoatPlugin,
            fishing::FishingPlugin,
            village::VillagePlugin,
            families::FamiliesPlugin,
            prayer::PrayerPlugin,
            market::MarketPlugin,
            ui::UiPlugin,
            audio::NatureAudioPlugin,
            debug::DebugPlugin,
        ))
        .run();
}
