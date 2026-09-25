//! Sama families. Each family lives on and works from its own boat. The
//! overseer gives a family a task and it carries the task out by itself:
//! sailing to fish, selling at Bongao, buying timber, building blueprints.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::{PI, TAU};

use crate::boat::paddle_pose;
use crate::characters::{spawn_character, CharAssets, CharStyle, Headwear};
use crate::common::*;
use crate::fishing::{FishSchool, UrchinBed};
use crate::market::Prices;
use crate::characters::Toon;
use crate::village::{complete_site, cursor_on_sea, BuildMode, CampWorker, Communities, Construction, Kind, Structure, Walkway, Worksite};
use crate::water::{sea_height, sea_normal, Sea};
use crate::weather::{StormPhase, Weather};
use crate::world::{depth_at, is_land, Props, LAND_CENTER, MARKET_DOCK, SHOALS};

pub const FAMILY_CAPACITY: u32 = 20;
const TIMBER_TRIP: u32 = 30;
const CRUISE: f32 = 7.5;
const PADDLE: f32 = 3.0;

const NAMES: [&str; 24] = [
    "Jalani", "Hadja", "Sarabi", "Kalbi", "Undang", "Pandi", "Masa", "Titing", "Umang", "Bangsa", "Laila",
    "Dayang", "Ambo", "Sali", "Nuraida", "Baddung", "Jamil", "Sitti", "Asbi", "Mudda", "Indah", "Kaha",
    "Tambuli", "Ibbang",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Task {
    Rest,
    /// Assigned to a work area (fishing ground, camp...).
    Worksite(Entity),
    FishFood,
    FishSell,
    Tayum,
    Build,
    Water,
    Trade,
}

pub const TASKS: [Task; 7] = [
    Task::Rest,
    Task::FishFood,
    Task::FishSell,
    Task::Tayum,
    Task::Build,
    Task::Water,
    Task::Trade,
];

impl Task {
    pub fn label(self) -> &'static str {
        match self {
            Task::Rest => "Rest & glean",
            Task::FishFood => "Fish for the village",
            Task::FishSell => "Fish to sell",
            Task::Tayum => "Dive for tayum",
            Task::Build => "Build & repair",
            Task::Water => "Fetch water",
            Task::Trade => "Trade goods",
            Task::Worksite(_) => "Work area",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Task::Rest => "Stay home and gather shellfish nearby (a little food).",
            Task::FishFood => "Catch fish in deep water and bring it home to eat.",
            Task::FishSell => "Catch fish and sell it at Bongao. The pesos go to the village purse.",
            Task::Tayum => "Dive for sea urchins on the reefs and sell them at Bongao.",
            Task::Build => "Buy timber at Bongao, build your blueprints and fix storm damage.",
            Task::Water => "Buy fresh water at Bongao and fill the village jars.",
            Task::Trade => "Carry dried fish and seaweed from the village to market.",
            Task::Worksite(_) => "Working an assigned work area. Pick another task to free them.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Dest {
    Home,
    Market,
    School(Entity),
    Spot(Vec2),
    Bed(Entity),
    Site(Entity),
    Worksite(Entity),
    Away(Vec2),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Work {
    Sell,
    BuyMaterials,
    BuyWater,
    Unload,
    LoadMaterials,
    LoadGoods,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Decide,
    Idle(f32),
    Travel(Dest),
    Fishing { school: Option<Entity>, t: f32 },
    Diving { bed: Entity, t: f32 },
    Working(Work, f32),
    Building { site: Entity, knock: f32 },
    Repairing { target: Entity, t: f32 },
    Harvest { site: Entity, t: f32 },
    Pray,
    Shelter,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Cargo {
    pub fish: u32,
    pub urchin: u32,
    pub timber: u32,
    pub nipa: u32,
    pub water: u32,
    pub dried: u32,
    pub seaweed: u32,
}

impl Cargo {
    pub fn total(&self) -> u32 {
        self.fish + self.urchin + self.timber + self.nipa + self.water + self.dried + self.seaweed
    }
    fn saleable(&self) -> u32 {
        self.fish + self.urchin + self.dried + self.seaweed
    }
    fn room(&self) -> u32 {
        FAMILY_CAPACITY.saturating_sub(self.total())
    }
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (n, label) in [
            (self.fish, "fish"),
            (self.urchin, "tayum"),
            (self.timber, "timber"),
            (self.nipa, "nipa"),
            (self.water, "water jars"),
            (self.dried, "dried fish"),
            (self.seaweed, "seaweed"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {label}"));
            }
        }
        if parts.is_empty() { "empty".into() } else { parts.join(", ") }
    }
}

#[derive(Clone, Copy)]
struct Rig {
    sail_pivot: Entity,
    cloth: Entity,
    diver: Entity,
    paddle: Entity,
}

#[derive(Component)]
pub struct Family {
    pub name: &'static str,
    pub community: usize,
    pub task: Task,
    pub leaving: bool,
    pub cargo: Cargo,
    pub status: String,
    pub pos: Vec2,
    heading: f32,
    speed: f32,
    sail: f32,
    slot: usize,
    step: Step,
    home: Option<Entity>,
    rig: Rig,
    paddle_phase: f32,
}

impl Family {
    pub fn target(&self) -> Option<Vec2> {
        match self.step {
            Step::Travel(Dest::Spot(p)) | Step::Travel(Dest::Away(p)) => Some(p),
            _ => None,
        }
    }
}

#[derive(Component)]
struct FamilyBoat;
#[derive(Component)]
struct FamilyPaddle;

#[derive(Resource, Default)]
pub struct Selected(pub Option<Entity>);

#[derive(Resource, Default)]
struct Migration {
    timers: Vec<f32>,
    next_name: usize,
    next_slot: usize,
}

pub struct FamiliesPlugin;

impl Plugin for FamiliesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selected>()
            .init_resource::<Migration>()
            .add_systems(Startup, spawn_first_families)
            .add_systems(
                Update,
                (select_family, assign_task_keys, assign_homes, staff_worksites, family_brain, migration, show_camp_workers)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, draw_family_gizmos)
            .add_systems(PostUpdate, place_family_boats.before(TransformSystems::Propagate));
    }
}

// ------------------------------------------------------------ spawning ---

fn spawn_family(
    commands: &mut Commands,
    props: &Props,
    chars: &CharAssets,
    rng: &mut Rng,
    name: &'static str,
    community: usize,
    pos: Vec2,
    slot: usize,
    step: Step,
) -> Entity {
    let root = commands
        .spawn((
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Visibility::default(),
            FamilyBoat,
        ))
        .id();
    let part = |commands: &mut Commands, parent: Entity, mesh: &Handle<Mesh>, m: &Handle<StandardMaterial>, t: Transform| {
        commands
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), t, ChildOf(parent)))
            .id()
    };
    let paint = &props.paints[slot % props.paints.len()];
    let paint2 = &props.paints[(slot + 2) % props.paints.len()];
    // Hull, painted gunwale, deck and carved prow.
    part(commands, root, &props.sphere, &props.wood, Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(1.25, 0.95, 6.2)));
    part(commands, root, &props.sphere, paint, Transform::from_xyz(0.0, 0.28, 0.0).with_scale(Vec3::new(1.3, 0.16, 6.25)));
    part(commands, root, &props.cube, &props.wood_dark, Transform::from_xyz(0.0, 0.42, 0.0).with_scale(Vec3::new(0.9, 0.08, 4.8)));
    for (z, tilt) in [(2.9, -0.7), (-2.9, 0.7)] {
        part(commands, root, &props.cube, paint, Transform::from_xyz(0.0, 0.62, z)
            .with_rotation(Quat::from_rotation_x(tilt))
            .with_scale(Vec3::new(0.16, 0.85, 0.28)));
    }
    // Katig outriggers.
    for side in [-1.0, 1.0] {
        part(commands, root, &props.sphere, &props.bamboo, Transform::from_xyz(2.3 * side, 0.1, 0.0).with_scale(Vec3::new(0.3, 0.3, 4.4)));
    }
    for z in [1.3, -1.3] {
        part(commands, root, &props.cyl, &props.bamboo, Transform::from_xyz(0.0, 0.6, z)
            .with_rotation(Quat::from_rotation_z(PI / 2.0))
            .with_scale(Vec3::new(0.1, 4.8, 0.1)));
    }
    // Mast and a striped sail.
    part(commands, root, &props.cyl, &props.wood_dark, Transform::from_xyz(0.0, 2.4, 0.8).with_scale(Vec3::new(0.1, 4.2, 0.1)));
    let sail_pivot = commands
        .spawn((Transform::from_xyz(0.0, 1.0, 0.8), Visibility::default(), ChildOf(root)))
        .id();
    let cloth = commands
        .spawn((Transform::default(), Visibility::default(), ChildOf(sail_pivot)))
        .id();
    for (i, m) in [paint, &props.white, paint2].into_iter().enumerate() {
        let band = 0.95;
        let width = 2.5 * (1.0 - i as f32 * 0.1);
        part(commands, cloth, &props.cube, m, Transform::from_xyz(0.0, band * (i as f32 + 0.5), -width / 2.0 - 0.1)
            .with_scale(Vec3::new(0.05, band * 1.02, width)));
    }
    // The family aboard: a paddler at the stern and a diver at the bow.
    let paddler = spawn_character(commands, chars, CharStyle::random(rng, false));
    commands.entity(paddler).insert((Transform::from_xyz(0.0, 0.42, -1.9), ChildOf(root)));
    let paddle = commands
        .spawn((Transform::default(), Visibility::default(), FamilyPaddle, ChildOf(paddler)))
        .id();
    part(commands, paddle, &props.cyl, &props.wood, Transform::from_xyz(0.0, -1.0, 0.0).with_scale(Vec3::new(0.07, 2.0, 0.07)));
    part(commands, paddle, &props.sphere, &props.wood, Transform::from_xyz(0.0, -2.1, 0.0).with_scale(Vec3::new(0.34, 0.8, 0.07)));
    let mut style = CharStyle::random(rng, false);
    style.headwear = Headwear::Scarf;
    let diver = spawn_character(commands, chars, style);
    commands.entity(diver).insert((Transform::from_xyz(0.0, 0.42, 1.6), ChildOf(root)));
    if rng.chance(0.6) {
        let kid = spawn_character(commands, chars, CharStyle::random(rng, true));
        commands.entity(kid).insert((Transform::from_xyz(0.25, 0.42, 0.0).with_scale(Vec3::splat(0.68)), ChildOf(root)));
    }

    commands.entity(root).insert(Family {
        name,
        community,
        task: Task::Rest,
        leaving: false,
        cargo: Cargo::default(),
        status: "Resting".into(),
        pos,
        heading: rng.range(0.0, TAU),
        speed: 0.0,
        sail: 0.0,
        slot,
        step,
        home: None,
        rig: Rig {
            sail_pivot,
            cloth,
            diver,
            paddle,
        },
        paddle_phase: rng.range(0.0, 1.0),
    });
    root
}

