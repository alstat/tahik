//! Stilt villages: building, walkways, the community simulation, storm damage
//! and the villagers who live there.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::{FRAC_PI_4, PI, TAU};

use crate::boat::BoatState;
use crate::characters::{spawn_character, CharAssets, CharStyle, Toon};
use crate::common::*;
use crate::families::{Family, Task};
use crate::fishing::Hints;
use crate::water::{sea_height, Sea};
use crate::weather::Weather;
use crate::world::{depth_at, seabed_height, shoal_at, spawn_canopy_tree, spawn_palm, Props, LAND_CENTER, LAND_RADIUS, SHOALS};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    House,
    Walkway,
    RainCatcher,
    FishPen,
    SeaweedFarm,
    DryingRack,
    Langgal,
    // Work areas: families are assigned and work them automatically.
    FishingGround,
    DiveSite,
    Woodcutter,
    NipaGrove,
}

pub const BUILD_KINDS: [Kind; 11] = [
    Kind::House,
    Kind::Walkway,
    Kind::RainCatcher,
    Kind::FishPen,
    Kind::SeaweedFarm,
    Kind::DryingRack,
    Kind::Langgal,
    Kind::FishingGround,
    Kind::DiveSite,
    Kind::Woodcutter,
    Kind::NipaGrove,
];

/// Build-bar categories: (name, indices into BUILD_KINDS).
pub const CATEGORIES: [(&str, &[usize]); 4] = [
    ("Homes", &[0, 1, 6]),
    ("Food", &[7, 8, 3, 4, 5]),
    ("Materials", &[9, 10]),
    ("Water", &[2]),
];

#[derive(Clone, Copy, Debug, Default)]
pub struct Cost {
    pub timber: i32,
    pub nipa: i32,
    pub pesos: i32,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::House => "Stilt house (luma')",
            Kind::Walkway => "Walkway (taytayan)",
            Kind::RainCatcher => "Rain catcher",
            Kind::FishPen => "Fish pen",
            Kind::SeaweedFarm => "Seaweed farm",
            Kind::DryingRack => "Fish drying rack",
            Kind::Langgal => "Langgal (prayer hall)",
            Kind::FishingGround => "Fishing ground",
            Kind::DiveSite => "Tayum dive site",
            Kind::Woodcutter => "Woodcutter camp",
            Kind::NipaGrove => "Nipa gatherers",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Kind::House => "house",
            Kind::Walkway => "walkway",
            Kind::RainCatcher => "rain catcher",
            Kind::FishPen => "fish pen",
            Kind::SeaweedFarm => "seaweed farm",
            Kind::DryingRack => "drying rack",
            Kind::Langgal => "langgal",
            Kind::FishingGround => "fishing ground",
            Kind::DiveSite => "dive site",
            Kind::Woodcutter => "woodcutter camp",
            Kind::NipaGrove => "nipa gatherers",
        }
    }
    /// Walkways cost timber by length (see `walkway_cost`).
    pub fn cost(self) -> Cost {
        let (timber, nipa, pesos) = match self {
            Kind::House => (12, 8, 10),
            Kind::Walkway => (0, 0, 0),
            Kind::RainCatcher => (6, 0, 10),
            Kind::FishPen => (8, 0, 20),
            Kind::SeaweedFarm => (4, 0, 15),
            Kind::DryingRack => (5, 3, 5),
            Kind::Langgal => (25, 15, 40),
            Kind::FishingGround => (0, 0, 0),
            Kind::DiveSite => (0, 0, 0),
            Kind::Woodcutter => (0, 0, 20),
            Kind::NipaGrove => (0, 0, 15),
        };
        Cost { timber, nipa, pesos }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Kind::House => "Home for one family: timber frame, nipa roof. The first one founds a village.",
            Kind::Walkway => "Click two platforms to link them (1 timber / 4 m). Linked homes are happier and sturdier in storms.",
            Kind::RainCatcher => "Collects rain into clay jars (+30 jar storage).",
            Kind::FishPen => "Farms fish for the village - even when you are away.",
            Kind::SeaweedFarm => "Grows agal-agal seaweed to sell. Needs very shallow water.",
            Kind::DryingRack => "Turns surplus fish into dried fish, which sells well.",
            Kind::Langgal => "A prayer hall: big approval boost. One per village.",
            Kind::FishingGround => "Mark deep water as a fishing ground. A family fishes it: food first, surplus sold at Bongao.",
            Kind::DiveSite => "Mark a reef flat for diving. A family gathers tayum there and sells it at Bongao.",
            Kind::Woodcutter => "A camp on Bongao's shore. A family cuts timber for the village stockpile.",
            Kind::NipaGrove => "A camp among Bongao's palms. A family gathers coconut and nipa leaves for roofs.",
        }
    }
    /// Work areas are placed instantly and staffed by a family.
    pub fn is_worksite(self) -> bool {
        matches!(self, Kind::FishingGround | Kind::DiveSite | Kind::Woodcutter | Kind::NipaGrove)
    }
    /// Seconds of work for one family to build it.
    pub fn build_time(self) -> f32 {
        match self {
            Kind::House => 25.0,
            Kind::Walkway => 10.0,
            Kind::RainCatcher => 12.0,
            Kind::FishPen => 18.0,
            Kind::SeaweedFarm => 12.0,
            Kind::DryingRack => 10.0,
            Kind::Langgal => 45.0,
            _ => 0.0,
        }
    }
    pub fn footprint(self) -> f32 {
        self.radius()
    }
    fn radius(self) -> f32 {
        match self {
            Kind::House => 4.2,
            Kind::Walkway => 0.8,
            Kind::RainCatcher => 2.6,
            Kind::FishPen => 5.0,
            Kind::SeaweedFarm => 7.0,
            Kind::DryingRack => 2.8,
            Kind::Langgal => 5.8,
            Kind::FishingGround => 18.0,
            Kind::DiveSite => 6.0,
            Kind::Woodcutter => 6.0,
            Kind::NipaGrove => 6.0,
        }
    }
    /// Structures with a deck that walkways and villagers can use.
    pub fn is_node(self) -> bool {
        matches!(self, Kind::House | Kind::RainCatcher | Kind::DryingRack | Kind::Langgal)
    }
    fn depth_range(self) -> (f32, f32) {
        match self {
            Kind::SeaweedFarm => (0.5, 4.0),
            Kind::FishPen => (1.2, 9.0),
            _ => (0.4, 8.0),
        }
    }
}

/// A work area that one family is assigned to.
#[derive(Component)]
pub struct Worksite {
    pub kind: Kind,
    pub pos: Vec2,
    /// Where the family's boat ties up to work.
    pub mooring: Vec2,
    pub community: usize,
    pub worker: Option<Entity>,
    /// Fish or tayum left to gather (regrows).
    pub stock: f32,
}

impl Worksite {
    pub fn max_stock(&self) -> f32 {
        match self.kind {
            Kind::FishingGround => 40.0,
            Kind::DiveSite => 16.0,
            _ => f32::INFINITY,
        }
    }
}

/// The little worker toon at a land camp, shown while someone works there.
#[derive(Component)]
pub struct CampWorker(pub Entity);

#[derive(Component)]
pub struct Structure {
    pub kind: Kind,
    pub community: usize,
    pub health: f32,
    pub pos: Vec2,
    tilt_axis: Vec3,
}

impl Structure {
    pub fn working(&self) -> bool {
        self.health >= 50.0
    }
}

#[derive(Component)]
pub struct Walkway {
    pub a: Entity,
    pub b: Entity,
    pub a_pos: Vec2,
    pub b_pos: Vec2,
    pub community: usize,
}

impl Walkway {
    pub fn mid(&self) -> Vec2 {
        (self.a_pos + self.b_pos) / 2.0
    }
}

/// A blueprint waiting for a Build family to bring timber and build it.
#[derive(Component)]
pub struct Construction {
    pub kind: Kind,
    pub wood_needed: i32,
    pub wood_have: i32,
    pub nipa_needed: i32,
    pub nipa_have: i32,
    /// 0..1
    pub progress: f32,
}

impl Construction {
    pub fn new(kind: Kind, wood: i32, nipa: i32) -> Self {
        Self {
            kind,
            wood_needed: wood,
            wood_have: 0,
            nipa_needed: nipa,
            nipa_have: 0,
            progress: 0.0,
        }
    }
    pub fn wood_missing(&self) -> i32 {
        (self.wood_needed - self.wood_have).max(0)
    }
    pub fn nipa_missing(&self) -> i32 {
        (self.nipa_needed - self.nipa_have).max(0)
    }
    /// How far building can get with the materials delivered so far.
    pub fn allowed(&self) -> f32 {
        let w = if self.wood_needed > 0 { self.wood_have as f32 / self.wood_needed as f32 } else { 1.0 };
        let n = if self.nipa_needed > 0 { self.nipa_have as f32 / self.nipa_needed as f32 } else { 1.0 };
        w.min(n)
    }
}

