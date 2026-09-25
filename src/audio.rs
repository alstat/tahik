//! Nature sounds only - no music. Every sound (waves, wind, rain, thunder,
//! splashes, bubbles, wood knocks) is synthesised at startup into WAV data.

use bevy::audio::Volume;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use crate::common::*;
use crate::weather::Weather;

const SR: u32 = 22_050;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Ambient {
    Waves,
    Wind,
    Rain,
}

#[derive(Resource)]
struct SfxHandles {
    splash: Handle<AudioSource>,
    thunder: Handle<AudioSource>,
    knock: Handle<AudioSource>,
    bubbles: Handle<AudioSource>,
    thud: Handle<AudioSource>,
    paddle: Handle<AudioSource>,
}

pub struct NatureAudioPlugin;

impl Plugin for NatureAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_audio)
            .add_systems(Update, (mix_ambience, play_sfx));
    }
}

// ------------------------------------------------------------ synth ---

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

struct Lowpass {
    a: f32,
    y: f32,
}

impl Lowpass {
    fn new(fc: f32) -> Self {
        Self {
            a: 1.0 - (-TAU * fc / SR as f32).exp(),
            y: 0.0,
        }
    }
    fn run(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

/// Chamberlin state-variable band-pass.
struct Bandpass {
    low: f32,
    band: f32,
    q: f32,
}

impl Bandpass {
    fn new(q: f32) -> Self {
        Self { low: 0.0, band: 0.0, q: 1.0 / q }
    }
    fn run(&mut self, x: f32, fc: f32) -> f32 {
        let f = 2.0 * (PI * fc / SR as f32).sin();
        self.low += f * self.band;
        let high = x - self.low - self.q * self.band;
        self.band += f * high;
        self.band
    }
}

fn normalize(s: &mut [f32], peak: f32) {
    let m = s.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    for v in s.iter_mut() {
        *v *= peak / m;
    }
}

/// Generate `secs` of audio plus a tail, then crossfade the tail into the
/// head so the clip loops seamlessly.
fn seamless(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * SR as f32) as usize;
    let fade = SR as usize;
    let raw: Vec<f32> = (0..n + fade).map(|i| f(i as f32 / SR as f32)).collect();
    let mut out = raw[..n].to_vec();
    for i in 0..fade {
        let t = i as f32 / fade as f32;
        out[i] = raw[i] * t + raw[n + i] * (1.0 - t);
    }
    out
}

pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SR.to_le_bytes());
    b.extend_from_slice(&(SR * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

fn waves_sound() -> Vec<f32> {
    let mut n = Noise(0x1234_5678);
    let mut body = Lowpass::new(380.0);
    let mut body2 = Lowpass::new(380.0);
    let mut wash = Lowpass::new(3500.0);
    let mut wash_hp = Lowpass::new(500.0);
    let crashes = [0.2, 3.3, 6.1, 9.4, 12.0];
    let mut s = seamless(12.0, |t| {
        let mut env = 0.3;
        for &c in &crashes {
            let dt = t - c;
            if dt > -1.4 && dt < 0.0 {
                env += smoothstep(-1.4, 0.0, dt) * 0.7;
            } else if dt >= 0.0 {
                env += 0.7 * (-dt / 1.6).exp();
            }
        }
        let x = n.next();
        let low = body2.run(body.run(x)) * 3.0;
        let w = wash.run(x);
        let fizz = w - wash_hp.run(w);
        low * (0.45 + 0.55 * env) + fizz * env * env * 0.35
    });
    normalize(&mut s, 0.8);
    s
}

fn wind_sound() -> Vec<f32> {
    let mut n = Noise(0x0bad_cafe);
    let mut bp = Bandpass::new(5.0);
    let mut bp2 = Bandpass::new(9.0);
    let mut rumble = Lowpass::new(140.0);
    let mut gust = Lowpass::new(0.4);
    let mut gn = Noise(77);
    let mut s = seamless(10.0, |t| {
        let x = n.next();
        let fc = 420.0 + 240.0 * (TAU * t / 5.0).sin() + 130.0 * (TAU * t / 3.3 + 1.0).sin();
        let g = 0.55 + 4.0 * gust.run(gn.next()).abs();
        (bp.run(x, fc) * 0.6 + bp2.run(x, fc * 2.1) * 0.25 + rumble.run(x) * 2.0) * g
    });
    normalize(&mut s, 0.7);
    s
}

fn rain_sound() -> Vec<f32> {
    let mut n = Noise(0x5eed_1234);
    let mut lp = Lowpass::new(5500.0);
    let mut hp = Lowpass::new(900.0);
    let mut drop_env = 0.0f32;
    let mut drop_amp = 0.0f32;
    let mut dn = Noise(99);
    let mut s = seamless(8.0, |_| {
        let x = n.next();
        let hiss = lp.run(x);
        let hiss = hiss - hp.run(hiss);
        if dn.next() > 0.9965 {
            drop_env = 1.0;
            drop_amp = 0.2 + 0.4 * dn.next().abs();
        }
        drop_env *= 0.992;
        hiss * 0.35 + x * drop_env * drop_amp
    });
    normalize(&mut s, 0.6);
    s
}

fn thunder_sound() -> Vec<f32> {
    let mut n = Noise(4242);
    let mut lp = Lowpass::new(110.0);
    let mut lp2 = Lowpass::new(110.0);
    let mut crack_lp = Lowpass::new(2500.0);
    let mut modu = Lowpass::new(3.0);
    let mut mn = Noise(17);
    let len = (6.0 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let x = n.next();
            let crack = crack_lp.run(x) * (-t * 18.0).exp() * 0.8;
            let env = smoothstep(0.0, 0.15, t) * (-t / 1.7).exp();
            let m = 0.5 + 6.0 * modu.run(mn.next()).abs();
            crack + lp2.run(lp.run(x)) * 14.0 * env * m
        })
        .collect();
    normalize(&mut s, 0.95);
    s
}

fn splash_sound() -> Vec<f32> {
    let mut n = Noise(31337);
    let mut bp = Bandpass::new(1.5);
    let len = (0.9 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let fc = 600.0 + 2600.0 * (-t * 5.0).exp();
            let plop_f = 90.0 + 120.0 * (-t * 20.0).exp();
            let plop = (TAU * plop_f * t).sin() * (-t * 22.0).exp() * 0.6;
            bp.run(n.next(), fc) * (-t * 5.5).exp() * smoothstep(0.0, 0.02, t) + plop
        })
        .collect();
    normalize(&mut s, 0.8);
    s
}