fn spawn_first_families(
    mut commands: Commands,
    props: Res<Props>,
    chars: Res<CharAssets>,
    mut rng: ResMut<Rng>,
    mut migration: ResMut<Migration>,
) {
    let c = SHOALS[0].center;
    for k in 0..2 {
        let pos = c + Vec2::new(10.0 + k as f32 * 9.0, -6.0 + k as f32 * 10.0);
        spawn_family(&mut commands, &props, &chars, &mut rng, NAMES[k], 0, pos, k, Step::Decide);
    }
    migration.next_name = 2;
    migration.next_slot = 2;
}

// ---------------------------------------------------------- selection ---

#[allow(clippy::too_many_arguments)]
fn select_family(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<ViewMode>,
    build: Res<BuildMode>,
    mut selected: ResMut<Selected>,
    mut goto: ResMut<CameraGoto>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui: Query<&Interaction, Or<(With<Button>, With<UiBlocker>)>>,
    families: Query<(Entity, &Family)>,
) {
    if selected.0.is_some_and(|e| families.get(e).is_err()) {
        selected.0 = None;
    }
    if *mode != ViewMode::Overseer || build.active {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        selected.0 = None;
    }
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    if !mouse.just_pressed(MouseButton::Left) || over_ui {
        return;
    }
    let Some(p) = cursor_on_sea(&windows, &cams) else { return };
    let hit = families
        .iter()
        .filter(|(_, f)| f.pos.distance(p) < 9.0)
        .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)));
    selected.0 = hit.map(|(e, _)| e);
    if let Some((_, f)) = hit {
        goto.0 = Some(f.pos);
    }
}

fn assign_task_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<ViewMode>,
    build: Res<BuildMode>,
    selected: Res<Selected>,
    mut families: Query<&mut Family>,
) {
    if *mode != ViewMode::Overseer || build.active || build.category.is_some() {
        return;
    }
    let Some(e) = selected.0 else { return };
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k)
            && let Ok(mut f) = families.get_mut(e)
        {
            set_task(&mut f, TASKS[i]);
        }
    }
}

