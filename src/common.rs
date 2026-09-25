//! Shared game state, helpers and a tiny deterministic RNG.

use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Title,
    Playing,
}

/// Height of the walkable deck on every stilt structure.
pub const DECK_Y: f32 = 2.4;
/// Half the edge length of the playable sea.
pub const WORLD_HALF: f32 = 600.0;

// ---------------------------------------------------------------- RNG ---

#[derive(Resource)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u32(&mut self) -> u32 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    pub fn index(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n.max(1)
    }
    pub fn int(&mut self, lo: i32, hi_inclusive: i32) -> i32 {
        lo + (self.next_u32() % ((hi_inclusive - lo + 1).max(1) as u32)) as i32
    }
}

// -------------------------------------------------------------- noise ---

fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (z as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

/// Smooth value noise in [0, 1].
pub fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - x.floor(), z - z.floor());
    let (ux, uz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = hash2(xi, zi, seed);
    let b = hash2(xi + 1, zi, seed);
    let c = hash2(xi, zi + 1, seed);
    let d = hash2(xi + 1, zi + 1, seed);
    let top = a + (b - a) * ux;
    let bot = c + (d - c) * ux;
    top + (bot - top) * uz
}

/// Fractal noise, roughly in [-1, 1].
pub fn fbm(x: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut amp, mut freq, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for o in 0..octaves {
        sum += amp * (value_noise(x * freq, z * freq, seed + o * 17) * 2.0 - 1.0);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    let t = t.clamp(0.0, 1.0);
    Color::LinearRgba(LinearRgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    ))
}

/// Unit vector on the sea plane for a heading angle (0 = +Z, PI/2 = +X).
pub fn heading_vec(angle: f32) -> Vec2 {
    Vec2::new(angle.sin(), angle.cos())
}

pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    } else if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

// ---------------------------------------------------------- inventory ---

#[derive(Resource)]
pub struct Inventory {
    pub pesos: i32,
    pub wood: i32,
    /// Coconut / nipa leaves for thatched roofs.
    pub nipa: i32,
    pub fish: u32,
    pub urchin: u32,
    pub seaweed: u32,
    pub dried_fish: u32,
    pub water_jars: u32,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            pesos: 150,
            wood: 0,
            nipa: 0,
            fish: 0,
            urchin: 0,
            seaweed: 0,
            dried_fish: 0,
            water_jars: 0,
        }
    }
}

impl Inventory {
    pub fn cargo(&self) -> u32 {
        self.fish + self.urchin + self.seaweed + self.dried_fish + self.water_jars
    }
}

// ---------------------------------------------------------------- log ---

pub struct LogEntry {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Default)]
pub struct GameLog {
    pub entries: Vec<LogEntry>,
}

impl GameLog {
    pub fn push(&mut self, text: impl Into<String>) {
        self.entries.push(LogEntry {
            text: text.into(),
            age: 0.0,
        });
        if self.entries.len() > 7 {
            self.entries.remove(0);
        }
    }
}

// --------------------------------------------------------- sound cues ---

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Splash,
    Thunder,
    Knock,
    Bubbles,
    Thud,
    Paddle,
}

/// Queue of one-shot nature sounds: (sound, delay seconds, volume).
#[derive(Resource, Default)]
pub struct SfxQueue(pub Vec<(Sfx, f32, f32)>);

impl SfxQueue {
    pub fn play(&mut self, sfx: Sfx, volume: f32) {
        self.0.push((sfx, 0.0, volume));
    }
    pub fn play_delayed(&mut self, sfx: Sfx, delay: f32, volume: f32) {
        self.0.push((sfx, delay, volume));
    }
}

#[derive(Component)]
pub struct MainCamera;

/// How the player is playing: overseeing families from above, or steering
/// their own boat.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Overseer,
    Captain,
}

/// Ask the overseer camera to glide to a point.
#[derive(Resource, Default)]
pub struct CameraGoto(pub Option<Vec2>);

/// UI panels that swallow mouse clicks so they don't reach the world.
#[derive(Component)]
pub struct UiBlocker;

pub struct CommonPlugin;

impl Plugin for CommonPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .insert_resource(Rng::new(0x5A_4A_D1_1A_07))
            .init_resource::<Inventory>()
            .init_resource::<GameLog>()
            .init_resource::<SfxQueue>()
            .init_resource::<ViewMode>()
            .init_resource::<CameraGoto>()
            .add_systems(Update, age_log);
    }
}

fn age_log(time: Res<Time>, mut log: ResMut<GameLog>) {
    for e in &mut log.entries {
        e.age += time.delta_secs();
    }
    log.entries.retain(|e| e.age < 22.0);
}