#[derive(Component)]
struct Floating;

#[derive(Component)]
struct Ghost;

pub struct Community {
    pub name: &'static str,
    pub center: Vec2,
    pub radius: f32,
    pub families: u32,
    pub food: f32,
    pub water: f32,
    pub dried: f32,
    pub seaweed: f32,
    pub approval: f32,
    pub approval_target: f32,
    anchor: Entity,
    warned_food: bool,
    warned_water: bool,
    // Derived each frame from the structures and families.
    pub idle_families: u32,
    pub capacity: u32,
    pub houses: u32,
    pub connected_houses: u32,
    pub damaged: u32,
    pub catchers: u32,
    pub pens: u32,
    pub farms: u32,
    pub racks: u32,
    pub hall: bool,
    pub water_cap: f32,
}

impl Community {
    fn new(shoal: usize, anchor: Entity) -> Self {
        let s = &SHOALS[shoal];
        Community {
            name: s.name,
            center: s.center,
            radius: s.radius,
            families: 0,
            food: 30.0,
            water: 24.0,
            dried: 0.0,
            seaweed: 0.0,
            approval: 60.0,
            approval_target: 60.0,
            anchor,
            warned_food: false,
            warned_water: false,
            idle_families: 0,
            capacity: 0,
            houses: 0,
            connected_houses: 0,
            damaged: 0,
            catchers: 0,
            pens: 0,
            farms: 0,
            racks: 0,
            hall: false,
            water_cap: 12.0,
        }
    }
    pub fn thriving(&self) -> bool {
        self.families >= 3 && self.approval >= 50.0
    }
    pub fn mood(&self) -> &'static str {
        if self.food <= 0.0 {
            "hungry"
        } else if self.water <= 0.0 {
            "thirsty"
        } else if self.families > self.capacity {
            "living on boats"
        } else if self.approval >= 70.0 {
            "content"
        } else if self.approval >= 45.0 {
            "steady"
        } else {
            "unhappy"
        }
    }
}

#[derive(Resource, Default)]
pub struct Communities {
    pub list: Vec<Option<Community>>,
}

impl Communities {
    pub fn nearest_center(&self, p: Vec2) -> Option<Vec2> {
        self.list
            .iter()
            .flatten()
            .map(|c| c.center)
            .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
    }
    pub fn at(&self, p: Vec2) -> Option<usize> {
        self.list.iter().enumerate().find_map(|(i, c)| match c {
            Some(c) if c.center.distance(p) < c.radius * 1.15 => Some(i),
            _ => None,
        })
    }
    pub fn thriving(&self) -> usize {
        self.list.iter().flatten().filter(|c| c.thriving()).count()
    }
    pub fn founded(&self) -> usize {
        self.list.iter().flatten().count()
    }
    pub fn people(&self) -> u32 {
        self.list.iter().flatten().map(|c| c.families * 5).sum()
    }
}

#[derive(Resource, Default)]
pub struct BuildMode {
    pub active: bool,
    pub selected: usize,
    pub cursor: Option<Vec2>,
    pub valid: bool,
    pub reason: String,
    pub walk_start: Option<Entity>,
    /// Open build-bar category (cards shown).
    pub category: Option<usize>,
    ghost: Option<(Entity, usize, Vec2)>,
    ghost_mat: Option<Handle<StandardMaterial>>,
    blueprint_mat: Option<Handle<StandardMaterial>>,
}

impl BuildMode {
    pub fn kind(&self) -> Kind {
        BUILD_KINDS[self.selected]
    }
}

#[derive(Resource, Default)]
struct SimClock(f32);

#[derive(Component)]
struct Villager {
    community: usize,
    node: Entity,
    target: Entity,
    from: Vec3,
    to: Vec3,
    t: f32,
    dur: f32,
    idle: f32,
}

pub struct VillagePlugin;

impl Plugin for VillagePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Communities {
            list: (0..SHOALS.len()).map(|_| None).collect(),
        })
        .init_resource::<BuildMode>()
        .init_resource::<SimClock>()
        .add_systems(Startup, (setup_ghost_material, found_home_village))
        .add_systems(
            Update,
            (
                build_input,
                build_cursor,
                build_preview,
                place_structure,
                regrow_worksites,
                update_stats,
                simulate,
                village_interact,
                sync_villagers,
                walk_villagers,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (show_damage, float_farms))
        .add_systems(OnEnter(GameState::Playing), demo_village);
    }
}

/// The two starting families anchor their boats at Sitangkai.
fn found_home_village(mut communities: ResMut<Communities>) {
    communities.list[0] = Some(Community::new(0, Entity::PLACEHOLDER));
}

fn setup_ghost_material(mut build: ResMut<BuildMode>, mut materials: ResMut<Assets<StandardMaterial>>) {
    build.blueprint_mat = Some(materials.add(StandardMaterial {
        base_color: Color::srgba(0.6, 0.85, 1.0, 0.38),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }));
    build.ghost_mat = Some(materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 1.0, 0.5, 0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }));
}

// ------------------------------------------------------------ models ---

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    props: &'a Props,
    root: Entity,
    origin: Vec2,
    ghost: Option<Handle<StandardMaterial>>,
}

impl Builder<'_, '_, '_> {
    fn mat(&self, m: &Handle<StandardMaterial>) -> Handle<StandardMaterial> {
        self.ghost.clone().unwrap_or_else(|| m.clone())
    }
    fn part(&mut self, mesh: &Handle<Mesh>, m: &Handle<StandardMaterial>, t: Transform) -> Entity {
        let m = self.mat(m);
        self.commands
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), t, ChildOf(self.root)))
            .id()
    }
    fn cube(&mut self, m: &Handle<StandardMaterial>, pos: Vec3, size: Vec3) -> Entity {
        let mesh = self.props.cube.clone();
        self.part(&mesh, m, Transform::from_translation(pos).with_scale(size))
    }
    /// A post from the sea floor up to `top`, at a local offset.
    fn stilt(&mut self, local: Vec2, top: f32, r: f32) {
        let w = self.origin + local;
        let bottom = seabed_height(w.x, w.y).min(top - 0.5) - 0.3;
        let h = top - bottom;
        let (mesh, m) = (self.props.cyl.clone(), self.props.wood_dark.clone());
        self.part(
            &mesh,
            &m,
            Transform::from_xyz(local.x, bottom + h / 2.0, local.y).with_scale(Vec3::new(r, h, r)),
        );
    }
    fn deck(&mut self, w: f32, d: f32) {
        let m = self.props.wood.clone();
        self.cube(&m, Vec3::new(0.0, DECK_Y, 0.0), Vec3::new(w, 0.3, d));
        let (hw, hd) = (w / 2.0 - 0.3, d / 2.0 - 0.3);
        for (x, z) in [(-hw, -hd), (hw, -hd), (-hw, hd), (hw, hd), (0.0, -hd), (0.0, hd), (-hw, 0.0), (hw, 0.0)] {
            self.stilt(Vec2::new(x, z), DECK_Y, 0.22);
        }
    }
    fn roof(&mut self, y: f32, size: Vec3) {
        let (mesh, m) = (self.props.cone4.clone(), self.props.thatch.clone());
        self.part(
            &mesh,
            &m,
            Transform::from_xyz(0.0, y, 0.0)
                .with_rotation(Quat::from_rotation_y(FRAC_PI_4))
                .with_scale(size),
        );
    }
}