/// Give a family new work; they drop what they're doing and re-plan.
pub fn set_task(f: &mut Family, task: Task) {
    if f.task != task {
        f.task = task;
        if !matches!(f.step, Step::Building { .. }) || task != Task::Build {
            f.step = Step::Decide;
        }
    }
}

// ------------------------------------------------------------- homes ---

fn assign_homes(
    mut families: Query<&mut Family>,
    houses: Query<(Entity, &Structure), Without<Construction>>,
) {
    let taken: Vec<Entity> = families.iter().filter_map(|f| f.home).collect();
    let mut free: Vec<(Entity, usize)> = houses
        .iter()
        .filter(|(e, s)| s.kind == crate::village::Kind::House && s.working() && !taken.contains(e))
        .map(|(e, s)| (e, s.community))
        .collect();
    for mut f in &mut families {
        if f.home.is_some_and(|h| houses.get(h).is_err()) {
            f.home = None;
        }
        if f.home.is_none()
            && !f.leaving
            && let Some(i) = free.iter().position(|(_, c)| *c == f.community)
        {
            f.home = Some(free.remove(i).0);
        }
    }
}

fn home_anchor(f: &Family, communities: &Communities, houses: &Query<(Entity, &mut Structure), Without<Construction>>) -> Vec2 {
    let center = communities.list[f.community].as_ref().map(|c| c.center).unwrap_or(SHOALS[f.community].center);
    if let Some(h) = f.home
        && let Ok((_, s)) = houses.get(h)
    {
        let out = (s.pos - center).normalize_or(Vec2::X);
        return s.pos + out * 6.5 + Vec2::new(-out.y, out.x) * 1.5;
    }
    // Boat-dwellers moor in a loose ring around the village centre.
    let a = f.slot as f32 * 2.4;
    let r = 14.0 + (f.slot % 3) as f32 * 6.0;
    let mut p = center + Vec2::new(a.cos(), a.sin()) * r;
    for k in 1..8 {
        if !is_land(p) {
            break;
        }
        let a = a + k as f32 * 0.8;
        p = center + Vec2::new(a.cos(), a.sin()) * r;
    }
    p
}

fn market_spot(slot: usize) -> Vec2 {
    MARKET_DOCK + Vec2::new(5.0 + (slot % 3) as f32 * 4.0, ((slot / 3) % 4) as f32 * 5.0 - 7.5)
}

// ------------------------------------------------------------- brain ---

#[derive(SystemParam)]
struct Env<'w> {
    time: Res<'w, Time>,
    weather: Res<'w, Weather>,
    prices: Res<'w, Prices>,
    props: Res<'w, Props>,
    inv: ResMut<'w, Inventory>,
    communities: ResMut<'w, Communities>,
    log: ResMut<'w, GameLog>,
    sfx: ResMut<'w, SfxQueue>,
    rng: ResMut<'w, Rng>,
    prayer: Res<'w, crate::prayer::Prayer>,
}

/// Steer toward `to`, skirting land. Returns true on arrival.
fn steer(f: &mut Family, to: Vec2, dt: f32, arrive: f32) -> bool {
    let d = to - f.pos;
    let dist = d.length();
    if dist < arrive {
        f.speed *= 0.9;
        f.sail = (f.sail - dt).max(0.0);
        return true;
    }
    let want = d.x.atan2(d.y);
    let mut best = want;
    for off in [0.0, 0.5, -0.5, 1.0, -1.0, 1.6, -1.6, 2.3, -2.3] {
        let h = want + off;
        if !is_land(f.pos + heading_vec(h) * 12.0) && !is_land(f.pos + heading_vec(h) * 5.0) {
            best = h;
            break;
        }
    }
    let diff = wrap_angle(best - f.heading);
    f.heading = wrap_angle(f.heading + diff.clamp(-1.4 * dt, 1.4 * dt));
    let cruising = dist > 22.0;
    let goal = if cruising { CRUISE } else { PADDLE };
    f.speed += (goal - f.speed) * (1.0 - (-dt * 0.8).exp());
    f.sail += ((if cruising { 1.0 } else { 0.0 }) - f.sail) * (1.0 - (-dt * 2.0).exp());
    let next = f.pos + heading_vec(f.heading) * f.speed * dt;
    if is_land(next) {
        f.heading += 1.2 * dt;
        f.speed *= 0.5;
    } else {
        f.pos = next;
    }
    false
}

fn slow_down(f: &mut Family, dt: f32) {
    f.speed *= (1.0 - dt * 1.5).max(0.0);
    f.sail = (f.sail - dt * 1.5).max(0.0);
    f.pos += heading_vec(f.heading) * f.speed * dt;
}