fn knock_sound() -> Vec<f32> {
    let mut n = Noise(555);
    let mut lp = Lowpass::new(1800.0);
    let hits = [0.0, 0.27, 0.52];
    let len = (0.9 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let x = lp.run(n.next());
            hits.iter()
                .filter(|&&h| t >= h)
                .map(|&h| {
                    let d = t - h;
                    (TAU * 210.0 * d).sin() * (-d * 35.0).exp() + (TAU * 480.0 * d).sin() * (-d * 60.0).exp() * 0.4 + x * (-d * 160.0).exp()
                })
                .sum()
        })
        .collect();
    normalize(&mut s, 0.7);
    s
}

fn thud_sound() -> Vec<f32> {
    let mut n = Noise(777);
    let mut lp = Lowpass::new(300.0);
    let len = (0.5 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            (TAU * 85.0 * t).sin() * (-t * 12.0).exp() + lp.run(n.next()) * 2.0 * (-t * 20.0).exp()
        })
        .collect();
    normalize(&mut s, 0.8);
    s
}

/// A paddle blade slipping into the water: a soft swish and a small plunk.
fn paddle_sound() -> Vec<f32> {
    let mut n = Noise(9091);
    let mut bp = Bandpass::new(2.0);
    let len = (0.5 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let fc = 500.0 + 1400.0 * (-t * 7.0).exp();
            let swish = bp.run(n.next(), fc) * smoothstep(0.0, 0.04, t) * (-t * 7.0).exp();
            let plunk_f = 140.0 + 160.0 * (-t * 30.0).exp();
            let plunk = (TAU * plunk_f * t).sin() * (-t * 30.0).exp() * 0.25;
            swish + plunk
        })
        .collect();
    normalize(&mut s, 0.6);
    s
}

