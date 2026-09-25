//! Bongao market on the neighbouring land: sell the catch, buy timber and
//! water, repair and upgrade the boat.

use bevy::prelude::*;

use crate::boat::BoatState;
use crate::common::*;
use crate::fishing::Hints;
use crate::village::BuildMode;
use crate::weather::{Clock, Season};
use crate::world::{LAND_CENTER, LAND_RADIUS, MARKET_DOCK};

#[derive(Resource, Default)]
pub struct MarketUi {
    pub open: bool,
    was_near: bool,
}

/// Something the player can do at the market (from a key or a button click).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarketAction {
    Sell,
    Wood,
    Water,
    Repair,
    Upgrade,
    Leave,
}

/// Market buttons clicked in the UI this frame.
#[derive(Resource, Default)]
pub struct MarketClicks(pub Vec<MarketAction>);

/// True when the boat is anywhere along Bongao's shore or at the pier.
pub fn near_market(p: Vec2) -> bool {
    p.distance(MARKET_DOCK) < 45.0 || p.distance(LAND_CENTER) < LAND_RADIUS * 0.82
}

#[derive(Resource)]
pub struct Prices {
    pub fish: i32,
    pub urchin: i32,
    pub seaweed: i32,
    pub dried: i32,
    pub wood10: i32,
    pub nipa10: i32,
    pub water5: i32,
    day: u32,
}

impl Prices {
    pub fn catch_value(&self, inv: &Inventory) -> i32 {
        inv.fish as i32 * self.fish
            + inv.urchin as i32 * self.urchin
            + inv.seaweed as i32 * self.seaweed
            + inv.dried_fish as i32 * self.dried
    }
    pub fn repair_cost(hull: f32) -> i32 {
        ((100.0 - hull) * 0.8).ceil().max(0.0) as i32
    }
}

pub struct MarketPlugin;

impl Plugin for MarketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarketUi>()
            .init_resource::<MarketClicks>()
            .insert_resource(Prices {
                fish: 3,
                urchin: 8,
                seaweed: 5,
                dried: 10,
                wood10: 40,
                nipa10: 20,
                water5: 15,
                day: 0,
            })
            .add_systems(Update, (update_prices, market).chain().run_if(in_state(GameState::Playing)));
    }
}

fn update_prices(clock: Res<Clock>, mut prices: ResMut<Prices>, mut rng: ResMut<Rng>) {
    if prices.day == clock.day {
        return;
    }
    prices.day = clock.day;
    let dry = clock.season() == Season::Dry;
    let mut vary = |base: f32| (base * rng.range(0.8, 1.25)).round().max(1.0) as i32;
    prices.fish = vary(3.0);
    prices.urchin = vary(8.0);
    prices.seaweed = vary(5.0);
    prices.dried = vary(10.0);
    prices.wood10 = vary(40.0);
    prices.nipa10 = vary(20.0);
    // Water is scarce and dear in the dry season.
    prices.water5 = vary(if dry { 24.0 } else { 12.0 });
}

#[allow(clippy::too_many_arguments)]
fn market(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<MarketUi>,
    mut boat: ResMut<BoatState>,
    mut inv: ResMut<Inventory>,
    prices: Res<Prices>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut hints: ResMut<Hints>,
    mut build: ResMut<BuildMode>,
    mut clicks: ResMut<MarketClicks>,
    mode: Res<ViewMode>,
) {
    let near = near_market(boat.pos) && *mode == ViewMode::Captain;
    let arrived = near && !ui.was_near;
    ui.was_near = near;
    if !near {
        ui.open = false;
        clicks.0.clear();
        return;
    }
    // Open the stalls automatically when the boat reaches Bongao.
    if arrived {
        ui.open = true;
        build.active = false;
        log.push("You tie up at Bongao. Click a button or press 1-5 to trade.");
    }
    if !ui.open {
        hints.0.push("[G] Open Bongao market".into());
    }
    if keys.just_pressed(KeyCode::KeyG) {
        ui.open = !ui.open;
        build.active = false;
    }
    let mut actions: Vec<MarketAction> = std::mem::take(&mut clicks.0);
    if !ui.open {
        return;
    }
    for (key, action) in [
        (KeyCode::Digit1, MarketAction::Sell),
        (KeyCode::Digit2, MarketAction::Wood),
        (KeyCode::Digit3, MarketAction::Water),
        (KeyCode::Digit4, MarketAction::Repair),
        (KeyCode::Digit5, MarketAction::Upgrade),
        (KeyCode::Escape, MarketAction::Leave),
    ] {
        if keys.just_pressed(key) {
            actions.push(action);
        }
    }
    let capacity = boat.tier.stats().capacity;
    for action in actions {
        match action {
            MarketAction::Leave => ui.open = false,
            MarketAction::Sell => {
                let value = prices.catch_value(&inv);
                if value == 0 {
                    log.push("You have nothing to sell yet. Catch fish (F) or dive for tayum (E), then come back.");
                } else {
                    log.push(format!(
                        "Sold {} fish, {} tayum, {} seaweed, {} dried fish for P{value}.",
                        inv.fish, inv.urchin, inv.seaweed, inv.dried_fish
                    ));
                    inv.pesos += value;
                    inv.fish = 0;
                    inv.urchin = 0;
                    inv.seaweed = 0;
                    inv.dried_fish = 0;
                    sfx.play(Sfx::Knock, 0.4);
                }
            }
            MarketAction::Wood => {
                if inv.pesos >= prices.wood10 {
                    inv.pesos -= prices.wood10;
                    inv.wood += 10;
                    sfx.play(Sfx::Thud, 0.5);
                    log.push(format!("Bought 10 timber for P{}.", prices.wood10));
                } else {
                    log.push("Not enough pesos for timber.");
                }
            }
            MarketAction::Water => {
                if inv.cargo() + 5 > capacity {
                    log.push("No room in the boat for 5 more jars.");
                } else if inv.pesos >= prices.water5 {
                    inv.pesos -= prices.water5;
                    inv.water_jars += 5;
                    sfx.play(Sfx::Thud, 0.4);
                    log.push(format!("Bought 5 jars of fresh water for P{}.", prices.water5));
                } else {
                    log.push("Not enough pesos for water.");
                }
            }
            MarketAction::Repair => {
                let cost = Prices::repair_cost(boat.hull);
                if cost == 0 {
                    log.push("Your hull is sound.");
                } else if inv.pesos >= cost {
                    inv.pesos -= cost;
                    boat.hull = 100.0;
                    sfx.play(Sfx::Knock, 0.8);
                    log.push(format!("The boatwright patches your hull for P{cost}."));
                } else {
                    log.push("Not enough pesos for repairs.");
                }
            }
            MarketAction::Upgrade => match boat.tier.next() {
                None => log.push("You already sail the finest balangay in the Sulu Sea."),
                Some(next) => {
                    let price = next.stats().price;
                    if inv.pesos >= price {
                        inv.pesos -= price;
                        boat.tier = next;
                        boat.hull = 100.0;
                        boat.request_rebuild();
                        sfx.play(Sfx::Knock, 1.0);
                        log.push(format!("You trade up to a {}!", next.stats().name));
                    } else {
                        log.push(format!("A {} costs P{price}.", next.stats().name));
                    }
                }
            },
        }
    }
}