#[allow(clippy::too_many_arguments)]
fn family_brain(
    mut commands: Commands,
    mut env: Env,
    camera: Query<&Transform, With<MainCamera>>,
    mut families: Query<(Entity, &mut Family)>,
    mut schools: Query<(Entity, &mut FishSchool)>,
    mut beds: Query<(Entity, &mut UrchinBed)>,
    mut sites: Query<(Entity, &mut Construction, Option<&Structure>, Option<&Walkway>)>,
    mut houses: Query<(Entity, &mut Structure), Without<Construction>>,
    mut worksites: Query<(Entity, &mut Worksite)>,
) {
    let dt = env.time.delta_secs();
    let cam = camera.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let near_cam = |p: Vec2| (1.0 - Vec2::new(cam.x, cam.z).distance(p) / 160.0).clamp(0.0, 1.0);

    for (e, mut f) in &mut families {
        let f = &mut *f;
        let home = home_anchor(f, &env.communities, &houses);
        let at_home = f.pos.distance(home) < 7.0;
        let market = market_spot(f.slot);
        let at_market = f.pos.distance(market) < 9.0;

        // At prayer time everyone downs tools.
        if env.prayer.active.is_some() && !f.leaving && f.step != Step::Pray {
            f.step = Step::Pray;
        }

        match f.step {
            Step::Pray => {
                let Some(name) = env.prayer.active else {
                    f.step = Step::Decide;
                    continue;
                };
                // Pray at the village langgal if it's near, else at home or on the boat.
                let langgal = houses
                    .iter()
                    .filter(|(_, s)| s.kind == Kind::Langgal && s.community == f.community)
                    .map(|(_, s)| s.pos + Vec2::new(0.0, -(s.kind.footprint() + 3.0 + f.slot as f32 % 4.0 * 3.0)))
                    .next();
                let spot = langgal.unwrap_or(home);
                if f.pos.distance(spot) < 150.0 {
                    steer(f, spot, dt, 4.0);
                } else {
                    slow_down(f, dt);
                }
                f.status = if langgal.is_some() {
                    format!("{name} prayer at the langgal")
                } else {
                    format!("{name} prayer")
                };
            }
            Step::Decide => {
                f.step = decide(f, e, home, at_home, at_market, &mut env, &schools, &beds, &sites, &houses, &worksites);
            }
            Step::Harvest { site, t } => {
                slow_down(f, dt);
                let Ok((_, mut ws)) = worksites.get_mut(site) else {
                    f.step = Step::Decide;
                    continue;
                };
                let (period, verb) = match ws.kind {
                    Kind::FishingGround => (4.0, "Fishing"),
                    Kind::DiveSite => (3.5, "Diving for tayum"),
                    Kind::Woodcutter => (5.0, "Cutting timber"),
                    _ => (4.0, "Gathering nipa leaves"),
                };
                f.status = format!("{verb} ({})", f.cargo.describe());
                let t = t + dt;
                if t < period {
                    f.step = Step::Harvest { site, t };
                    continue;
                }
                let room = f.cargo.room();
                let v = near_cam(f.pos);
                match ws.kind {
                    Kind::FishingGround => {
                        let n = (env.rng.int(3, 6) as u32).min(ws.stock as u32).min(room);
                        ws.stock -= n as f32;
                        f.cargo.fish += n;
                        if v > 0.05 {
                            env.sfx.play(Sfx::Splash, v * 0.5);
                        }
                    }
                    Kind::DiveSite => {
                        let n = (env.rng.int(2, 4) as u32).min(ws.stock as u32).min(room);
                        ws.stock -= n as f32;
                        f.cargo.urchin += n;
                        if v > 0.05 {
                            env.sfx.play(Sfx::Bubbles, v * 0.4);
                        }
                    }
                    Kind::Woodcutter => {
                        f.cargo.timber += 3.min(room);
                        if v > 0.05 {
                            env.sfx.play(Sfx::Knock, v * 0.8);
                        }
                    }
                    _ => {
                        f.cargo.nipa += 3.min(room);
                        if v > 0.05 {
                            env.sfx.play(Sfx::Thud, v * 0.3);
                        }
                    }
                }
                let empty = ws.stock < 1.0 && matches!(ws.kind, Kind::FishingGround | Kind::DiveSite);
                f.step = if f.cargo.room() < 3 || empty { Step::Decide } else { Step::Harvest { site, t: 0.0 } };
            }
            Step::Idle(t) => {
                if at_home || f.pos.distance(home) > 40.0 {
                    slow_down(f, dt);
                } else {
                    steer(f, home, dt, 4.0);
                }
                f.step = if t - dt <= 0.0 { Step::Decide } else { Step::Idle(t - dt) };
            }
            Step::Shelter => {
                if !at_home {
                    steer(f, home, dt, 4.0);
                } else {
                    slow_down(f, dt);
                }
                f.status = "Sheltering from the storm at home".into();
                if env.weather.phase == StormPhase::Clear {
                    f.step = Step::Decide;
                }
            }
            Step::Travel(dest) => {
                let (to, arrive) = match dest {
                    Dest::Home => (Some(home), 4.0),
                    Dest::Market => (Some(market), 4.0),
                    Dest::Spot(p) => (Some(p), 5.0),
                    Dest::Away(p) => (Some(p), 8.0),
                    Dest::School(s) => (schools.get(s).ok().map(|(_, s)| s.pos), 14.0),
                    Dest::Bed(b) => (beds.get(b).ok().map(|(_, b)| b.pos + Vec2::new(3.0, 0.0)), 5.0),
                    Dest::Site(s) => (sites.get(s).ok().map(|(_, _, st, w)| site_mooring(st, w)), 5.0),
                    Dest::Worksite(w) => (worksites.get(w).ok().map(|(_, w)| w.mooring), 5.0),
                };
                match to {
                    None => f.step = Step::Decide,
                    Some(to) => {
                        if steer(f, to, dt, arrive) {
                            if let Dest::Away(_) = dest {
                                commands.entity(e).despawn();
                                continue;
                            }
                            f.step = Step::Decide;
                        }
                    }
                }
            }
            Step::Fishing { school, t } => {
                slow_down(f, dt);
                let t = t + dt;
                f.status = format!("Fishing ({}/{} fish)", f.cargo.fish, FAMILY_CAPACITY);
                if t < 4.0 {
                    f.step = Step::Fishing { school, t };
                    continue;
                }
                let room = f.cargo.room();
                let caught = match school.and_then(|s| schools.get_mut(s).ok()) {
                    Some((_, mut s)) if s.pos.distance(f.pos) < 35.0 => {
                        let n = (env.rng.int(3, 6) as u32).min(s.stock as u32).min(room);
                        s.stock -= n as f32;
                        n
                    }
                    Some(_) => {
                        // The school swam off; follow it.
                        f.step = Step::Decide;
                        continue;
                    }
                    None => (env.rng.int(0, 2) as u32).min(room),
                };
                f.cargo.fish += caught;
                let v = near_cam(f.pos) * 0.5;
                if v > 0.05 {
                    env.sfx.play(Sfx::Splash, v);
                }
                let school_empty = school.and_then(|s| schools.get(s).ok()).is_some_and(|(_, s)| s.stock < 1.0);
                f.step = if f.cargo.room() == 0 || school_empty {
                    Step::Decide
                } else {
                    Step::Fishing { school, t: 0.0 }
                };
            }
            Step::Diving { bed, t } => {
                slow_down(f, dt);
                f.status = format!("Diving for tayum ({} so far)", f.cargo.urchin);
                if t + dt < 3.5 {
                    f.step = Step::Diving { bed, t: t + dt };
                    continue;
                }
                if let Ok((_, mut b)) = beds.get_mut(bed) {
                    let n = (env.rng.int(2, 4) as u32).min(b.stock as u32).min(f.cargo.room());
                    b.stock -= n as f32;
                    f.cargo.urchin += n;
                }
                let v = near_cam(f.pos) * 0.5;
                if v > 0.05 {
                    env.sfx.play(Sfx::Splash, v);
                }
                f.step = Step::Decide;
            }
            Step::Working(work, t) => {
                slow_down(f, dt);
                if t - dt > 0.0 {
                    f.step = Step::Working(work, t - dt);
                    continue;
                }
                do_work(f, work, &mut env, &sites);
                f.step = Step::Decide;
            }
            Step::Building { site, knock } => {
                slow_down(f, dt);
                let Ok((_, mut c, st, w)) = sites.get_mut(site) else {
                    f.step = Step::Decide;
                    continue;
                };
                // Hand over any timber and nipa we carry.
                let give = f.cargo.timber.min(c.wood_missing() as u32);
                f.cargo.timber -= give;
                c.wood_have += give as i32;
                let give = f.cargo.nipa.min(c.nipa_missing() as u32);
                f.cargo.nipa -= give;
                c.nipa_have += give as i32;
                let allowed = c.allowed();
                if c.progress >= allowed - 0.001 && c.progress < 1.0 {
                    // Out of timber for now.
                    f.step = Step::Decide;
                    continue;
                }
                c.progress = (c.progress + dt / c.kind.build_time()).min(allowed);
                f.status = format!("Building a {} ({:.0}%)", c.kind.short(), c.progress * 100.0);
                let mut knock = knock - dt;
                if knock <= 0.0 {
                    knock = 1.2;
                    let v = near_cam(f.pos) * 0.7;
                    if v > 0.05 {
                        env.sfx.play(Sfx::Knock, v);
                    }
                }
                if c.progress >= 1.0 {
                    let village = st
                        .map(|s| s.community)
                        .and_then(|i| env.communities.list[i].as_ref().map(|c| c.name))
                        .unwrap_or("the village");
                    env.log.push(format!("{}'s family finished a {} at {village}!", f.name, c.kind.short()));
                    let (props, rng) = (&*env.props, &mut *env.rng);
                    complete_site(&mut commands, props, rng, site, st, w);
                    f.step = Step::Decide;
                } else {
                    f.step = Step::Building { site, knock };
                }
            }
            Step::Repairing { target, t } => {
                slow_down(f, dt);
                let Ok((_, mut s)) = houses.get_mut(target) else {
                    f.step = Step::Decide;
                    continue;
                };
                f.status = format!("Repairing a {}", s.kind.short());
                if t + dt < 5.0 {
                    f.step = Step::Repairing { target, t: t + dt };
                    continue;
                }
                let need = repair_cost(s.health);
                f.cargo.timber -= need.min(f.cargo.timber);
                s.health = 100.0;
                env.sfx.play(Sfx::Knock, near_cam(f.pos) * 0.7);
                f.step = Step::Decide;
            }
        }
    }
}