fn spawn_structure_model(
    commands: &mut Commands,
    props: &Props,
    kind: Kind,
    pos: Vec2,
    ghost: Option<Handle<StandardMaterial>>,
    rng: &mut Rng,
) -> Entity {
    let yaw = (rng.range(-1.0, 1.0) * 0.5).round() * PI / 2.0 + rng.range(-0.08, 0.08);
    let root = commands
        .spawn((
            Transform::from_xyz(pos.x, 0.0, pos.y).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    if kind.is_worksite() {
        fill_worksite_model(commands, props, kind, root, pos, ghost);
    } else {
        fill_structure_model(commands, props, kind, root, pos, ghost, rng);
    }
    root
}

fn fill_worksite_model(commands: &mut Commands, props: &Props, kind: Kind, root: Entity, pos: Vec2, ghost: Option<Handle<StandardMaterial>>) {
    let mut b = Builder {
        commands,
        props,
        root,
        origin: pos,
        ghost,
    };
    let (s, cyl) = (props.sphere.clone(), props.cyl.clone());
    match kind {
        Kind::FishingGround => {
            // A ring of floats marking the ground, and a flagged pole buoy.
            for k in 0..8 {
                let a = k as f32 / 8.0 * TAU;
                let m = if k % 2 == 0 { props.buoy.clone() } else { props.white.clone() };
                b.part(&s, &m, Transform::from_xyz(a.cos() * 8.0, 0.15, a.sin() * 8.0).with_scale(Vec3::splat(0.6)));
            }
            b.part(&s, &props.buoy, Transform::from_xyz(0.0, 0.2, 0.0).with_scale(Vec3::new(1.0, 0.7, 1.0)));
            b.part(&cyl, &props.bamboo, Transform::from_xyz(0.0, 1.5, 0.0).with_scale(Vec3::new(0.1, 2.8, 0.1)));
            b.cube(&props.paints[0], Vec3::new(0.0, 2.6, 0.45), Vec3::new(0.04, 0.5, 0.9));
        }
        Kind::DiveSite => {
            b.cube(&props.bamboo, Vec3::new(0.0, 0.1, 0.0), Vec3::new(2.2, 0.2, 2.2));
            b.part(&cyl, &props.bamboo, Transform::from_xyz(0.0, 1.2, 0.0).with_scale(Vec3::new(0.08, 2.2, 0.08)));
            b.cube(&props.paints[3], Vec3::new(0.0, 2.0, 0.4), Vec3::new(0.04, 0.45, 0.8));
            for x in [-2.5, 2.5] {
                b.part(&s, &props.buoy, Transform::from_xyz(x, 0.15, 1.5).with_scale(Vec3::splat(0.5)));
            }
        }
        Kind::Woodcutter | Kind::NipaGrove => {
            // A lean-to shelter on the beach.
            for (x, z, h) in [(-1.5, -1.0, 2.2), (1.5, -1.0, 2.2), (-1.5, 1.0, 1.4), (1.5, 1.0, 1.4)] {
                b.cube(&props.wood_dark, Vec3::new(x, h / 2.0, z), Vec3::new(0.12, h, 0.12));
            }
            let roof = b.cube(&props.thatch, Vec3::new(0.0, 1.9, 0.0), Vec3::new(3.6, 0.15, 2.6));
            b.commands.entity(roof).insert(
                Transform::from_xyz(0.0, 1.85, 0.0)
                    .with_rotation(Quat::from_rotation_x(0.3))
                    .with_scale(Vec3::new(3.6, 0.15, 2.6)),
            );
            if kind == Kind::Woodcutter {
                // Log pile and chopping block.
                for i in 0..5 {
                    let (x, y) = ((i % 3) as f32 * 0.45 - 0.45, 0.25 + (i / 3) as f32 * 0.4);
                    b.part(&cyl, &props.trunk, Transform::from_xyz(2.6 + x, y, 0.5)
                        .with_rotation(Quat::from_rotation_x(PI / 2.0))
                        .with_scale(Vec3::new(0.4, 2.0, 0.4)));
                }
                b.part(&cyl, &props.bark, Transform::from_xyz(-2.4, 0.3, 1.4).with_scale(Vec3::new(0.7, 0.6, 0.7)));
            } else {
                // Bundles of leaves drying on a frame.
                for i in 0..4 {
                    let m = props.leaves[i % props.leaves.len()].clone();
                    b.part(&s, &m, Transform::from_xyz(2.4, 0.3 + i as f32 * 0.25, 0.6 - (i % 2) as f32 * 0.4)
                        .with_scale(Vec3::new(1.6, 0.25, 0.8)));
                }
                b.cube(&props.bamboo, Vec3::new(-2.4, 1.2, 1.2), Vec3::new(0.1, 0.1, 2.0));
                for i in 0..4 {
                    let m = props.leaves[(i + 1) % props.leaves.len()].clone();
                    b.part(&s, &m, Transform::from_xyz(-2.4, 0.8, 0.5 + i as f32 * 0.45)
                        .with_scale(Vec3::new(0.2, 0.8, 0.35)));
                }
            }
        }
        _ => {}
    }
}

fn fill_structure_model(
    commands: &mut Commands,
    props: &Props,
    kind: Kind,
    root: Entity,
    pos: Vec2,
    ghost: Option<Handle<StandardMaterial>>,
    rng: &mut Rng,
) {
    let mut b = Builder {
        commands,
        props,
        root,
        origin: pos,
        ghost,
    };
    let top = DECK_Y + 0.15;
    let paint = props.paints[rng.index(props.paints.len())].clone();
    let flag = props.paints[rng.index(props.paints.len())].clone();
    match kind {
        Kind::House => {
            b.deck(7.0, 7.0);
            b.cube(&paint, Vec3::new(0.0, top + 1.2, -0.4), Vec3::new(5.0, 2.4, 4.2));
            b.cube(&props.dark, Vec3::new(0.0, top + 0.8, 1.72), Vec3::new(0.9, 1.6, 0.06));
            b.cube(&props.dark, Vec3::new(2.52, top + 1.4, -0.4), Vec3::new(0.06, 0.7, 1.0));
            b.roof(top + 3.4, Vec3::new(7.6, 2.4, 6.6));
            // Panji flag pole and a lantern by the door.
            b.cube(&props.wood_dark, Vec3::new(3.1, top + 2.2, 3.1), Vec3::new(0.1, 4.4, 0.1));
            b.cube(&flag, Vec3::new(3.1, top + 4.0, 3.75), Vec3::new(0.04, 0.6, 1.3));
            let (s, l) = (props.sphere.clone(), props.lantern.clone());
            b.part(&s, &l, Transform::from_xyz(0.7, top + 1.9, 1.9).with_scale(Vec3::splat(0.35)));
            let (s, c) = (props.sphere.clone(), props.clay.clone());
            b.part(&s, &c, Transform::from_xyz(-2.6, top + 0.4, 2.6).with_scale(Vec3::new(0.8, 0.9, 0.8)));
        }
        Kind::RainCatcher => {
            b.deck(4.4, 4.4);
            for x in [-0.9, 0.9] {
                let (s, c) = (props.sphere.clone(), props.clay.clone());
                b.part(&s, &c, Transform::from_xyz(x, top + 0.75, 0.3).with_scale(Vec3::new(1.4, 1.6, 1.4)));
                let (cy, c) = (props.cyl.clone(), props.clay.clone());
                b.part(&cy, &c, Transform::from_xyz(x, top + 1.6, 0.3).with_scale(Vec3::new(0.7, 0.3, 0.7)));
                let (cy, w) = (props.cyl.clone(), props.water_in_jar.clone());
                b.part(&cy, &w, Transform::from_xyz(x, top + 1.76, 0.3).with_scale(Vec3::new(0.55, 0.04, 0.55)));
            }
            for x in [-1.9, 1.9] {
                b.cube(&props.wood_dark, Vec3::new(x, top + 1.5, -1.8), Vec3::new(0.12, 3.0, 0.12));
            }
            let s = b.cube(&props.bamboo, Vec3::new(0.0, top + 2.9, -0.6), Vec3::new(4.2, 0.08, 3.0));
            b.commands.entity(s).insert(Transform::from_xyz(0.0, top + 2.8, -0.6)
                .with_rotation(Quat::from_rotation_x(-0.3))
                .with_scale(Vec3::new(4.2, 0.08, 3.0)));
        }
        Kind::FishPen => {
            let h = 4.5;
            for (x, z) in [(-h, -h), (h, -h), (-h, h), (h, h), (0.0, -h), (0.0, h), (-h, 0.0), (h, 0.0)] {
                b.stilt(Vec2::new(x, z), 1.3, 0.18);
            }
            for (pos, size) in [
                (Vec3::new(0.0, -0.4, -h), Vec3::new(2.0 * h, 2.6, 0.03)),
                (Vec3::new(0.0, -0.4, h), Vec3::new(2.0 * h, 2.6, 0.03)),
                (Vec3::new(-h, -0.4, 0.0), Vec3::new(0.03, 2.6, 2.0 * h)),
                (Vec3::new(h, -0.4, 0.0), Vec3::new(0.03, 2.6, 2.0 * h)),
            ] {
                b.cube(&props.net, pos, size);
                b.cube(&props.bamboo, pos.with_y(0.75), Vec3::new(size.x.max(0.2), 0.18, size.z.max(0.2)));
            }
            // A little floating guard hut in one corner.
            b.cube(&props.wood, Vec3::new(h - 1.0, 0.95, h - 1.0), Vec3::new(2.2, 0.2, 2.2));
            b.cube(&props.thatch, Vec3::new(h - 1.0, 2.2, h - 1.0), Vec3::new(2.4, 0.15, 2.4));
            for (x, z) in [(h - 2.0, h - 2.0), (h, h - 2.0), (h - 2.0, h), (h, h)] {
                b.cube(&props.wood_dark, Vec3::new(x, 1.55, z), Vec3::new(0.1, 1.2, 0.1));
            }
            for k in 0..5 {
                let a = k as f32 * 1.3;
                let (s, f) = (props.sphere.clone(), props.fish.clone());
                b.part(&s, &f, Transform::from_xyz(a.cos() * 2.5, -0.5, a.sin() * 2.5)
                    .with_rotation(Quat::from_rotation_y(a))
                    .with_scale(Vec3::new(0.25, 0.3, 0.8)));
            }
        }
        Kind::SeaweedFarm => {
            for i in 0..6 {
                let z = -4.0 + i as f32 * 1.6;
                let (cy, bm) = (props.cyl.clone(), props.bamboo.clone());
                b.part(&cy, &bm, Transform::from_xyz(0.0, 0.12, z)
                    .with_rotation(Quat::from_rotation_z(PI / 2.0))
                    .with_scale(Vec3::new(0.05, 12.0, 0.05)));
                for k in 0..6 {
                    let x = -5.0 + k as f32 * 2.0;
                    let (s, w) = (props.sphere.clone(), props.seaweed.clone());
                    b.part(&s, &w, Transform::from_xyz(x, 0.05, z).with_scale(Vec3::new(1.1, 0.35, 0.7)));
                }
                for x in [-6.0, 6.0] {
                    let (s, bu) = (props.sphere.clone(), props.buoy.clone());
                    b.part(&s, &bu, Transform::from_xyz(x, 0.2, z).with_scale(Vec3::splat(0.45)));
                }
            }
            b.commands.entity(root).insert(Floating);
        }
        Kind::DryingRack => {
            b.deck(5.0, 4.0);
            for x in [-2.0, 2.0] {
                b.cube(&props.bamboo, Vec3::new(x, top + 0.9, 0.0), Vec3::new(0.12, 1.8, 0.12));
            }
            b.cube(&props.bamboo, Vec3::new(0.0, top + 1.75, 0.0), Vec3::new(4.2, 0.1, 0.1));
            for k in 0..8 {
                let (s, f) = (props.sphere.clone(), props.coconut.clone());
                b.part(&s, &f, Transform::from_xyz(-1.7 + k as f32 * 0.48, top + 1.35, 0.0)
                    .with_rotation(Quat::from_rotation_x(PI / 2.0))
                    .with_scale(Vec3::new(0.22, 0.18, 0.7)));
            }
            b.cube(&props.bamboo, Vec3::new(0.0, top + 0.5, 1.2), Vec3::new(4.0, 0.08, 1.2));
        }
        Kind::Langgal => {
            b.deck(11.0, 11.0);
            for (x, z) in [(-2.5, -2.5), (2.5, 2.5), (-2.5, 2.5), (2.5, -2.5)] {
                b.stilt(Vec2::new(x, z), DECK_Y, 0.22);
            }
            b.cube(&props.white, Vec3::new(0.0, top + 1.5, 0.0), Vec3::new(8.0, 3.0, 8.0));
            b.cube(&props.paints[5], Vec3::new(0.0, top + 0.2, 0.0), Vec3::new(8.1, 0.4, 8.1));
            b.cube(&props.dark, Vec3::new(0.0, top + 1.0, 4.02), Vec3::new(1.4, 2.0, 0.06));
            b.roof(top + 4.2, Vec3::new(12.0, 2.6, 12.0));
            b.roof(top + 6.0, Vec3::new(6.5, 2.0, 6.5));
            let (s, g) = (props.sphere.clone(), props.paints[5].clone());
            b.part(&s, &g, Transform::from_xyz(0.0, top + 7.2, 0.0).with_scale(Vec3::splat(1.3)));
            let (s, l) = (props.sphere.clone(), props.lantern.clone());
            for x in [-1.2, 1.2] {
                b.part(&s, &l, Transform::from_xyz(x, top + 2.2, 4.2).with_scale(Vec3::splat(0.35)));
            }
        }
        _ => {}
    }
}

fn spawn_walkway_model(commands: &mut Commands, props: &Props, a: Vec2, b: Vec2, ghost: Option<Handle<StandardMaterial>>) -> Entity {
    let mid = (a + b) / 2.0;
    let d = b - a;
    let yaw = d.x.atan2(d.y);
    let root = commands
        .spawn((
            Transform::from_xyz(mid.x, 0.0, mid.y).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    fill_walkway_model(commands, props, root, a, b, ghost);
    root
}

fn fill_walkway_model(commands: &mut Commands, props: &Props, root: Entity, a: Vec2, b: Vec2, ghost: Option<Handle<StandardMaterial>>) {
    let mid = (a + b) / 2.0;
    let d = b - a;
    let len = d.length();
    let mut bl = Builder {
        commands,
        props,
        root,
        origin: mid,
        ghost,
    };
    bl.cube(&props.wood, Vec3::new(0.0, DECK_Y + 0.05, 0.0), Vec3::new(1.5, 0.2, len));
    for x in [-0.72, 0.72] {
        bl.cube(&props.wood_dark, Vec3::new(x, DECK_Y + 0.1, 0.0), Vec3::new(0.08, 0.22, len));
    }
    bl.cube(&props.bamboo, Vec3::new(0.7, DECK_Y + 1.0, 0.0), Vec3::new(0.08, 0.08, len));
    let n = (len / 4.5).ceil().max(1.0) as i32;
    let dir = d / len.max(0.01);
    for i in 0..=n {
        let along = -len / 2.0 + len * i as f32 / n as f32;
        // Stilt positions are in world space for the sea-floor lookup.
        let world = mid + dir * along;
        let bottom = seabed_height(world.x, world.y).min(DECK_Y - 0.5) - 0.3;
        let h = DECK_Y - bottom;
        for x in [-0.6, 0.6] {
            bl.cube(&props.wood_dark, Vec3::new(x, bottom + h / 2.0, along), Vec3::new(0.16, h, 0.16));
        }
        bl.cube(&props.bamboo, Vec3::new(0.7, DECK_Y + 0.55, along), Vec3::new(0.08, 0.9, 0.08));
    }
}

/// Swap a finished blueprint's see-through model for the real thing.
pub fn complete_site(
    commands: &mut Commands,
    props: &Props,
    rng: &mut Rng,
    site: Entity,
    structure: Option<&Structure>,
    walkway: Option<&Walkway>,
) {
    commands.entity(site).remove::<Construction>().despawn_related::<Children>();
    if let Some(s) = structure {
        fill_structure_model(commands, props, s.kind, site, s.pos, None, rng);
    } else if let Some(w) = walkway {
        fill_walkway_model(commands, props, site, w.a_pos, w.b_pos, None);
    }
}

// ------------------------------------------------------ build mode ---

fn build_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut build: ResMut<BuildMode>,
    market: Res<crate::market::MarketUi>,
    boat: Res<BoatState>,
    communities: Res<Communities>,
    mut hints: ResMut<Hints>,
    mut log: ResMut<GameLog>,
    mode: Res<ViewMode>,
) {
    if !build.active && !market.open && *mode == ViewMode::Overseer {
        hints.0.push("[B] Place building blueprints".into());
    }
    if !build.active && !market.open && *mode == ViewMode::Captain {
        if let Some(i) = shoal_at(boat.pos) {
            if communities.list[i].is_none() {
                hints.0.push(format!("[B] Build a stilt house to found a village at {}", SHOALS[i].name));
            } else {
                hints.0.push("[B] Build".into());
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyB) && !market.open {
        if build.active || build.category.is_some() {
            build.active = false;
            build.category = None;
        } else {
            let sel = build.selected;
            build.category = CATEGORIES.iter().position(|(_, ks)| ks.contains(&sel)).or(Some(0));
            build.active = true;
            log.push("Pick a building from the bar, point at the water, and left-click (or Enter) where the ghost turns GREEN.");
        }
        build.walk_start = None;
    }
    if keys.just_pressed(KeyCode::Escape) {
        build.active = false;
        build.category = None;
        build.walk_start = None;
    }
    let Some(cat) = build.category else { return };
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k)
            && let Some(&idx) = CATEGORIES[cat].1.get(i)
        {
            build.selected = idx;
            build.active = true;
            build.walk_start = None;
        }
    }
}

pub fn cursor_on_sea(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cams: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (cam, gt) = cams.single().ok()?;
    let ray = cam.viewport_to_world(gt, cursor).ok()?;
    let dist = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let p = ray.get_point(dist);
    Some(Vec2::new(p.x, p.z))
}

fn nearest_node(p: Vec2, structures: &Query<(Entity, &Structure)>, max: f32) -> Option<(Entity, Vec2, usize)> {
    structures
        .iter()
        .filter(|(_, s)| s.kind.is_node() && s.pos.distance(p) < max)
        .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
        .map(|(e, s)| (e, s.pos, s.community))
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(0.001)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn walkway_cost(len: f32) -> i32 {
    ((len / 4.0).ceil() as i32).max(2)
}

#[allow(clippy::too_many_arguments)]
fn build_cursor(
    mut build: ResMut<BuildMode>,
    boat: Res<BoatState>,
    inv: Res<Inventory>,
    communities: Res<Communities>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    structures: Query<(Entity, &Structure)>,
    walkways: Query<&Walkway>,
    worksites: Query<&Worksite>,
) {
    if !build.active {
        return;
    }
    // Without the mouse over the sea, preview just ahead of the boat.
    build.cursor = cursor_on_sea(&windows, &cams).or(Some(boat.pos + boat.forward() * 14.0));
    let Some(p) = build.cursor else { return };
    let kind = build.kind();
    let (valid, reason) = (|| -> (bool, String) {
        if kind.is_worksite() {
            if let Some(w) = worksites.iter().find(|w| w.pos.distance(p) < w.kind.radius() + kind.radius()) {
                return (false, format!("Too close to the {}", w.kind.short()));
            }
            let ok = match kind {
                Kind::FishingGround => {
                    if depth_at(p) < 9.0 || shoal_at(p).is_some() {
                        return (false, "Fishing grounds go in deep blue water, away from the reefs".into());
                    }
                    true
                }
                Kind::DiveSite => {
                    let d = depth_at(p);
                    if shoal_at(p).is_none() || !(1.2..6.0).contains(&d) {
                        return (false, "Dive sites go on a reef flat, 1-6 m deep".into());
                    }
                    true
                }
                _ => {
                    let h = seabed_height(p.x, p.y);
                    if !(0.8..9.0).contains(&h) || p.distance(LAND_CENTER) > LAND_RADIUS * 0.72 {
                        return (false, "Camps go on Bongao island, among the trees near the shore".into());
                    }
                    if p.distance(crate::world::MARKET_DOCK) < 70.0 {
                        return (false, "Too close to the market - pick another stretch of shore".into());
                    }
                    true
                }
            };
            let cost = kind.cost();
            if ok && inv.pesos < cost.pesos {
                return (false, format!("Needs P{} - you have P{}", cost.pesos, inv.pesos));
            }
            return (true, format!("LEFT-CLICK (or Enter): place the {} - an idle family will work it", kind.short()));
        }
        if kind == Kind::Walkway {
            let Some((e, epos, comm)) = nearest_node(p, &structures, 7.0) else {
                return (false, "Point at a house, langgal, rain catcher or drying rack to connect it".into());
            };
            let Some(start) = build.walk_start else {
                return (true, "LEFT-CLICK (or Enter) to start the walkway at this platform".into());
            };
            let Ok((_, s)) = structures.get(start) else {
                return (false, "Start point is gone".into());
            };
            if e == start {
                return (false, "Pick a second platform".into());
            }
            if comm != s.community {
                return (false, "Walkways must stay inside one village".into());
            }
            let len = epos.distance(s.pos);
            if len > 42.0 {
                return (false, "Too long (max 42 m)".into());
            }
            if walkways.iter().any(|w| (w.a == e && w.b == start) || (w.a == start && w.b == e)) {
                return (false, "Already connected".into());
            }
            let wood = walkway_cost(len);
            return (true, format!("LEFT-CLICK (or Enter) to place the walkway blueprint (builders fetch {wood} timber)"));
        }
        let Some(shoal) = shoal_at(p) else {
            return (false, "Build on a shallow reef flat - the turquoise water".into());
        };
        let depth = depth_at(p);
        let (lo, hi) = kind.depth_range();
        if depth < lo {
            return (false, "Too shallow here".into());
        }
        if depth > hi {
            return (false, "Too deep here - move toward the lighter turquoise water".into());
        }
        let community = communities.list[shoal].as_ref();
        if community.is_none() && kind != Kind::House {
            return (false, "Found a village first by building a stilt house".into());
        }
        if kind == Kind::Langgal && community.is_some_and(|c| c.hall) {
            return (false, "This village already has a langgal".into());
        }
        for (_, s) in structures.iter() {
            if s.pos.distance(p) < s.kind.radius() + kind.radius() {
                return (false, format!("Too close to a {}", s.kind.short()));
            }
        }
        for w in walkways.iter() {
            if seg_dist(p, w.a_pos, w.b_pos) < kind.radius() + 0.6 {
                return (false, "Blocked by a walkway".into());
            }
        }
        let cost = kind.cost();
        if inv.pesos < cost.pesos {
            return (false, format!("Needs P{} for rope and nails - you have P{}. Sell fish first.", cost.pesos, inv.pesos));
        }
        let mats = if cost.nipa > 0 {
            format!("{} timber + {} nipa", cost.timber, cost.nipa)
        } else {
            format!("{} timber", cost.timber)
        };
        if community.is_none() {
            return (true, format!("LEFT-CLICK (or Enter): blueprint to found the village of {} (P{} now, builders bring {mats})", SHOALS[shoal].name, cost.pesos));
        }
        (true, format!("LEFT-CLICK (or Enter): {} blueprint (P{} now, builders bring {mats})", kind.short(), cost.pesos))
    })();
    build.valid = valid;
    build.reason = reason;
}

#[allow(clippy::too_many_arguments)]
fn build_preview(
    mut commands: Commands,
    mut build: ResMut<BuildMode>,
    props: Res<Props>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
    structures: Query<(Entity, &Structure)>,
    mut gizmos: Gizmos,
) {
    let wanted = if build.active && build.kind() != Kind::Walkway { build.cursor } else { None };
    // Rebuild the ghost when the kind changes or it moves far enough for stilts to matter.
    let stale = match (build.ghost, wanted) {
        (Some((_, k, at)), Some(p)) => k != build.selected || at.distance(p) > 1.5,
        (Some(_), None) => true,
        (None, Some(_)) => true,
        (None, None) => false,
    };
    if stale {
        if let Some((e, _, _)) = build.ghost.take() {
            commands.entity(e).despawn();
        }
        if let Some(p) = wanted {
            let e = spawn_structure_model(&mut commands, &props, build.kind(), p, build.ghost_mat.clone(), &mut rng);
            commands.entity(e).insert(Ghost);
            build.ghost = Some((e, build.selected, p));
        }
    }
    if let Some(h) = &build.ghost_mat
        && let Some(mut m) = materials.get_mut(h)
    {
        m.base_color = if build.valid {
            Color::srgba(0.45, 1.0, 0.55, 0.45)
        } else {
            Color::srgba(1.0, 0.35, 0.3, 0.45)
        };
    }
    if !build.active {
        return;
    }
    let col = if build.valid { Color::srgb(0.5, 1.0, 0.6) } else { Color::srgb(1.0, 0.4, 0.35) };
    let flat = Quat::from_rotation_x(PI / 2.0);
    if let Some(p) = build.cursor {
        if build.kind() == Kind::Walkway {
            if let Some((_, npos, _)) = nearest_node(p, &structures, 7.0) {
                gizmos.circle(Isometry3d::new(Vec3::new(npos.x, DECK_Y + 0.3, npos.y), flat), 3.0, col);
            }
            if let Some(start) = build.walk_start
                && let Ok((_, s)) = structures.get(start)
            {
                let end = nearest_node(p, &structures, 7.0).map(|n| n.1).unwrap_or(p);
                gizmos.line(
                    Vec3::new(s.pos.x, DECK_Y + 0.3, s.pos.y),
                    Vec3::new(end.x, DECK_Y + 0.3, end.y),
                    col,
                );
                gizmos.circle(Isometry3d::new(Vec3::new(s.pos.x, DECK_Y + 0.3, s.pos.y), flat), 3.2, Color::srgb(1.0, 0.9, 0.4));
            }
        } else {
            gizmos.circle(Isometry3d::new(Vec3::new(p.x, 0.3, p.y), flat), build.kind().radius(), col);
        }
    }
    // Show shoal outlines so players can see where villages may go.
    for s in SHOALS.iter() {
        gizmos.circle(
            Isometry3d::new(Vec3::new(s.center.x, 0.4, s.center.y), flat),
            s.radius * 1.05,
            Color::srgba(1.0, 1.0, 0.8, 0.35),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn place_structure(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    mut build: ResMut<BuildMode>,
    mut inv: ResMut<Inventory>,
    mut communities: ResMut<Communities>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut rng: ResMut<Rng>,
    props: Res<Props>,
    structures: Query<(Entity, &Structure)>,
    keys: Res<ButtonInput<KeyCode>>,
    ui: Query<&Interaction, Or<(With<Button>, With<UiBlocker>)>>,
    chars: Res<CharAssets>,
) {
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    let clicked = mouse.just_pressed(MouseButton::Left) && !over_ui;
    if !build.active || !(clicked || keys.just_pressed(KeyCode::Enter)) {
        return;
    }
    let Some(p) = build.cursor else { return };
    let kind = build.kind();
    if kind == Kind::Walkway {
        let node = nearest_node(p, &structures, 7.0);
        match (build.walk_start, node) {
            (None, Some((e, _, _))) => build.walk_start = Some(e),
            (Some(start), Some((e, epos, comm))) if build.valid => {
                let Ok((_, s)) = structures.get(start) else { return };
                let len = epos.distance(s.pos);
                let root = spawn_walkway_model(&mut commands, &props, s.pos, epos, build.blueprint_mat.clone());
                commands.entity(root).insert((
                    Walkway {
                        a: start,
                        b: e,
                        a_pos: s.pos,
                        b_pos: epos,
                        community: comm,
                    },
                    Construction::new(Kind::Walkway, walkway_cost(len), 0),
                ));
                sfx.play(Sfx::Knock, 0.8);
                // Chain walkways: the end becomes the next start.
                build.walk_start = Some(e);
            }
            (_, None) => build.walk_start = None,
            _ => {}
        }
        return;
    }
    if !build.valid {
        log.push(build.reason.clone());
        return;
    }
    if kind.is_worksite() {
        inv.pesos -= kind.cost().pesos;
        sfx.play(Sfx::Knock, 0.5);
        place_worksite(&mut commands, &props, &chars, &mut rng, &communities, &mut log, kind, p);
        return;
    }
    let Some(shoal) = shoal_at(p) else { return };
    inv.pesos -= kind.cost().pesos;
    sfx.play(Sfx::Knock, 0.5);
    place_blueprint(&mut commands, &props, &build, &mut rng, &mut communities, &mut log, kind, shoal, p);
}

/// Put down a see-through blueprint that Build families will construct.
#[allow(clippy::too_many_arguments)]
pub fn place_blueprint(
    commands: &mut Commands,
    props: &Props,
    build: &BuildMode,
    rng: &mut Rng,
    communities: &mut Communities,
    log: &mut GameLog,
    kind: Kind,
    shoal: usize,
    p: Vec2,
) -> Entity {
    let cost = kind.cost();
    let root = spawn_structure_model(commands, props, kind, p, build.blueprint_mat.clone(), rng);
    let a = rng.range(0.0, TAU);
    commands.entity(root).insert((
        Structure {
            kind,
            community: shoal,
            health: 100.0,
            pos: p,
            tilt_axis: Vec3::new(a.cos(), 0.0, a.sin()),
        },
        Construction::new(kind, cost.timber, cost.nipa),
    ));
    match &mut communities.list[shoal] {
        None => {
            communities.list[shoal] = Some(Community::new(shoal, root));
            log.push(format!(
                "Blueprint for the first house of {} placed! When it's built, new families can settle there.",
                SHOALS[shoal].name
            ));
        }
        Some(c) => {
            if c.anchor == Entity::PLACEHOLDER && kind == Kind::House {
                c.anchor = root;
            }
            log.push(format!(
                "{} blueprint placed. A family on 'Build & repair' will bring the materials and build it.",
                kind.name()
            ));
        }
    }
    root
}

/// Put down a work area; the families module staffs it with an idle family.
#[allow(clippy::too_many_arguments)]
pub fn place_worksite(
    commands: &mut Commands,
    props: &Props,
    chars: &CharAssets,
    rng: &mut Rng,
    communities: &Communities,
    log: &mut GameLog,
    kind: Kind,
    p: Vec2,
) -> Entity {
    let on_land = matches!(kind, Kind::Woodcutter | Kind::NipaGrove);
    let ground = if on_land { seabed_height(p.x, p.y) - 0.1 } else { 0.0 };
    let out = (p - LAND_CENTER).normalize_or(Vec2::X);
    let root = commands
        .spawn((
            Transform::from_xyz(p.x, ground, p.y).with_rotation(Quat::from_rotation_y(out.x.atan2(out.y))),
            Visibility::default(),
        ))
        .id();
    fill_worksite_model(commands, props, kind, root, p, None);
    let mut mooring = match kind {
        Kind::DiveSite => p + Vec2::new(3.0, 0.0),
        _ => p,
    };
    if on_land {
        // Walk out from the camp until the water is deep enough for a boat.
        let mut q = p;
        for _ in 0..80 {
            q += out * 2.0;
            if depth_at(q) > 1.2 {
                break;
            }
        }
        mooring = q + out * 2.0;
        // A worker who appears while the family is ashore.
        let worker = spawn_character(commands, chars, CharStyle::random(rng, false));
        commands.entity(worker).insert((
            Transform::from_xyz(-1.0, 0.0, 1.8),
            Visibility::Hidden,
            CampWorker(root),
            ChildOf(root),
        ));
        // Plant a few extra trees around the camp.
        for k in 0..3 {
            let a = k as f32 * 2.1 + 0.5;
            let q = p + Vec2::new(a.cos(), a.sin()) * rng.range(7.0, 11.0);
            let h = seabed_height(q.x, q.y);
            if h > 0.6 {
                if kind == Kind::NipaGrove {
                    spawn_palm(commands, props, Vec3::new(q.x, h - 0.2, q.y), rng);
                } else {
                    spawn_canopy_tree(commands, props, Vec3::new(q.x, h - 0.2, q.y), rng, None);
                }
            }
        }
    } else {
        commands.entity(root).insert(Floating);
    }
    let community = communities
        .list
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.as_ref().map(|c| (i, c.center.distance(p))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let stock = match kind {
        Kind::FishingGround => 30.0,
        Kind::DiveSite => 12.0,
        _ => 0.0,
    };
    commands.entity(root).insert(Worksite {
        kind,
        pos: p,
        mooring,
        community,
        worker: None,
        stock,
    });
    log.push(format!("{} placed. The next idle family (on 'Rest') will work it.", kind.name()));
    root
}

fn regrow_worksites(time: Res<Time>, mut sites: Query<&mut Worksite>) {
    let dt = time.delta_secs();
    for mut w in &mut sites {
        let rate = match w.kind {
            Kind::FishingGround => 0.35,
            Kind::DiveSite => 0.12,
            _ => 0.0,
        };
        w.stock = (w.stock + rate * dt).min(w.max_stock());
    }
}

// ------------------------------------------------------ simulation ---

fn update_stats(
    mut communities: ResMut<Communities>,
    structures: Query<(Entity, &Structure), Without<Construction>>,
    walkways: Query<&Walkway, Without<Construction>>,
    families: Query<&Family>,
) {
    let mut adj: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for w in &walkways {
        adj.entry(w.a).or_default().push(w.b);
        adj.entry(w.b).or_default().push(w.a);
    }
    for (i, slot) in communities.list.iter_mut().enumerate() {
        let Some(c) = slot else { continue };
        // Everything reachable from the founding house counts as connected.
        let mut seen: HashSet<Entity> = HashSet::new();
        let mut queue = VecDeque::from([c.anchor]);
        while let Some(e) = queue.pop_front() {
            if seen.insert(e) {
                if let Some(n) = adj.get(&e) {
                    queue.extend(n.iter().copied());
                }
            }
        }
        c.capacity = 0;
        c.houses = 0;
        c.connected_houses = 0;
        c.damaged = 0;
        c.catchers = 0;
        c.pens = 0;
        c.farms = 0;
        c.racks = 0;
        c.hall = false;
        c.families = 0;
        c.idle_families = 0;
        for f in families.iter().filter(|f| f.community == i && !f.leaving) {
            c.families += 1;
            if f.task == Task::Rest {
                c.idle_families += 1;
            }
        }
        for (e, s) in structures.iter().filter(|(_, s)| s.community == i) {
            if s.health < 99.0 {
                c.damaged += 1;
            }
            let ok = s.working();
            match s.kind {
                Kind::House => {
                    c.houses += 1;
                    if ok {
                        c.capacity += 1;
                    }
                    if seen.contains(&e) {
                        c.connected_houses += 1;
                    }
                }
                Kind::RainCatcher if ok => c.catchers += 1,
                Kind::FishPen if ok => c.pens += 1,
                Kind::SeaweedFarm if ok => c.farms += 1,
                Kind::DryingRack if ok => c.racks += 1,
                Kind::Langgal if ok => c.hall = true,
                _ => {}
            }
        }
        c.water_cap = 12.0 + 30.0 * c.catchers as f32;
    }
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    mut clock: ResMut<SimClock>,
    weather: Res<Weather>,
    mut communities: ResMut<Communities>,
    mut structures: Query<(Entity, &mut Structure), Without<Construction>>,
    walkways: Query<&Walkway, Without<Construction>>,
    mut log: ResMut<GameLog>,
    mut rng: ResMut<Rng>,
) {
    clock.0 += time.delta_secs();
    if clock.0 < 1.0 {
        return;
    }
    clock.0 -= 1.0;
    let dt = 1.0;
    let raging = weather.raging();

    // Storm damage. Walkways brace the houses they join.
    if raging {
        let braced: HashSet<Entity> = walkways.iter().flat_map(|w| [w.a, w.b]).collect();
        let mut hits: HashMap<usize, u32> = HashMap::new();
        for (e, mut s) in &mut structures {
            let mut chance = 0.014 * weather.storm;
            if braced.contains(&e) {
                chance *= 0.45;
            }
            if s.kind == Kind::Langgal {
                chance *= 0.5;
            }
            if rng.chance(chance) {
                s.health = (s.health - rng.range(25.0, 55.0)).max(0.0);
                *hits.entry(s.community).or_default() += 1;
            }
        }
        for (i, n) in hits {
            if let Some(c) = &communities.list[i] {
                log.push(format!("The storm batters {}: {n} structure(s) damaged.", c.name));
            }
        }
    }

    for c in communities.list.iter_mut().flatten() {
        let fam = c.families as f32;
        // Food: resting families glean shellfish near home; pens add more.
        // Nobody gathers in a storm.
        if !raging {
            c.food += c.idle_families as f32 / 30.0 * dt + c.pens as f32 / 16.0 * dt;
            c.seaweed = (c.seaweed + c.farms as f32 / 22.0 * dt).min(40.0 * c.farms.max(1) as f32);
        }
        c.food = (c.food - fam / 30.0 * dt).max(0.0);
        // Surplus fish goes on the drying racks.
        if c.racks > 0 && c.food > fam * 6.0 + 2.0 {
            let amount = (c.racks as f32 * 2.0 / 15.0 * dt).min(c.food - fam * 6.0);
            c.food -= amount;
            c.dried += amount / 2.0;
        }
        // Water: rain fills the jars; everyone drinks.
        c.water = (c.water + c.catchers as f32 * weather.rain * 0.3 * dt).min(c.water_cap);
        c.water = (c.water - fam / 36.0 * dt).max(0.0);
        // Warn before hunger or thirst sets in.
        if fam > 0.0 && c.food < fam * 4.0 && !c.warned_food {
            c.warned_food = true;
            log.push(format!("{} is running low on fish! Place a Fishing ground (Food tab) or put a family on 'Fish for the village'.", c.name));
        } else if c.food > fam * 8.0 {
            c.warned_food = false;
        }
        if fam > 0.0 && c.water < fam * 4.0 && !c.warned_water {
            c.warned_water = true;
            log.push(format!("{} is running low on water! Build rain catchers or put a family on 'Fetch water'.", c.name));
        } else if c.water > fam * 8.0 {
            c.warned_water = false;
        }

        let mut target = 50.0;
        target += if c.food <= 0.0 {
            -30.0
        } else if c.food < fam * 3.0 {
            0.0
        } else {
            12.0
        };
        target += if c.water <= 0.0 {
            -35.0
        } else if c.water < fam * 3.0 {
            0.0
        } else {
            12.0
        };
        target += if c.houses > 1 {
            15.0 * c.connected_houses as f32 / c.houses as f32 - 7.0
        } else {
            3.0
        };
        if c.hall {
            target += 15.0;
        }
        target -= (c.damaged as f32 * 6.0).min(24.0);
        // Sama can live on their boats, but a house makes a family happier.
        if c.families > 0 {
            let housed = c.families.min(c.capacity) as f32 / c.families as f32;
            target += 10.0 * housed - 6.0;
        }
        if c.pens > 0 {
            target += 4.0;
        }
        if c.farms > 0 {
            target += 3.0;
        }
        if raging {
            target -= 5.0;
        }
        c.approval_target = target.clamp(0.0, 100.0);
        c.approval += (c.approval_target - c.approval) * 0.04 * dt;
    }
}

#[allow(clippy::too_many_arguments)]
fn village_interact(
    keys: Res<ButtonInput<KeyCode>>,
    boat: Res<BoatState>,
    mut inv: ResMut<Inventory>,
    mut communities: ResMut<Communities>,
    mut structures: Query<&mut Structure>,
    mut log: ResMut<GameLog>,
    mut sfx: ResMut<SfxQueue>,
    mut hints: ResMut<Hints>,
    build: Res<BuildMode>,
    mode: Res<ViewMode>,
) {
    if *mode != ViewMode::Captain {
        return;
    }
    let Some(i) = communities.at(boat.pos) else { return };
    let Some(c) = communities.list[i].as_mut() else { return };
    let damaged = structures.iter().filter(|s| s.community == i && s.health < 99.0).count();
    let _ = &mut structures;
    hints.0.push(format!("[G] Trade with {} (drop fish & water, collect goods)", c.name));
    if damaged > 0 {
        hints.0.push(format!("[R] Repair {damaged} damaged structure(s)"));
    }
    if build.active {
        return;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        let fish = inv.fish;
        c.food += fish as f32;
        inv.fish = 0;
        let space = (c.water_cap - c.water).max(0.0).floor() as u32;
        let jars = inv.water_jars.min(space);
        c.water += jars as f32;
        inv.water_jars -= jars;
        let room = boat.tier.stats().capacity.saturating_sub(inv.cargo());
        let dried = (c.dried.floor() as u32).min(room);
        c.dried -= dried as f32;
        inv.dried_fish += dried;
        let room = room - dried;
        let weed = (c.seaweed.floor() as u32).min(room);
        c.seaweed -= weed as f32;
        inv.seaweed += weed;
        sfx.play(Sfx::Knock, 0.4);
        let mut parts = Vec::new();
        if fish > 0 {
            parts.push(format!("gave {fish} fish"));
        }
        if jars > 0 {
            parts.push(format!("poured {jars} jars of water"));
        }
        if dried > 0 {
            parts.push(format!("loaded {dried} dried fish"));
        }
        if weed > 0 {
            parts.push(format!("loaded {weed} seaweed"));
        }
        if parts.is_empty() {
            log.push(format!("{}: nothing to trade right now.", c.name));
        } else {
            log.push(format!("{}: {}.", c.name, parts.join(", ")));
        }
    }
    if keys.just_pressed(KeyCode::KeyR) && damaged > 0 {
        let mut fixed = 0;
        let mut short = false;
        for mut s in structures.iter_mut().filter(|s| s.community == i && s.health < 99.0) {
            let cost = ((100.0 - s.health) / 20.0).ceil() as i32;
            if inv.wood >= cost {
                inv.wood -= cost;
                s.health = 100.0;
                fixed += 1;
            } else {
                short = true;
            }
        }
        if fixed > 0 {
            sfx.play(Sfx::Knock, 0.9);
            log.push(format!("Repaired {fixed} structure(s) in {}.", c.name));
        }
        if short {
            log.push("Not enough wood to repair everything. Buy more at Bongao.");
        }
    }
}

fn show_damage(mut q: Query<(&Structure, &mut Transform), Changed<Structure>>) {
    for (s, mut t) in &mut q {
        let broken = 1.0 - s.health / 100.0;
        let yaw = t.rotation.to_euler(EulerRot::YXZ).0;
        t.rotation = Quat::from_axis_angle(s.tilt_axis, broken * 0.3) * Quat::from_rotation_y(yaw);
        t.translation.y = -broken * 0.8;
    }
}

fn float_farms(sea: Res<Sea>, mut q: Query<&mut Transform, (With<Floating>, Without<Ghost>)>) {
    for mut t in &mut q {
        let p = Vec2::new(t.translation.x, t.translation.z);
        t.translation.y = sea_height(&sea, p);
    }
}

// -------------------------------------------------------- villagers ---

#[allow(clippy::too_many_arguments)]
fn sync_villagers(
    mut commands: Commands,
    communities: Res<Communities>,
    chars: Res<CharAssets>,
    mut rng: ResMut<Rng>,
    villagers: Query<(Entity, &Villager)>,
    structures: Query<(Entity, &Structure), Without<Construction>>,
) {
    for (i, slot) in communities.list.iter().enumerate() {
        let Some(c) = slot else { continue };
        let want = (c.families * 2).min(22) as usize;
        let have: Vec<Entity> = villagers.iter().filter(|(_, v)| v.community == i).map(|(e, _)| e).collect();
        if have.len() > want {
            for e in have.iter().skip(want) {
                commands.entity(*e).despawn();
            }
        } else if have.len() < want {
            let homes: Vec<(Entity, Vec2)> = structures
                .iter()
                .filter(|(_, s)| s.community == i && s.kind == Kind::House)
                .map(|(e, s)| (e, s.pos))
                .collect();
            if homes.is_empty() {
                continue;
            }
            // One new villager per frame keeps spawning smooth.
            let (home, pos) = homes[rng.index(homes.len())];
            let child = have.len() % 2 == 1;
            let e = spawn_character(&mut commands, &chars, CharStyle::random(&mut rng, child));
            let p = Vec3::new(pos.x + rng.range(-2.0, 2.0), DECK_Y + 0.15, pos.y + rng.range(-2.0, 2.0));
            let scale = if child { 0.68 } else { 1.0 };
            commands.entity(e).insert((
                Transform::from_translation(p).with_scale(Vec3::splat(scale)),
                Villager {
                    community: i,
                    node: home,
                    target: home,
                    from: p,
                    to: p,
                    t: 1.0,
                    dur: 1.0,
                    idle: rng.range(0.5, 4.0),
                },
            ));
        }
    }
}

fn walk_villagers(
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut villagers: Query<(&mut Villager, &mut Transform, &mut Toon)>,
    structures: Query<&Structure>,
    walkways: Query<&Walkway, Without<Construction>>,
    langgals: Query<(Entity, &Structure), (Without<Construction>, With<Structure>)>,
    prayer: Res<crate::prayer::Prayer>,
) {
    let dt = time.delta_secs();
    let praying = prayer.active.is_some();
    let mut adj: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for w in &walkways {
        adj.entry(w.a).or_default().push(w.b);
        adj.entry(w.b).or_default().push(w.a);
    }
    for (mut v, mut t, mut toon) in &mut villagers {
        if v.t < 1.0 {
            v.t = (v.t + dt / v.dur).min(1.0);
            let p = v.from.lerp(v.to, v.t);
            t.translation = p;
            toon.moving = true;
            if v.t >= 1.0 {
                v.node = v.target;
                v.idle = if praying { 0.2 } else { rng.range(1.5, 6.0) };
            }
            continue;
        }
        toon.moving = false;
        v.idle -= dt;
        if v.idle > 0.0 {
            continue;
        }
        let Ok(here) = structures.get(v.node) else { continue };
        let neighbours = adj.get(&v.node).cloned().unwrap_or_default();
        // At prayer time, walk the walkways toward the village langgal.
        let langgal = langgals
            .iter()
            .find(|(_, s)| s.kind == Kind::Langgal && s.community == v.community)
            .map(|(e, _)| e);
        let toward_langgal = if praying { langgal.and_then(|l| next_step(&adj, v.node, l)) } else { None };
        let (target, dest) = if let Some(n) = toward_langgal {
            match structures.get(n) {
                Ok(s) => (n, s.pos + Vec2::new(rng.range(-1.2, 1.2), rng.range(-1.2, 1.2))),
                Err(_) => (v.node, here.pos),
            }
        } else if praying {
            // Already at the langgal (or no way there): stay and pray.
            v.idle = 1.0;
            continue;
        } else if !neighbours.is_empty() && rng.chance(0.6) {
            let n = neighbours[rng.index(neighbours.len())];
            match structures.get(n) {
                Ok(s) => (n, s.pos + Vec2::new(rng.range(-1.5, 1.5), rng.range(-1.5, 1.5))),
                Err(_) => (v.node, here.pos),
            }
        } else {
            // Potter about on the home deck.
            (v.node, here.pos + Vec2::new(rng.range(-2.4, 2.4), rng.range(-2.4, 2.4)))
        };
        let to = Vec3::new(dest.x, t.translation.y, dest.y);
        let dist = t.translation.distance(to);
        v.from = t.translation;
        v.to = to;
        v.target = target;
        v.t = 0.0;
        v.dur = (dist / 1.5).max(0.4);
        let d = to - t.translation;
        if d.length_squared() > 0.01 {
            t.rotation = Quat::from_rotation_y(d.x.atan2(d.z));
        }
    }
}

/// First step on the shortest walkway path from `from` to `to`.
fn next_step(adj: &HashMap<Entity, Vec<Entity>>, from: Entity, to: Entity) -> Option<Entity> {
    if from == to {
        return None;
    }
    let mut prev: HashMap<Entity, Entity> = HashMap::new();
    let mut queue = VecDeque::from([from]);
    let mut seen = HashSet::from([from]);
    while let Some(e) = queue.pop_front() {
        if e == to {
            let mut step = to;
            while let Some(&p) = prev.get(&step) {
                if p == from {
                    return Some(step);
                }
                step = p;
            }
            return None;
        }
        for &n in adj.get(&e).into_iter().flatten() {
            if seen.insert(n) {
                prev.insert(n, e);
                queue.push_back(n);
            }
        }
    }
    None
}

/// `TAHIK_DEMO`: pre-build a small village at Sitangkai for testing.
fn demo_village(
    mut commands: Commands,
    props: Res<Props>,
    mut communities: ResMut<Communities>,
    mut rng: ResMut<Rng>,
) {
    if std::env::var("TAHIK_DEMO").is_err() {
        return;
    }
    let shoal = 0;
    let s = &SHOALS[shoal];
    let plan = [
        Kind::House, Kind::House, Kind::House, Kind::RainCatcher, Kind::House,
        Kind::DryingRack, Kind::Langgal, Kind::FishPen, Kind::SeaweedFarm,
    ];
    let mut placed: Vec<(Entity, Kind, Vec2)> = Vec::new();
    for kind in plan {
        for _ in 0..400 {
            let a = rng.range(0.0, TAU);
            let p = s.center + Vec2::new(a.cos(), a.sin()) * rng.range(0.0, s.radius * 0.8);
            let d = depth_at(p);
            let (lo, hi) = kind.depth_range();
            if d < lo || d > hi {
                continue;
            }
            if p.distance(crate::world::PLAYER_START) < 14.0
                || placed.iter().any(|(_, k, q)| q.distance(p) < k.radius() + kind.radius() + 1.0)
            {
                continue;
            }
            // Keep nodes close together so walkways are short.
            if kind.is_node() && !placed.is_empty() && placed.iter().all(|(_, _, q)| q.distance(p) > 22.0) {
                continue;
            }
            let root = spawn_structure_model(&mut commands, &props, kind, p, None, &mut rng);
            let a = rng.range(0.0, TAU);
            commands.entity(root).insert(Structure {
                kind,
                community: shoal,
                health: 100.0,
                pos: p,
                tilt_axis: Vec3::new(a.cos(), 0.0, a.sin()),
            });
            placed.push((root, kind, p));
            break;
        }
    }
    let nodes: Vec<_> = placed.iter().filter(|(_, k, _)| k.is_node()).cloned().collect();
    for w in nodes.windows(2) {
        let (a, b) = (w[0], w[1]);
        let root = spawn_walkway_model(&mut commands, &props, a.2, b.2, None);
        commands.entity(root).insert(Walkway { a: a.0, b: b.0, a_pos: a.2, b_pos: b.2, community: shoal });
    }
    let mut c = Community::new(shoal, placed[0].0);
    c.food = 30.0;
    c.water = 40.0;
    c.approval = 65.0;
    communities.list[shoal] = Some(c);
}
