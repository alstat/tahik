//! Hand-sketched UI icons, painted at startup with signed-distance shapes
//! (so the game needs no image files). Every shape gets a dark ink outline,
//! which gives the icons a drawn, storybook look.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

const N: usize = 96;
const INK: [f32; 4] = [0.20, 0.13, 0.09, 1.0];

#[derive(Resource, Default)]
pub struct Icons(HashMap<&'static str, Handle<Image>>);

impl Icons {
    pub fn get(&self, name: &str) -> Handle<Image> {
        self.0.get(name).cloned().unwrap_or_default()
    }
}

pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, paint_icons);
    }
}

struct Canvas {
    px: Vec<[f32; 4]>,
}

fn sd_tri(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let (e0, e1, e2) = (b - a, c - b, a - c);
    let (v0, v1, v2) = (p - a, p - b, p - c);
    let pq0 = v0 - e0 * (v0.dot(e0) / e0.dot(e0)).clamp(0.0, 1.0);
    let pq1 = v1 - e1 * (v1.dot(e1) / e1.dot(e1)).clamp(0.0, 1.0);
    let pq2 = v2 - e2 * (v2.dot(e2) / e2.dot(e2)).clamp(0.0, 1.0);
    let s = (e0.x * e2.y - e0.y * e2.x).signum();
    let d = Vec2::new(pq0.dot(pq0), s * (v0.x * e0.y - v0.y * e0.x))
        .min(Vec2::new(pq1.dot(pq1), s * (v1.x * e1.y - v1.y * e1.x)))
        .min(Vec2::new(pq2.dot(pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    -d.x.sqrt() * d.y.signum()
}

fn sd_seg(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let (pa, ba) = (p - a, b - a);
    let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
    (pa - ba * h).length()
}

fn rot(p: Vec2, c: Vec2, angle: f32) -> Vec2 {
    let (s, co) = angle.sin_cos();
    let d = p - c;
    c + Vec2::new(d.x * co + d.y * s, -d.x * s + d.y * co)
}

impl Canvas {
    fn new() -> Self {
        Self { px: vec![[0.0; 4]; N * N] }
    }

    /// Paint a shape given as a signed distance (in 0..100 icon units).
    fn shape(&mut self, col: [f32; 4], outline: bool, sdf: impl Fn(Vec2) -> f32) {
        let scale = N as f32 / 100.0;
        for y in 0..N {
            for x in 0..N {
                let p = Vec2::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                let d = sdf(p) * scale;
                if d > 4.0 {
                    continue;
                }
                let px = &mut self.px[y * N + x];
                if outline {
                    blend(px, INK, (0.5 - (d - 2.6)).clamp(0.0, 1.0));
                }
                blend(px, col, (0.5 - d).clamp(0.0, 1.0) * col[3]);
            }
        }
    }
    fn circle(&mut self, c: (f32, f32), r: f32, col: [f32; 4]) {
        let c = Vec2::from(c);
        self.shape(col, true, |p| p.distance(c) - r);
    }
    fn dot(&mut self, c: (f32, f32), r: f32, col: [f32; 4]) {
        let c = Vec2::from(c);
        self.shape(col, false, |p| p.distance(c) - r);
    }
    fn ellipse(&mut self, c: (f32, f32), rx: f32, ry: f32, angle: f32, col: [f32; 4]) {
        let c = Vec2::from(c);
        self.shape(col, true, |p| {
            let q = rot(p, c, angle) - c;
            (Vec2::new(q.x / rx, q.y / ry).length() - 1.0) * rx.min(ry)
        });
    }
    fn rect(&mut self, c: (f32, f32), half: (f32, f32), angle: f32, col: [f32; 4]) {
        let c = Vec2::from(c);
        let h = Vec2::from(half);
        self.shape(col, true, |p| {
            let q = (rot(p, c, angle) - c).abs() - h + Vec2::splat(1.5);
            q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - 1.5
        });
    }
    fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: [f32; 4]) {
        let (a, b, c) = (Vec2::from(a), Vec2::from(b), Vec2::from(c));
        self.shape(col, true, |p| sd_tri(p, a, b, c));
    }
    fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, col: [f32; 4]) {
        let (a, b) = (Vec2::from(a), Vec2::from(b));
        self.shape(col, true, |p| sd_seg(p, a, b) - w / 2.0);
    }
    fn thin(&mut self, a: (f32, f32), b: (f32, f32), w: f32, col: [f32; 4]) {
        let (a, b) = (Vec2::from(a), Vec2::from(b));
        self.shape(col, false, |p| sd_seg(p, a, b) - w / 2.0);
    }
    fn crescent(&mut self, c: (f32, f32), r: f32, off: (f32, f32), col: [f32; 4]) {
        let (c, o) = (Vec2::from(c), Vec2::from(off));
        self.shape(col, true, |p| (p.distance(c) - r).max(-(p.distance(c + o) - r * 0.82)));
    }
    fn into_image(self) -> Image {
        let data = self
            .px
            .iter()
            .flat_map(|p| {
                let a = p[3].clamp(0.0, 1.0);
                // Stored straight (not premultiplied).
                let un = |v: f32| if a > 0.0 { (v / a).clamp(0.0, 1.0) } else { 0.0 };
                [(un(p[0]) * 255.0) as u8, (un(p[1]) * 255.0) as u8, (un(p[2]) * 255.0) as u8, (a * 255.0) as u8]
            })
            .collect();
        Image::new(
            Extent3d {
                width: N as u32,
                height: N as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
    }
}

/// Premultiplied "over" compositing.
fn blend(dst: &mut [f32; 4], col: [f32; 4], cov: f32) {
    if cov <= 0.0 {
        return;
    }
    for k in 0..3 {
        dst[k] = col[k] * cov + dst[k] * (1.0 - cov);
    }
    dst[3] = cov + dst[3] * (1.0 - cov);
}

// ------------------------------------------------------------ motifs ---

struct Pal;
#[allow(non_upper_case_globals)]
impl Pal {
    const wood: [f32; 4] = [0.69, 0.48, 0.29, 1.0];
    const wood_dark: [f32; 4] = [0.47, 0.31, 0.20, 1.0];
    const thatch: [f32; 4] = [0.84, 0.67, 0.36, 1.0];
    const sea: [f32; 4] = [0.27, 0.67, 0.78, 1.0];
    const sea_light: [f32; 4] = [0.55, 0.84, 0.90, 1.0];
    const fish: [f32; 4] = [0.60, 0.76, 0.86, 1.0];
    const belly: [f32; 4] = [0.88, 0.93, 0.95, 1.0];
    const gold: [f32; 4] = [0.95, 0.77, 0.27, 1.0];
    const gold_dark: [f32; 4] = [0.78, 0.56, 0.16, 1.0];
    const leaf: [f32; 4] = [0.38, 0.68, 0.32, 1.0];
    const leaf_dark: [f32; 4] = [0.24, 0.52, 0.26, 1.0];
    const red: [f32; 4] = [0.86, 0.38, 0.30, 1.0];
    const clay: [f32; 4] = [0.77, 0.44, 0.28, 1.0];
    const white: [f32; 4] = [0.97, 0.95, 0.90, 1.0];
    const purple: [f32; 4] = [0.45, 0.22, 0.52, 1.0];
    const green: [f32; 4] = [0.30, 0.63, 0.45, 1.0];
    const orange: [f32; 4] = [0.98, 0.56, 0.20, 1.0];
    const skin: [f32; 4] = [0.66, 0.45, 0.30, 1.0];
}

fn water(c: &mut Canvas, y: f32) {
    c.ellipse((50.0, y + 6.0), 44.0, 9.0, 0.0, Pal::sea);
    c.thin((20.0, y + 4.0), (38.0, y + 4.0), 2.5, Pal::sea_light);
    c.thin((56.0, y + 8.0), (78.0, y + 8.0), 2.5, Pal::sea_light);
}

fn fish(c: &mut Canvas, x: f32, y: f32, s: f32, angle: f32) {
    let p = |dx: f32, dy: f32| {
        let (sn, cs) = angle.sin_cos();
        (x + (dx * cs - dy * sn) * s, y + (dx * sn + dy * cs) * s)
    };
    c.tri(p(18.0, 0.0), p(34.0, -12.0), p(34.0, 12.0), Pal::fish);
    c.ellipse(p(0.0, 0.0), 22.0 * s, 12.0 * s, -angle, Pal::fish);
    c.ellipse(p(-2.0, 4.0), 14.0 * s, 5.0 * s, -angle, Pal::belly);
    c.dot(p(-12.0, -3.0), 2.6 * s, INK);
}

fn coin(c: &mut Canvas, x: f32, y: f32, r: f32) {
    c.circle((x, y), r, Pal::gold);
    c.circle((x, y), r * 0.68, Pal::gold_dark);
    c.thin((x - r * 0.18, y - r * 0.38), (x - r * 0.18, y + r * 0.38), r * 0.16, Pal::gold);
    c.ellipse((x + r * 0.02, y - r * 0.16), r * 0.24, r * 0.2, 0.0, Pal::gold);
}

fn log(c: &mut Canvas, x: f32, y: f32, len: f32, angle: f32) {
    let (s, co) = angle.sin_cos();
    let (ax, ay) = (x - co * len / 2.0, y - s * len / 2.0);
    let (bx, by) = (x + co * len / 2.0, y + s * len / 2.0);
    c.line((ax, ay), (bx, by), 18.0, Pal::wood);
    c.ellipse((bx, by), 7.0, 9.0, -angle, Pal::thatch);
    c.circle((bx, by), 3.5, Pal::wood);
}

fn leaf(c: &mut Canvas, x: f32, y: f32, len: f32, angle: f32, col: [f32; 4]) {
    let (s, co) = angle.sin_cos();
    c.ellipse((x + co * len / 2.0, y + s * len / 2.0), len / 2.0, len / 7.0, -angle, col);
    c.thin((x, y), (x + co * len, y + s * len), 1.6, Pal::leaf_dark);
}

fn drop(c: &mut Canvas, x: f32, y: f32, r: f32) {
    c.tri((x, y - r * 2.2), (x - r * 0.92, y - r * 0.3), (x + r * 0.92, y - r * 0.3), Pal::sea);
    c.circle((x, y), r, Pal::sea);
    c.dot((x - r * 0.35, y - r * 0.2), r * 0.25, Pal::sea_light);
}

fn urchin(c: &mut Canvas, x: f32, y: f32, r: f32) {
    for k in 0..14 {
        let a = k as f32 / 14.0 * std::f32::consts::TAU;
        c.line((x, y), (x + a.cos() * r * 1.7, y + a.sin() * r * 1.7), 3.0, Pal::purple);
    }
    c.circle((x, y), r, Pal::purple);
    c.dot((x - r * 0.3, y - r * 0.35), r * 0.25, [0.7, 0.5, 0.8, 1.0]);
}

fn house(c: &mut Canvas, wall: [f32; 4]) {
    water(c, 80.0);
    for x in [28.0, 50.0, 72.0] {
        c.line((x, 60.0), (x, 88.0), 4.5, Pal::wood_dark);
    }
    c.rect((50.0, 62.0), (34.0, 4.5), 0.0, Pal::wood);
    c.rect((50.0, 46.0), (22.0, 13.0), 0.0, wall);
    c.rect((57.0, 50.0), (5.0, 9.0), 0.0, Pal::wood_dark);
    c.tri((14.0, 38.0), (50.0, 10.0), (86.0, 38.0), Pal::thatch);
    c.thin((30.0, 30.0), (46.0, 18.0), 1.5, Pal::gold_dark);
    c.thin((54.0, 18.0), (70.0, 30.0), 1.5, Pal::gold_dark);
}

fn person(c: &mut Canvas, x: f32, y: f32, s: f32, cloth: [f32; 4]) {
    c.ellipse((x, y + 16.0 * s), 11.0 * s, 13.0 * s, 0.0, cloth);
    c.circle((x, y - 4.0 * s), 11.0 * s, Pal::skin);
    c.ellipse((x, y - 11.0 * s), 11.5 * s, 6.0 * s, 0.0, INK);
}

fn boat(c: &mut Canvas) {
    water(c, 76.0);
    c.line((50.0, 18.0), (50.0, 70.0), 4.0, Pal::wood_dark);
    c.tri((53.0, 20.0), (53.0, 62.0), (82.0, 62.0), Pal::red);
    c.tri((53.0, 30.0), (53.0, 50.0), (66.0, 50.0), Pal::gold);
    c.shape(Pal::wood, true, |p| {
        let q = p - Vec2::new(50.0, 72.0);
        ((q / Vec2::new(38.0, 12.0)).length() - 1.0).max(-(q.y)) * 12.0
    });
}

fn paint(name: &str) -> Canvas {
    let mut c = Canvas::new();
    match name {
        "house" | "cat_homes" => house(&mut c, Pal::red),
        "walkway" => {
            water(&mut c, 78.0);
            for i in 0..5 {
                let x = 18.0 + i as f32 * 16.0;
                let y = 70.0 - i as f32 * 10.0;
                c.line((x, y + 4.0), (x, y + 22.0), 4.0, Pal::wood_dark);
                c.rect((x, y), (8.0, 5.0), -0.55, Pal::wood);
            }
            c.thin((14.0, 58.0), (84.0, 16.0), 2.5, Pal::thatch);
        }
        "raincatcher" | "cat_water" => {
            if name == "cat_water" {
                drop(&mut c, 50.0, 60.0, 22.0);
            } else {
                drop(&mut c, 28.0, 22.0, 7.0);
                drop(&mut c, 70.0, 16.0, 6.0);
                c.ellipse((50.0, 66.0), 24.0, 22.0, 0.0, Pal::clay);
                c.rect((50.0, 42.0), (11.0, 6.0), 0.0, Pal::clay);
                c.ellipse((50.0, 37.0), 13.0, 4.0, 0.0, Pal::sea);
                c.thin((32.0, 66.0), (68.0, 66.0), 2.0, Pal::wood_dark);
            }
        }
        "fishpen" => {
            water(&mut c, 70.0);
            c.rect((50.0, 58.0), (32.0, 22.0), 0.0, [0.27, 0.67, 0.78, 0.35]);
            for i in 0..5 {
                let x = 22.0 + i as f32 * 14.0;
                c.thin((x, 38.0), (x, 80.0), 1.5, [0.2, 0.3, 0.3, 0.6]);
            }
            for x in [18.0, 82.0] {
                c.line((x, 30.0), (x, 84.0), 5.0, Pal::wood_dark);
            }
            c.line((18.0, 36.0), (82.0, 36.0), 4.0, Pal::thatch);
            fish(&mut c, 50.0, 60.0, 0.7, 0.2);
        }
        "seaweed" => {
            water(&mut c, 72.0);
            for (i, y) in [34.0, 54.0, 74.0].into_iter().enumerate() {
                c.thin((14.0, y), (86.0, y), 2.0, Pal::wood_dark);
                for k in 0..4 {
                    let x = 24.0 + k as f32 * 17.0 + i as f32 * 3.0;
                    c.ellipse((x, y), 7.0, 4.5, 0.4, if k % 2 == 0 { Pal::leaf } else { Pal::leaf_dark });
                }
                c.circle((12.0, y), 5.0, Pal::orange);
                c.circle((88.0, y), 5.0, Pal::white);
            }
        }
        "dryingrack" => {
            for x in [18.0, 82.0] {
                c.line((x, 20.0), (x, 88.0), 5.0, Pal::wood_dark);
            }
            c.line((14.0, 24.0), (86.0, 24.0), 5.0, Pal::thatch);
            for (i, x) in [32.0, 50.0, 68.0].into_iter().enumerate() {
                c.thin((x, 24.0), (x, 36.0), 1.5, INK);
                let body = [0.62 - i as f32 * 0.05, 0.44, 0.28, 1.0];
                c.tri((x - 7.0, 78.0), (x + 7.0, 78.0), (x, 66.0), body);
                c.ellipse((x, 52.0), 8.0, 17.0, 0.0, body);
                c.dot((x - 2.0, 42.0), 1.8, INK);
            }
        }
        "langgal" => {
            water(&mut c, 80.0);
            c.rect((50.0, 70.0), (38.0, 4.0), 0.0, Pal::wood);
            c.ellipse((42.0, 42.0), 17.0, 16.0, 0.0, Pal::green);
            c.rect((42.0, 56.0), (24.0, 12.0), 0.0, Pal::white);
            c.rect((42.0, 58.0), (5.0, 9.0), 0.0, Pal::wood_dark);
            c.rect((78.0, 44.0), (4.5, 24.0), 0.0, Pal::white);
            c.tri((72.0, 22.0), (84.0, 22.0), (78.0, 10.0), Pal::green);
            c.crescent((42.0, 18.0), 7.0, (3.0, -2.0), Pal::gold);
        }
        "fishing" | "cat_food" | "fish" => {
            if name == "fishing" {
                water(&mut c, 72.0);
                for k in 0..3 {
                    c.circle((20.0 + k as f32 * 30.0, 80.0), 5.0, if k % 2 == 0 { Pal::orange } else { Pal::white });
                }
                fish(&mut c, 50.0, 42.0, 1.1, -0.25);
            } else {
                fish(&mut c, 48.0, 50.0, 1.4, -0.2);
            }
        }
        "dive" | "tayum" => {
            if name == "dive" {
                water(&mut c, 20.0);
            }
            urchin(&mut c, 50.0, 60.0, 16.0);
            c.circle((78.0, 28.0), 5.0, Pal::sea_light);
            c.circle((70.0, 14.0), 3.5, Pal::sea_light);
        }
        "woodcutter" | "cat_materials" | "timber" => {
            if name == "timber" {
                log(&mut c, 44.0, 62.0, 56.0, 0.0);
                log(&mut c, 52.0, 38.0, 56.0, 0.0);
            } else {
                log(&mut c, 44.0, 70.0, 60.0, 0.0);
                c.line((58.0, 18.0), (34.0, 58.0), 6.0, Pal::wood);
                c.tri((52.0, 12.0), (74.0, 16.0), (64.0, 34.0), [0.75, 0.78, 0.82, 1.0]);
            }
        }
        "nipa" | "nipagrove" => {
            for (i, a) in [-2.3f32, -1.9, -1.5, -1.15, -0.8].into_iter().enumerate() {
                leaf(&mut c, 50.0, 86.0, 62.0, a, if i % 2 == 0 { Pal::leaf } else { Pal::leaf_dark });
            }
            c.rect((50.0, 80.0), (8.0, 4.0), 0.0, Pal::thatch);
        }
        "coin" | "sell" => coin(&mut c, 50.0, 50.0, 34.0),
        "water" => drop(&mut c, 50.0, 62.0, 22.0),
        "people" => {
            person(&mut c, 34.0, 44.0, 1.1, Pal::red);
            person(&mut c, 66.0, 50.0, 0.95, Pal::green);
        }
        "rest" => {
            c.crescent((50.0, 50.0), 30.0, (12.0, -8.0), Pal::gold);
            c.circle((78.0, 22.0), 4.0, Pal::white);
            c.circle((24.0, 80.0), 3.0, Pal::white);
        }
        "fish_food" => {
            house(&mut c, Pal::red);
            fish(&mut c, 64.0, 80.0, 0.6, 0.0);
        }
        "fish_sell" => {
            fish(&mut c, 42.0, 44.0, 1.05, -0.2);
            coin(&mut c, 72.0, 72.0, 18.0);
        }
        "build" | "repair" => {
            c.line((30.0, 84.0), (60.0, 38.0), 8.0, Pal::wood);
            c.rect((64.0, 30.0), (22.0, 9.0), -0.58, [0.62, 0.64, 0.68, 1.0]);
        }
        "fetch_water" => {
            c.ellipse((50.0, 62.0), 26.0, 24.0, 0.0, Pal::clay);
            c.rect((50.0, 36.0), (12.0, 6.0), 0.0, Pal::clay);
            drop(&mut c, 50.0, 64.0, 10.0);
        }
        "trade" => {
            c.ellipse((50.0, 66.0), 34.0, 18.0, 0.0, Pal::thatch);
            for k in 0..3 {
                c.thin((22.0 + k as f32 * 20.0, 58.0), (28.0 + k as f32 * 20.0, 80.0), 2.0, Pal::gold_dark);
            }
            fish(&mut c, 42.0, 42.0, 0.6, -0.3);
            c.ellipse((66.0, 44.0), 10.0, 6.0, 0.4, Pal::leaf);
        }
        "work" => {
            c.line((34.0, 88.0), (34.0, 12.0), 5.0, Pal::wood_dark);
            c.tri((36.0, 14.0), (84.0, 28.0), (36.0, 44.0), Pal::red);
        }
        "prayer" => {
            c.crescent((46.0, 50.0), 32.0, (14.0, -6.0), Pal::gold);
            c.circle((78.0, 36.0), 7.0, Pal::gold);
        }
        "boat" | "upgrade" => boat(&mut c),
        "leave" => {
            c.line((80.0, 50.0), (26.0, 50.0), 10.0, Pal::white);
            c.tri((14.0, 50.0), (40.0, 30.0), (40.0, 70.0), Pal::white);
        }
        "food" => fish(&mut c, 48.0, 50.0, 1.4, -0.2),
        "approval" => {
            c.circle((50.0, 50.0), 34.0, Pal::gold);
            c.dot((38.0, 42.0), 5.0, INK);
            c.dot((62.0, 42.0), 5.0, INK);
            c.shape(INK, false, |p| {
                let q = p - Vec2::new(50.0, 52.0);
                ((q.length() - 18.0).abs() - 2.5).max(-(q.y - 6.0))
            });
        }
        _ => {
            c.circle((50.0, 50.0), 30.0, Pal::white);
        }
    }
    c
}

pub const ICON_NAMES: [&str; 40] = [
    "house", "walkway", "raincatcher", "fishpen", "seaweed", "dryingrack", "langgal", "fishing", "dive", "woodcutter",
    "nipagrove", "cat_homes", "cat_food", "cat_materials", "cat_water", "coin", "timber", "nipa", "fish", "water",
    "tayum", "people", "rest", "fish_food", "fish_sell", "build", "fetch_water", "trade", "work", "prayer", "boat",
    "upgrade", "sell", "repair", "leave", "food", "approval", "wood", "wood2", "blank",
];

fn paint_icons(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut map = HashMap::new();
    for name in ICON_NAMES {
        map.insert(name, images.add(paint(name).into_image()));
    }
    commands.insert_resource(Icons(map));
}