fn repair_cost(health: f32) -> u32 {
    ((100.0 - health) / 20.0).ceil().max(1.0) as u32
}

fn site_mooring(st: Option<&Structure>, w: Option<&Walkway>) -> Vec2 {
    if let Some(s) = st {
        s.pos + Vec2::new(0.0, -(s.kind.footprint() + 3.0))
    } else if let Some(w) = w {
        let d = (w.b_pos - w.a_pos).normalize_or(Vec2::X);
        w.mid() + Vec2::new(-d.y, d.x) * 3.5
    } else {
        Vec2::ZERO
    }
}

#[allow(clippy::too_many_arguments)]
fn decide(
    f: &mut Family,
    _e: Entity,
    home: Vec2,
    at_home: bool,
    at_market: bool,
    env: &mut Env,
    schools: &Query<(Entity, &mut FishSchool)>,
    beds: &Query<(Entity, &mut UrchinBed)>,
    sites: &Query<(Entity, &mut Construction, Option<&Structure>, Option<&Walkway>)>,
    houses: &Query<(Entity, &mut Structure), Without<Construction>>,
    worksites: &Query<(Entity, &mut Worksite)>,
) -> Step {
    let go_home = |f: &mut Family, work: Work, status: &str| {
        f.status = status.into();
        if at_home { Step::Working(work, 2.0) } else { Step::Travel(Dest::Home) }
    };
    let go_market = |f: &mut Family, work: Work, status: &str| {
        f.status = status.into();
        if at_market { Step::Working(work, 2.5) } else { Step::Travel(Dest::Market) }
    };
    let idle = |f: &mut Family, status: &str, t: f32| {
        f.status = status.into();
        Step::Idle(t)
    };

    if f.leaving {
        return Step::Travel(Dest::Away(f.pos + (f.pos - LAND_CENTER).normalize_or(Vec2::Y) * 400.0));
    }
    if env.weather.phase != StormPhase::Clear {
        if at_home {
            return Step::Shelter;
        }
        f.status = "Racing home ahead of the storm".into();
        return Step::Travel(Dest::Home);
    }
    let c = env.communities.list[f.community].as_ref();
    let (c_water, c_cap, c_goods) = c.map(|c| (c.water, c.water_cap, c.dried + c.seaweed)).unwrap_or((0.0, 0.0, 0.0));

    // Somewhere to fish: the nearest school with fish, else any deep water.
    let fish_step = |f: &mut Family, env: &mut Env| -> Step {
        let school = schools
            .iter()
            .filter(|(_, s)| s.stock >= 3.0 && s.pos.distance(f.pos) < 450.0)
            .min_by(|a, b| a.1.pos.distance(f.pos).total_cmp(&b.1.pos.distance(f.pos)));
        if let Some((se, s)) = school {
            if s.pos.distance(f.pos) < 16.0 {
                f.status = "Casting the net".into();
                return Step::Fishing { school: Some(se), t: 0.0 };
            }
            f.status = "Sailing to a school of fish".into();
            return Step::Travel(Dest::School(se));
        }
        if depth_at(f.pos) > 10.0 {
            return Step::Fishing { school: None, t: 0.0 };
        }
        f.status = "Looking for deep water".into();
        for _ in 0..30 {
            let a = env.rng.range(0.0, TAU);
            let p = home + Vec2::new(a.cos(), a.sin()) * env.rng.range(70.0, 160.0);
            if depth_at(p) > 12.0 {
                return Step::Travel(Dest::Spot(p));
            }
        }
        Step::Idle(5.0)
    };

    match f.task {
        Task::Worksite(site) => {
            let Ok((_, ws)) = worksites.get(site) else {
                f.task = Task::Rest;
                return Step::Decide;
            };
            let at_site = f.pos.distance(ws.mooring) < 8.0;
            let full = f.cargo.room() < 3;
            match ws.kind {
                Kind::FishingGround => {
                    if full || (f.cargo.fish > 0 && ws.stock < 1.0) {
                        let hungry = env.communities.list[f.community]
                            .as_ref()
                            .is_some_and(|c| c.food < c.families as f32 * 10.0 + 10.0);
                        return if hungry {
                            go_home(f, Work::Unload, "Bringing fish home to eat")
                        } else {
                            go_market(f, Work::Sell, "Selling surplus fish at Bongao")
                        };
                    }
                    if ws.stock < 1.0 {
                        return idle(f, "Waiting for the fish to come back", 5.0);
                    }
                }
                Kind::DiveSite => {
                    if full || f.cargo.urchin >= 14 || (f.cargo.urchin > 0 && ws.stock < 1.0) {
                        return go_market(f, Work::Sell, "Taking tayum to Bongao");
                    }
                    if ws.stock < 1.0 {
                        return idle(f, "Waiting for tayum to regrow", 5.0);
                    }
                }
                Kind::Woodcutter => {
                    if full || f.cargo.timber >= 18 {
                        return go_home(f, Work::Unload, "Bringing timber to the village stockpile");
                    }
                }
                _ => {
                    if full || f.cargo.nipa >= 18 {
                        return go_home(f, Work::Unload, "Bringing nipa leaves to the village stockpile");
                    }
                }
            }
            if at_site {
                Step::Harvest { site, t: 0.0 }
            } else {
                f.status = format!("Heading to the {}", ws.kind.short());
                Step::Travel(Dest::Worksite(site))
            }
        }
        Task::Rest => {
            if f.cargo.total() > 0 {
                return go_home(f, Work::Unload, "Bringing cargo home");
            }
            if at_home {
                idle(f, "Resting and gleaning shellfish", env.rng.range(4.0, 8.0))
            } else {
                f.status = "Heading home".into();
                Step::Travel(Dest::Home)
            }
        }
        Task::FishFood => {
            if f.cargo.room() == 0 || (f.cargo.fish > 0 && f.cargo.room() < 3) {
                return go_home(f, Work::Unload, "Bringing the catch home");
            }
            fish_step(f, env)
        }
        Task::FishSell => {
            if f.cargo.room() < 3 && f.cargo.saleable() > 0 {
                return go_market(f, Work::Sell, "Sailing to Bongao to sell the catch");
            }
            fish_step(f, env)
        }
        Task::Tayum => {
            if f.cargo.urchin >= 14 || (f.cargo.room() < 3 && f.cargo.saleable() > 0) {
                return go_market(f, Work::Sell, "Taking tayum to Bongao");
            }
            let bed = beds
                .iter()
                .filter(|(_, b)| b.stock >= 2.0)
                .min_by(|a, b| a.1.pos.distance(f.pos).total_cmp(&b.1.pos.distance(f.pos)));
            match bed {
                Some((be, b)) if b.pos.distance(f.pos) < 9.0 => Step::Diving { bed: be, t: 0.0 },
                Some((be, _)) => {
                    f.status = "Sailing to a tayum bed".into();
                    Step::Travel(Dest::Bed(be))
                }
                None if f.cargo.urchin > 0 => go_market(f, Work::Sell, "Taking tayum to Bongao"),
                None => idle(f, "The reefs are picked clean - waiting for tayum to regrow", 8.0),
            }
        }
        Task::Build => {
            // Nearest blueprint (own village first), else storm damage to fix.
            let site = sites
                .iter()
                .filter(|(_, c, _, _)| c.progress < 1.0)
                .min_by(|a, b| {
                    let da = site_mooring(a.2, a.3).distance(f.pos) + if site_village(a.2, a.3) == Some(f.community) { 0.0 } else { 500.0 };
                    let db = site_mooring(b.2, b.3).distance(f.pos) + if site_village(b.2, b.3) == Some(f.community) { 0.0 } else { 500.0 };
                    da.total_cmp(&db)
                });
            let all_missing: u32 = sites.iter().map(|(_, c, _, _)| c.wood_missing() as u32).sum();
            let _ = all_missing;
            if let Some((se, c, st, w)) = site {
                let spot = site_mooring(st, w);
                let at_site = f.pos.distance(spot) < 7.0;
                let carrying = (f.cargo.timber > 0 && c.wood_missing() > 0) || (f.cargo.nipa > 0 && c.nipa_missing() > 0);
                if c.allowed() > c.progress + 0.001 || carrying {
                    f.status = format!("Going to build a {}", c.kind.short());
                    return if at_site { Step::Building { site: se, knock: 0.0 } } else { Step::Travel(Dest::Site(se)) };
                }
                let need_w = c.wood_missing() > 0;
                let need_n = c.nipa_missing() > 0;
                if (need_w && env.inv.wood > 0) || (need_n && env.inv.nipa > 0) {
                    return go_home(f, Work::LoadMaterials, "Fetching materials from the village stockpile");
                }
                let unit_w = env.prices.wood10 / 10;
                let unit_n = env.prices.nipa10 / 10;
                if (need_w && env.inv.pesos >= unit_w) || (need_n && env.inv.pesos >= unit_n) {
                    return go_market(f, Work::BuyMaterials, "Sailing to Bongao to buy timber and nipa");
                }
                return idle(f, "Waiting for materials - place a Woodcutter or Nipa camp, or earn pesos", 6.0);
            }
            let damaged = houses
                .iter()
                .filter(|(_, s)| s.health < 99.0)
                .min_by(|a, b| a.1.pos.distance(f.pos).total_cmp(&b.1.pos.distance(f.pos)));
            if let Some((he, s)) = damaged {
                let need = repair_cost(s.health);
                if f.cargo.timber >= need {
                    let spot = s.pos + Vec2::new(0.0, -(s.kind.footprint() + 3.0));
                    if f.pos.distance(spot) < 7.0 {
                        return Step::Repairing { target: he, t: 0.0 };
                    }
                    f.status = format!("Going to repair a {}", s.kind.short());
                    return Step::Travel(Dest::Spot(spot));
                }
                if env.inv.wood > 0 {
                    return go_home(f, Work::LoadMaterials, "Fetching timber for repairs");
                }
                if env.inv.pesos >= env.prices.wood10 / 10 {
                    return go_market(f, Work::BuyMaterials, "Buying timber for repairs");
                }
                return idle(f, "Waiting for pesos to buy timber for repairs", 6.0);
            }
            if f.cargo.total() > 0 {
                return go_home(f, Work::Unload, "Storing leftover timber");
            }
            if at_home {
                idle(f, "No blueprints - press B to plan a building", 5.0)
            } else {
                f.status = "Heading home".into();
                Step::Travel(Dest::Home)
            }
        }
        Task::Water => {
            if f.cargo.water > 0 {
                return go_home(f, Work::Unload, "Carrying fresh water home");
            }
            if c_water < c_cap - 5.0 {
                if env.inv.pesos >= env.prices.water5 {
                    return go_market(f, Work::BuyWater, "Sailing to Bongao for fresh water");
                }
                return idle(f, "No pesos to buy water", 6.0);
            }
            if f.cargo.total() > 0 {
                return go_home(f, Work::Unload, "Bringing cargo home");
            }
            idle(f, "The water jars are full", 6.0)
        }
        Task::Trade => {
            if f.cargo.saleable() > 0 {
                return go_market(f, Work::Sell, "Taking goods to Bongao");
            }
            if c_goods >= 3.0 {
                return go_home(f, Work::LoadGoods, "Loading dried fish and seaweed");
            }
            if at_home {
                idle(f, "Waiting for goods (build drying racks and seaweed farms)", 6.0)
            } else {
                f.status = "Heading home".into();
                Step::Travel(Dest::Home)
            }
        }
    }
}