fn bubbles_sound() -> Vec<f32> {
    let mut r = Rng::new(2024);
    let bubbles: Vec<(f32, f32)> = (0..16).map(|_| (r.range(0.0, 1.8), r.range(350.0, 950.0))).collect();
    let len = (2.2 * SR as f32) as usize;
    let mut s: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            bubbles
                .iter()
                .filter(|(start, _)| t >= *start)
                .map(|(start, f0)| {
                    let d = t - start;
                    let f = f0 * (1.0 + d * 12.0);
                    (TAU * f * d).sin() * (-d * 28.0).exp()
                })
                .sum()
        })
        .collect();
    normalize(&mut s, 0.6);
    s
}

/// A distant call to prayer: a chanted, melismatic line on an "aa" vowel in a
/// Hijaz-flavoured mode, softened by distance and echoing over the water.
/// (Drop a real recording at `assets/azan.ogg` to use that instead.)
pub fn azan_sound() -> Vec<f32> {
    // (semitones above the base note, seconds); None = breath between phrases.
    let phrases: [&[(f32, f32)]; 4] = [
        &[(0.0, 0.32), (0.0, 0.3), (1.0, 0.28), (4.0, 1.4), (5.0, 0.3), (4.0, 0.3), (1.0, 0.35), (0.0, 1.5)],
        &[(4.0, 0.3), (5.0, 0.3), (7.0, 0.3), (8.0, 1.5), (7.0, 0.35), (5.0, 0.3), (4.0, 1.6)],
        &[(4.0, 0.3), (5.0, 0.3), (7.0, 0.8), (8.0, 0.3), (10.0, 0.9), (8.0, 0.3), (7.0, 0.3), (5.0, 0.3), (4.0, 0.4), (1.0, 0.4), (0.0, 1.7)],
        &[(0.0, 0.3), (1.0, 0.3), (4.0, 0.3), (5.0, 1.3), (4.0, 0.4), (1.0, 0.4), (0.0, 2.2)],
    ];
    let base = 196.0f32; // G3
    // Flatten into (start, end, semitone, phrase_end) segments.
    let mut segs: Vec<(f32, f32, f32)> = Vec::new();
    let mut t = 0.4;
    for p in phrases {
        for &(semi, dur) in p {
            segs.push((t, t + dur, semi));
            t += dur;
        }
        t += 1.3;
    }
    let total = t + 2.5;
    let n = (total * SR as f32) as usize;
    let formants = [(730.0f32, 90.0f32, 1.0f32), (1090.0, 110.0, 0.55), (2440.0, 160.0, 0.25)];
    let mut phases = [0.0f32; 24];
    let mut semi_smooth = 0.0f32;
    let mut noise = Noise(8080);
    let mut out = vec![0.0f32; n];
    let mut seg_i = 0;
    for (i, sample) in out.iter_mut().enumerate() {
        let time = i as f32 / SR as f32;
        while seg_i + 1 < segs.len() && time >= segs[seg_i].1 {
            seg_i += 1;
        }
        let (s0, s1, semi) = segs[seg_i];
        let inside = time >= s0 && time < s1;
        // Voiced only inside a note; short dips between syllables.
        let env = if inside {
            smoothstep(s0, s0 + 0.04, time) * (1.0 - smoothstep(s1 - 0.05, s1, time) * 0.7)
        } else {
            0.0
        };
        semi_smooth += (semi - semi_smooth) * (1.0 - (-1.0 / (0.045 * SR as f32)).exp());
        let long = (s1 - s0) > 0.7;
        let vib = if long { (TAU * 5.3 * time).sin() * 0.28 * smoothstep(s0 + 0.2, s0 + 0.6, time) } else { 0.0 };
        // A little melisma on the long notes.
        let turn = if long { ((time - s0) * 3.1).sin() * 0.35 * smoothstep(s0 + 0.3, s0 + 0.8, time) } else { 0.0 };
        let f0 = base * 2f32.powf((semi_smooth + vib + turn) / 12.0);
        let mut v = 0.0;
        if env > 0.0 {
            for (h, phase) in phases.iter_mut().enumerate() {
                let fh = f0 * (h + 1) as f32;
                if fh > 5000.0 {
                    break;
                }
                let mut amp = 0.08 / (h + 1) as f32;
                for (fc, bw, g) in formants {
                    amp += g * (-((fh - fc) / bw).powi(2)).exp() * 0.5 / ((h + 1) as f32).sqrt();
                }
                *phase = (*phase + fh / SR as f32).fract();
                v += (*phase * TAU).sin() * amp;
            }
            v += noise.next() * 0.01;
        }
        *sample = v * env;
    }
    // Distance: soften the highs, then echo off the water and the houses.
    let mut lp = Lowpass::new(2600.0);
    for v in out.iter_mut() {
        *v = lp.run(*v);
    }
    let delays = [0.043f32, 0.067, 0.097, 0.131];
    let dry = out.clone();
    for (k, d) in delays.iter().enumerate() {
        let dn = (d * SR as f32) as usize;
        let fb = 0.62 - k as f32 * 0.05;
        let mut buf = vec![0.0f32; n];
        for i in 0..n {
            let back = if i >= dn { buf[i - dn] } else { 0.0 };
            buf[i] = dry[i] + back * fb;
        }
        for i in 0..n {
            out[i] += buf[i] * 0.22;
        }
    }
    normalize(&mut out, 0.7);
    out
}