fn site_village(st: Option<&Structure>, w: Option<&Walkway>) -> Option<usize> {
    st.map(|s| s.community).or(w.map(|w| w.community))
}

fn do_work(
    f: &mut Family,
    work: Work,
    env: &mut Env,
    sites: &Query<(Entity, &mut Construction, Option<&Structure>, Option<&Walkway>)>,
) {
    match work {
        Work::Sell => {
            let p = &env.prices;
            let value = f.cargo.fish as i32 * p.fish
                + f.cargo.urchin as i32 * p.urchin
                + f.cargo.dried as i32 * p.dried
                + f.cargo.seaweed as i32 * p.seaweed;
            if value > 0 {
                env.log.push(format!("{}'s family sold {} at Bongao for P{value}.", f.name, Cargo { timber: 0, water: 0, ..f.cargo }.describe()));
                env.inv.pesos += value;
            }
            f.cargo.fish = 0;
            f.cargo.urchin = 0;
            f.cargo.dried = 0;
            f.cargo.seaweed = 0;
        }
        Work::BuyMaterials => {
            let missing_w: u32 = sites.iter().map(|(_, c, _, _)| c.wood_missing() as u32).sum();
            let missing_n: u32 = sites.iter().map(|(_, c, _, _)| c.nipa_missing() as u32).sum();
            let mut want_w = missing_w.saturating_sub(env.inv.wood.max(0) as u32 + f.cargo.timber);
            let want_n = missing_n.saturating_sub(env.inv.nipa.max(0) as u32 + f.cargo.nipa).min(f.cargo.room());
            if want_w == 0 && want_n == 0 {
                want_w = 6; // repairs
            }
            let unit_n = env.prices.nipa10 as f32 / 10.0;
            let n_nipa = want_n.min((env.inv.pesos as f32 / unit_n).floor() as u32);
            let cost_n = (n_nipa as f32 * unit_n).ceil() as i32;
            env.inv.pesos -= cost_n;
            f.cargo.nipa += n_nipa;
            let unit_w = env.prices.wood10 as f32 / 10.0;
            let n_wood = want_w.min(TIMBER_TRIP).min(f.cargo.room()).min((env.inv.pesos as f32 / unit_w).floor() as u32);
            let cost_w = (n_wood as f32 * unit_w).ceil() as i32;
            env.inv.pesos -= cost_w;
            f.cargo.timber += n_wood;
            if n_wood + n_nipa > 0 {
                env.log.push(format!(
                    "{}'s family bought {n_wood} timber and {n_nipa} nipa at Bongao for P{}.",
                    f.name,
                    cost_w + cost_n
                ));
            }
        }
        Work::BuyWater => {
            let c_room = env.communities.list[f.community]
                .as_ref()
                .map(|c| (c.water_cap - c.water).max(0.0) as u32)
                .unwrap_or(0);
            let unit = env.prices.water5 as f32 / 5.0;
            let n = c_room.min(f.cargo.room()).min((env.inv.pesos as f32 / unit).floor() as u32);
            if n > 0 {
                let cost = (n as f32 * unit).ceil() as i32;
                env.inv.pesos -= cost;
                f.cargo.water += n;
                env.log.push(format!("{}'s family bought {n} jars of water for P{cost}.", f.name));
            }
        }
        Work::Unload => {
            if let Some(c) = env.communities.list[f.community].as_mut() {
                c.food += (f.cargo.fish + f.cargo.urchin) as f32;
                c.water = (c.water + f.cargo.water as f32).min(c.water_cap.max(c.water));
                c.dried += f.cargo.dried as f32;
                c.seaweed += f.cargo.seaweed as f32;
            }
            env.inv.wood += f.cargo.timber as i32;
            env.inv.nipa += f.cargo.nipa as i32;
            f.cargo = Cargo::default();
        }
        Work::LoadMaterials => {
            let missing_n: u32 = sites.iter().map(|(_, c, _, _)| c.nipa_missing() as u32).sum();
            let n = (env.inv.nipa.max(0) as u32).min(missing_n.max(0)).min(f.cargo.room());
            env.inv.nipa -= n as i32;
            f.cargo.nipa += n;
            let w = (env.inv.wood.max(0) as u32).min(TIMBER_TRIP).min(f.cargo.room());
            env.inv.wood -= w as i32;
            f.cargo.timber += w;
        }
        Work::LoadGoods => {
            if let Some(c) = env.communities.list[f.community].as_mut() {
                let d = (c.dried.floor() as u32).min(f.cargo.room());
                c.dried -= d as f32;
                f.cargo.dried += d;
                let w = (c.seaweed.floor() as u32).min(f.cargo.room());
                c.seaweed -= w as f32;
                f.cargo.seaweed += w;
            }
        }
    }
}

// --------------------------------------------------------- migration ---

#[allow(clippy::too_many_arguments)]
fn migration(
    mut commands: Commands,
    time: Res<Time>,
    props: Res<Props>,
    chars: Res<CharAssets>,
    mut rng: ResMut<Rng>,
    mut state: ResMut<Migration>,
    mut communities: ResMut<Communities>,
    mut log: ResMut<GameLog>,
    mut families: Query<&mut Family>,
) {
    let dt = time.delta_secs();
    if state.timers.len() < communities.list.len() {
        state.timers.resize(communities.list.len(), 0.0);
    }
    for i in 0..communities.list.len() {
        let Some(c) = communities.list[i].as_mut() else { continue };
        state.timers[i] += dt;
        if state.timers[i] < 20.0 {
            continue;
        }
        state.timers[i] = 0.0;
        // Only truly miserable villages lose people, and never their last family.
        if c.approval < 20.0 && c.families > 1 {
            if let Some(mut f) = families.iter_mut().find(|f| f.community == i && !f.leaving) {
                f.leaving = true;
                f.step = Step::Decide;
                log.push(format!("{}'s family sails away from {} - they were {}.", f.name, c.name, c.mood()));
            }
        } else if c.approval >= 50.0 && c.families < c.capacity {
            let name = NAMES[state.next_name % NAMES.len()];
            state.next_name += 1;
            let slot = state.next_slot;
            state.next_slot += 1;
            // They come in from the open sea, away from Bongao.
            let away = (c.center - LAND_CENTER).normalize_or(Vec2::X);
            let a = away.to_angle() + rng.range(-0.8, 0.8);
            let from = (c.center + Vec2::from_angle(a) * 240.0).clamp(Vec2::splat(-580.0), Vec2::splat(580.0));
            spawn_family(&mut commands, &props, &chars, &mut rng, name, i, from, slot, Step::Travel(Dest::Home));
            c.approval -= 3.0;
            log.push(format!("{name}'s family sails in to settle at {}. Give them work!", c.name));
        }
    }
}