// ----------------------------------------------------------- bevy ---

fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| {
        sources.add(AudioSource {
            bytes: Arc::from(wav(&samples)),
        })
    };
    let waves = add(waves_sound());
    let wind = add(wind_sound());
    let rain = add(rain_sound());
    let handles = SfxHandles {
        splash: add(splash_sound()),
        thunder: add(thunder_sound()),
        knock: add(knock_sound()),
        bubbles: add(bubbles_sound()),
        thud: add(thud_sound()),
        paddle: add(paddle_sound()),
    };
    for (h, kind, v) in [(waves, Ambient::Waves, 0.4), (wind, Ambient::Wind, 0.2), (rain, Ambient::Rain, 0.0)] {
        commands.spawn((
            AudioPlayer(h),
            PlaybackSettings {
                volume: Volume::Linear(v),
                ..PlaybackSettings::LOOP
            },
            kind,
        ));
    }
    commands.insert_resource(handles);
}

fn mix_ambience(time: Res<Time>, weather: Res<Weather>, mut sinks: Query<(&Ambient, &mut AudioSink)>) {
    let dt = time.delta_secs();
    for (kind, mut sink) in &mut sinks {
        let target = match kind {
            Ambient::Waves => 0.35 + 0.5 * weather.storm,
            Ambient::Wind => 0.06 + 0.22 * weather.wind_strength + 0.5 * weather.storm,
            Ambient::Rain => weather.rain * 0.8,
        };
        let cur = sink.volume().to_linear();
        let v = cur + (target - cur) * (1.0 - (-dt * 2.0).exp());
        sink.set_volume(Volume::Linear(v));
        if *kind == Ambient::Wind {
            sink.set_speed(0.9 + 0.35 * weather.storm);
        }
    }
}

fn play_sfx(mut commands: Commands, time: Res<Time>, mut queue: ResMut<SfxQueue>, handles: Option<Res<SfxHandles>>) {
    let Some(h) = handles else { return };
    let dt = time.delta_secs();
    let mut ready = Vec::new();
    queue.0.retain_mut(|(sfx, delay, vol)| {
        *delay -= dt;
        if *delay <= 0.0 {
            ready.push((*sfx, *vol));
            false
        } else {
            true
        }
    });
    for (sfx, vol) in ready {
        let handle = match sfx {
            Sfx::Splash => h.splash.clone(),
            Sfx::Thunder => h.thunder.clone(),
            Sfx::Knock => h.knock.clone(),
            Sfx::Bubbles => h.bubbles.clone(),
            Sfx::Thud => h.thud.clone(),
            Sfx::Paddle => h.paddle.clone(),
        };
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                volume: Volume::Linear(vol),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}