// ---------------------------------------------------------- visuals ---

#[allow(clippy::too_many_arguments)]
fn place_family_boats(
    time: Res<Time>,
    sea: Res<Sea>,
    weather: Res<Weather>,
    mut families: Query<(&mut Family, &mut Transform), With<FamilyBoat>>,
    mut parts: Query<&mut Transform, Without<FamilyBoat>>,
    mut vis: Query<&mut Visibility>,
) {
    let dt = time.delta_secs();
    for (mut f, mut t) in &mut families {
        let h = sea_height(&sea, f.pos);
        let n = sea_normal(&sea, f.pos).lerp(Vec3::Y, 0.35).normalize();
        t.translation = Vec3::new(f.pos.x, h - 0.05, f.pos.y);
        t.rotation = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(f.heading);
        let rel = wrap_angle(weather.wind_dir - f.heading);
        if let Ok(mut p) = parts.get_mut(f.rig.sail_pivot) {
            p.rotation = Quat::from_rotation_y((-rel * 0.5).clamp(-1.35, 1.35));
        }
        if let Ok(mut c) = parts.get_mut(f.rig.cloth) {
            c.scale = Vec3::new(1.0, 0.06 + 0.94 * f.sail, 1.0);
        }
        // Paddle when moving slowly under oar power; rest it otherwise.
        let paddling = f.speed > 0.4 && f.sail < 0.5;
        if paddling {
            f.paddle_phase += dt * 1.4;
        }
        let side = if (f.paddle_phase as i32) % 2 == 0 { 1.0 } else { -1.0 };
        if let Ok(mut p) = parts.get_mut(f.rig.paddle) {
            let active = if paddling { 1.0 } else { 0.0 };
            *p = paddle_pose(f.paddle_phase.fract(), side, false, active);
        }
        if let Ok(mut v) = vis.get_mut(f.rig.diver) {
            *v = if matches!(f.step, Step::Diving { .. }) { Visibility::Hidden } else { Visibility::Inherited };
        }
    }
}

fn draw_family_gizmos(
    time: Res<Time>,
    sea: Res<Sea>,
    selected: Res<Selected>,
    families: Query<(Entity, &Family)>,
    schools: Query<&FishSchool>,
    mut gizmos: Gizmos,
) {
    let flat = Quat::from_rotation_x(PI / 2.0);
    let t = time.elapsed_secs();
    for (e, f) in &families {
        let h = sea_height(&sea, f.pos) + 0.1;
        let at = Vec3::new(f.pos.x, h, f.pos.y);
        if selected.0 == Some(e) {
            let pulse = 5.5 + (t * 3.0).sin() * 0.4;
            gizmos.circle(Isometry3d::new(at, flat), pulse, Color::srgb(1.0, 0.9, 0.3));
            let dest = match f.step {
                Step::Travel(Dest::School(s)) => {
                    schools.get(s).ok().map(|s| s.pos)
                }
                _ => f.target(),
            };
            if let Some(d) = dest {
                gizmos.line(at, Vec3::new(d.x, 0.3, d.y), Color::srgba(1.0, 0.9, 0.3, 0.5));
            }
        }
        match f.step {
            Step::Fishing { t: ft, .. } => {
                let side = heading_vec(f.heading + PI / 2.0) * 6.0;
                let c = f.pos + side;
                let r = 1.5 + (ft / 4.0).min(0.35) * 10.0;
                let ch = sea_height(&sea, c) + 0.05;
                gizmos.circle(Isometry3d::new(Vec3::new(c.x, ch, c.y), flat), r, Color::srgba(0.95, 0.95, 0.85, 0.7));
            }
            Step::Diving { .. } => {
                for k in 0..5 {
                    let ph = (t * 1.3 + k as f32 * 0.37) % 1.0;
                    let p = f.pos + heading_vec(f.heading) * 3.0 + Vec2::new((k as f32 * 2.1).sin(), (k as f32 * 1.3).cos()) * 0.8;
                    let bh = sea_height(&sea, p) + 0.05;
                    gizmos.circle(Isometry3d::new(Vec3::new(p.x, bh, p.y), flat), 0.2 + ph * 1.1, Color::srgba(1.0, 1.0, 1.0, 0.7 * (1.0 - ph)));
                }
            }
            _ => {}
        }
    }
}

/// Idle families (on 'Rest') are automatically assigned to unstaffed work areas.
fn staff_worksites(
    mut worksites: Query<(Entity, &mut Worksite)>,
    mut families: Query<(Entity, &mut Family)>,
    mut log: ResMut<GameLog>,
) {
    for (site, mut ws) in &mut worksites {
        if let Some(w) = ws.worker
            && families.get(w).map_or(true, |(_, f)| f.task != Task::Worksite(site) || f.leaving)
        {
            ws.worker = None;
        }
        if ws.worker.is_some() {
            continue;
        }
        let pick = families
            .iter()
            .filter(|(_, f)| f.task == Task::Rest && !f.leaving)
            .min_by(|a, b| {
                // Prefer families from the work area's own village.
                let cost = |f: &Family| f.pos.distance(ws.pos) + if f.community == ws.community { 0.0 } else { 1000.0 };
                cost(a.1).total_cmp(&cost(b.1))
            })
            .map(|(e, _)| e);
        if let Some(e) = pick
            && let Ok((_, mut f)) = families.get_mut(e)
        {
            set_task(&mut f, Task::Worksite(site));
            ws.worker = Some(e);
            log.push(format!("{}'s family now works the {}.", f.name, ws.kind.short()));
        }
    }
    // Families whose work area was removed go back to resting.
    for (_, mut f) in &mut families {
        if let Task::Worksite(site) = f.task
            && worksites.get(site).is_err()
        {
            set_task(&mut f, Task::Rest);
        }
    }
}

/// Show the camp worker toon while a family is ashore cutting or gathering.
fn show_camp_workers(families: Query<&Family>, mut workers: Query<(&CampWorker, &mut Visibility, &mut Toon)>) {
    for (w, mut vis, mut toon) in &mut workers {
        let busy = families.iter().any(|f| matches!(f.step, Step::Harvest { site, .. } if site == w.0));
        *vis = if busy { Visibility::Inherited } else { Visibility::Hidden };
        toon.moving = busy;
    }
}
