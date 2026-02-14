use macroquad::prelude::*;
use macroquad::file::load_string;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use crate::helpers::{asset_path, data_path, load_wasm_manifest_files};

#[derive(Debug)]
pub enum ParticleLoadError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    Texture(String),
}

impl std::fmt::Display for ParticleLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Yaml(err) => write!(f, "yaml error: {err}"),
            Self::Texture(err) => write!(f, "texture error: {err}"),
        }
    }
}

impl std::error::Error for ParticleLoadError {}

impl From<std::io::Error> for ParticleLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_yaml::Error> for ParticleLoadError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Yaml(err)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticleShape {
    Circle,
    Quad,
    Texture,
}

#[derive(Clone)]
pub struct ParticleConfig {
    pub id: String,
    pub max_particles: usize,
    pub spawn_rate: f32,
    pub trail_rate: f32,
    pub burst: u32,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub speed: f32,
    pub speed_variance: f32,
    pub angle: f32,
    pub angle_variance: f32,
    pub gravity: Vec2,
    pub damping: f32,
    pub size_start: f32,
    pub size_end: f32,
    pub color_start: Color,
    pub color_end: Color,
    pub shape: ParticleShape,
    pub inherit_velocity: f32,
    pub rotation: f32,
    pub rotation_variance: f32,
    pub rotation_speed: f32,
    pub rotation_speed_variance: f32,
    pub dynamic_sprite: bool,
}

#[derive(Clone)]
struct ParticleTemplate {
    config: ParticleConfig,
    texture: Option<Texture2D>,
}

#[derive(Clone)]
struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    life_max: f32,
    size_start: f32,
    size_end: f32,
    color_start: Color,
    color_end: Color,
    rotation: f32,
    rotation_speed: f32,
    template: usize,
    texture: Option<Texture2D>,
    dest_size: Option<Vec2>,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            life: 0.0,
            life_max: 1.0,
            size_start: 1.0,
            size_end: 0.0,
            color_start: WHITE,
            color_end: Color::new(1.0, 1.0, 1.0, 0.0),
            rotation: 0.0,
            rotation_speed: 0.0,
            template: 0,
            texture: None,
            dest_size: None,
        }
    }
}

struct ParticlePool {
    particles: Vec<Particle>,
    free: Vec<usize>,
    active: Vec<usize>,
}

impl ParticlePool {
    fn new(capacity: usize) -> Self {
        let mut free = Vec::with_capacity(capacity);
        for i in (0..capacity).rev() {
            free.push(i);
        }
        Self {
            particles: vec![Particle::default(); capacity],
            free,
            active: Vec::with_capacity(capacity),
        }
    }

    fn spawn(&mut self, particle: Particle) -> bool {
        if let Some(idx) = self.free.pop() {
            self.particles[idx] = particle;
            self.active.push(idx);
            true
        } else {
            false
        }
    }

    fn update(&mut self, dt: f32, templates: &[ParticleTemplate], counts: &mut [usize]) {
        let mut i = 0;
        while i < self.active.len() {
            let idx = self.active[i];
            let template = &templates[self.particles[idx].template];
            let cfg = &template.config;
            let particle = &mut self.particles[idx];

            particle.life -= dt;
            if particle.life <= 0.0 {
                let template = particle.template;
                if let Some(count) = counts.get_mut(template) {
                    if *count > 0 {
                        *count -= 1;
                    }
                }
                self.free.push(idx);
                self.active.swap_remove(i);
                continue;
            }

            particle.vel += cfg.gravity * dt;
            if cfg.damping != 1.0 {
                let damp = cfg.damping.clamp(0.0, 1.0).powf(dt.max(0.0));
                particle.vel *= damp;
            }
            particle.pos += particle.vel * dt;
            particle.rotation += particle.rotation_speed * dt;

            i += 1;
        }
    }

    fn draw(&self, templates: &[ParticleTemplate]) {
        for &idx in &self.active {
            let particle = &self.particles[idx];
            let template = &templates[particle.template];
            let cfg = &template.config;

            let t = 1.0 - (particle.life / particle.life_max).clamp(0.0, 1.0);
            let size = particle.size_start + (particle.size_end - particle.size_start) * t;
            let color = lerp_color(particle.color_start, particle.color_end, t);

            match cfg.shape {
                ParticleShape::Circle => {
                    draw_circle(particle.pos.x, particle.pos.y, size.max(0.0), color);
                }
                ParticleShape::Quad => {
                    let half = size * 0.5;
                    draw_rectangle(
                        particle.pos.x - half,
                        particle.pos.y - half,
                        size.max(0.0),
                        size.max(0.0),
                        color,
                    );
                }
                ParticleShape::Texture => {
                    let tex = particle.texture.as_ref().or(template.texture.as_ref());
                    if let Some(tex) = tex {
                        let base_dest = particle
                            .dest_size
                            .unwrap_or_else(|| vec2(tex.width(), tex.height()));
                        let dest = base_dest * size;
                        draw_texture_ex(
                            tex,
                            particle.pos.x - dest.x * 0.5,
                            particle.pos.y - dest.y * 0.5,
                            color,
                            DrawTextureParams {
                                dest_size: Some(dest),
                                rotation: particle.rotation,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }

    fn draw_in_rect(&self, templates: &[ParticleTemplate], rect: Rect) {
        for &idx in &self.active {
            let particle = &self.particles[idx];
            let template = &templates[particle.template];
            let cfg = &template.config;

            let t = 1.0 - (particle.life / particle.life_max).clamp(0.0, 1.0);
            let size = particle.size_start + (particle.size_end - particle.size_start) * t;

            let mut radius = match cfg.shape {
                ParticleShape::Circle => size,
                ParticleShape::Quad => size * 0.5,
                ParticleShape::Texture => {
                    let tex = particle.texture.as_ref().or(template.texture.as_ref());
                    let base = particle.dest_size.unwrap_or_else(|| {
                        tex.map(|t| vec2(t.width(), t.height()))
                            .unwrap_or(vec2(size, size))
                    });
                    base.x.max(base.y) * size * 0.5
                }
            };
            if radius.is_nan() || radius < 0.0 {
                radius = 0.0;
            }

            if particle.pos.x + radius < rect.x
                || particle.pos.y + radius < rect.y
                || particle.pos.x - radius > rect.x + rect.w
                || particle.pos.y - radius > rect.y + rect.h
            {
                continue;
            }

            let color = lerp_color(particle.color_start, particle.color_end, t);

            match cfg.shape {
                ParticleShape::Circle => {
                    draw_circle(particle.pos.x, particle.pos.y, size.max(0.0), color);
                }
                ParticleShape::Quad => {
                    let half = size * 0.5;
                    draw_rectangle(
                        particle.pos.x - half,
                        particle.pos.y - half,
                        size.max(0.0),
                        size.max(0.0),
                        color,
                    );
                }
                ParticleShape::Texture => {
                    let tex = particle.texture.as_ref().or(template.texture.as_ref());
                    if let Some(tex) = tex {
                        let base_dest = particle
                            .dest_size
                            .unwrap_or_else(|| vec2(tex.width(), tex.height()));
                        let dest = base_dest * size;
                        draw_texture_ex(
                            tex,
                            particle.pos.x - dest.x * 0.5,
                            particle.pos.y - dest.y * 0.5,
                            color,
                            DrawTextureParams {
                                dest_size: Some(dest),
                                rotation: particle.rotation,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }
}

pub struct ParticleEmitter {
    template: usize,
    spawn_accum: f32,
    trail_accum: f32,
    last_pos: Vec2,
    first: bool,
    burst_done: bool,
}

impl ParticleEmitter {
    fn new(template: usize, pos: Vec2) -> Self {
        Self {
            template,
            spawn_accum: 0.0,
            trail_accum: 0.0,
            last_pos: pos,
            first: true,
            burst_done: false,
        }
    }
}

pub struct ParticleSystem {
    templates: Vec<ParticleTemplate>,
    lookup: HashMap<String, usize>,
    pool: ParticlePool,
    template_counts: Vec<usize>,
    budget_scale: f32,
}

impl ParticleSystem {
    pub fn empty() -> Self {
        Self {
            templates: Vec::new(),
            lookup: HashMap::new(),
            pool: ParticlePool::new(1),
            template_counts: vec![0],
            budget_scale: 1.0,
        }
    }

    pub async fn load_from(dir: impl AsRef<Path>) -> Result<Self, ParticleLoadError> {
        let dir = dir.as_ref();
        let mut templates = Vec::new();
        let mut lookup = HashMap::new();
        let mut total_capacity = 0usize;

        if cfg!(target_arch = "wasm32") {
            let dir = data_path(&dir.to_string_lossy());
            let files = load_wasm_manifest_files(&dir, &["trail.yaml", "dash.yaml"]).await;
            for file in files {
                let path = format!("{}/{}", dir, file);
                let raw_str = load_string(&path)
                    .await
                    .map_err(|err| ParticleLoadError::Io(std::io::Error::new(std::io::ErrorKind::Other, err.to_string())))?;
                let raw: ParticleConfigFile = serde_yaml::from_str(&raw_str)?;
                let (config, texture_path) = config_from_file(raw);
                total_capacity = total_capacity.saturating_add(config.max_particles);

                let texture = if let Some(path) = texture_path {
                    let tex = load_texture(&asset_path(&path))
                        .await
                        .map_err(|err| ParticleLoadError::Texture(err.to_string()))?;
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                } else {
                    None
                };

                lookup.insert(config.id.clone(), templates.len());
                templates.push(ParticleTemplate { config, texture });
            }
        } else if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if !is_yaml(&path) {
                    continue;
                }
                let raw: ParticleConfigFile = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
                let (config, texture_path) = config_from_file(raw);
                total_capacity = total_capacity.saturating_add(config.max_particles);

                let texture = if let Some(path) = texture_path {
                    let tex = load_texture(&asset_path(&path))
                        .await
                        .map_err(|err| ParticleLoadError::Texture(err.to_string()))?;
                    tex.set_filter(FilterMode::Nearest);
                    Some(tex)
                } else {
                    None
                };

                lookup.insert(config.id.clone(), templates.len());
                templates.push(ParticleTemplate { config, texture });
            }
        }

        if total_capacity == 0 {
            total_capacity = 1;
        }

        let template_count = templates.len();
        Ok(Self {
            templates,
            lookup,
            pool: ParticlePool::new(total_capacity),
            template_counts: vec![0; template_count],
            budget_scale: 1.0,
        })
    }

    pub fn emitter(&self, id: &str, pos: Vec2) -> Option<ParticleEmitter> {
        let idx = self.lookup.get(id).copied()?;
        Some(ParticleEmitter::new(idx, pos))
    }

    pub fn update_emitter(&mut self, emitter: &mut ParticleEmitter, pos: Vec2, dt: f32) {
        self.update_emitter_with_texture(emitter, pos, dt, None, None);
    }

    pub fn update_emitter_with_texture(
        &mut self,
        emitter: &mut ParticleEmitter,
        pos: Vec2,
        dt: f32,
        texture: Option<&Texture2D>,
        dest_size: Option<Vec2>,
    ) {
        let cfg = self.templates[emitter.template].config.clone();

        if emitter.first {
            emitter.last_pos = pos;
            emitter.first = false;
        }

        if !emitter.burst_done && cfg.burst > 0 {
            for _ in 0..cfg.burst {
                self.spawn_particle(emitter.template, pos, Vec2::ZERO, texture, dest_size);
            }
            emitter.burst_done = true;
        }

        // Rate-based spawn
        if cfg.spawn_rate > 0.0 {
            emitter.spawn_accum += cfg.spawn_rate * dt;
            let count = emitter.spawn_accum.floor() as u32;
            emitter.spawn_accum -= count as f32;
            for _ in 0..count {
                self.spawn_particle(
                    emitter.template,
                    pos,
                    (pos - emitter.last_pos) / dt.max(0.0001),
                    texture,
                    dest_size,
                );
            }
        }

        // Trail-based spawn (per unit distance)
        if cfg.trail_rate > 0.0 {
            let dist = pos.distance(emitter.last_pos);
            let total = dist * cfg.trail_rate + emitter.trail_accum;
            let count = total.floor() as u32;
            emitter.trail_accum = total - count as f32;
            if count > 0 {
                let dir = pos - emitter.last_pos;
                for i in 0..count {
                    let t = (i + 1) as f32 / count as f32;
                    let spawn_pos = emitter.last_pos + dir * t;
                    self.spawn_particle(
                        emitter.template,
                        spawn_pos,
                        dir / dt.max(0.0001),
                        texture,
                        dest_size,
                    );
                }
            }
        }

        emitter.last_pos = pos;
    }

    pub fn track_emitter(&mut self, emitter: &mut ParticleEmitter, pos: Vec2) {
        emitter.last_pos = pos;
        emitter.first = false;
        emitter.spawn_accum = 0.0;
        emitter.trail_accum = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        self.pool
            .update(dt, &self.templates, &mut self.template_counts);
    }

    pub fn draw(&self) {
        self.pool.draw(&self.templates);
    }

    pub fn draw_in_rect(&self, rect: Rect) {
        self.pool.draw_in_rect(&self.templates, rect);
    }

    pub fn set_budget_scale(&mut self, scale: f32) {
        self.budget_scale = scale.clamp(0.1, 1.0);
    }

    fn spawn_particle(
        &mut self,
        template: usize,
        pos: Vec2,
        emitter_vel: Vec2,
        override_texture: Option<&Texture2D>,
        override_dest_size: Option<Vec2>,
    ) {
        let cfg = &self.templates[template].config;
        let max_particles = ((cfg.max_particles as f32) * self.budget_scale)
            .round()
            .max(1.0) as usize;
        if self.template_counts[template] >= max_particles {
            return;
        }

        let life = (cfg.lifetime + rand_range(cfg.lifetime_variance)).max(0.01);
        let speed = cfg.speed + rand_range(cfg.speed_variance);
        let angle = (cfg.angle + rand_range(cfg.angle_variance)).to_radians();
        let dir = vec2(angle.cos(), angle.sin());
        let mut vel = dir * speed;
        if cfg.inherit_velocity != 0.0 {
            vel += emitter_vel * cfg.inherit_velocity;
        }

        let rotation = cfg.rotation + rand_range(cfg.rotation_variance);
        let rotation_speed = cfg.rotation_speed + rand_range(cfg.rotation_speed_variance);
        let texture = if cfg.dynamic_sprite {
            override_texture.map(|tex| tex.weak_clone())
        } else {
            None
        };
        let dest_size = if cfg.dynamic_sprite {
            override_dest_size
        } else {
            None
        };

        let spawned = self.pool.spawn(Particle {
            pos,
            vel,
            life,
            life_max: life,
            size_start: cfg.size_start,
            size_end: cfg.size_end,
            color_start: cfg.color_start,
            color_end: cfg.color_end,
            rotation,
            rotation_speed,
            template,
            texture,
            dest_size,
        });
        if spawned {
            self.template_counts[template] += 1;
        }
    }
}

fn rand_range(amount: f32) -> f32 {
    if amount == 0.0 {
        0.0
    } else {
        macroquad::rand::gen_range(-amount, amount)
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}

fn config_from_file(raw: ParticleConfigFile) -> (ParticleConfig, Option<String>) {
    let max_particles = raw.max_particles.unwrap_or(512);
    let spawn_rate = raw.spawn_rate.unwrap_or(0.0);
    let trail_rate = raw.trail_rate.unwrap_or(0.0);
    let burst = raw.burst.unwrap_or(0);
    let lifetime = raw.lifetime.unwrap_or(1.0);
    let lifetime_variance = raw.lifetime_variance.unwrap_or(0.0);
    let speed = raw.speed.unwrap_or(0.0);
    let speed_variance = raw.speed_variance.unwrap_or(0.0);
    let angle = raw.angle.unwrap_or(0.0);
    let angle_variance = raw.angle_variance.unwrap_or(360.0);
    let gravity = raw.gravity.unwrap_or([0.0, 0.0]);
    let damping = raw.damping.unwrap_or(1.0);
    let size_start = raw.size_start.unwrap_or(4.0);
    let size_end = raw.size_end.unwrap_or(0.0);
    let color_start = raw.color_start.unwrap_or([255, 255, 255, 255]);
    let color_end = raw.color_end.unwrap_or([255, 255, 255, 0]);
    let inherit_velocity = raw.inherit_velocity.unwrap_or(0.0);
    let rotation = raw.rotation.unwrap_or(0.0);
    let rotation_variance = raw.rotation_variance.unwrap_or(0.0);
    let rotation_speed = raw.rotation_speed.unwrap_or(0.0);
    let rotation_speed_variance = raw.rotation_speed_variance.unwrap_or(0.0);
    let dynamic_sprite = raw.dynamic_sprite.unwrap_or(false);

    let shape = raw
        .shape
        .unwrap_or_else(|| {
            if raw.texture.is_some() || dynamic_sprite {
                ParticleShape::Texture
            } else {
                ParticleShape::Circle
            }
        });

    let config = ParticleConfig {
        id: raw.id,
        max_particles,
        spawn_rate,
        trail_rate,
        burst,
        lifetime,
        lifetime_variance,
        speed,
        speed_variance,
        angle,
        angle_variance,
        gravity: vec2(gravity[0], gravity[1]),
        damping,
        size_start,
        size_end,
        color_start: Color::from_rgba(color_start[0], color_start[1], color_start[2], color_start[3]),
        color_end: Color::from_rgba(color_end[0], color_end[1], color_end[2], color_end[3]),
        shape,
        inherit_velocity,
        rotation,
        rotation_variance,
        rotation_speed,
        rotation_speed_variance,
        dynamic_sprite,
    };

    let texture = raw.texture.map(|path| asset_path(&path));
    (config, texture)
}

#[derive(Deserialize)]
struct ParticleConfigFile {
    id: String,
    #[serde(default)]
    max_particles: Option<usize>,
    #[serde(default)]
    spawn_rate: Option<f32>,
    #[serde(default)]
    trail_rate: Option<f32>,
    #[serde(default)]
    burst: Option<u32>,
    #[serde(default)]
    lifetime: Option<f32>,
    #[serde(default)]
    lifetime_variance: Option<f32>,
    #[serde(default)]
    speed: Option<f32>,
    #[serde(default)]
    speed_variance: Option<f32>,
    #[serde(default)]
    angle: Option<f32>,
    #[serde(default)]
    angle_variance: Option<f32>,
    #[serde(default)]
    gravity: Option<[f32; 2]>,
    #[serde(default)]
    damping: Option<f32>,
    #[serde(default)]
    size_start: Option<f32>,
    #[serde(default)]
    size_end: Option<f32>,
    #[serde(default)]
    color_start: Option<[u8; 4]>,
    #[serde(default)]
    color_end: Option<[u8; 4]>,
    #[serde(default)]
    shape: Option<ParticleShape>,
    #[serde(default)]
    texture: Option<String>,
    #[serde(default)]
    inherit_velocity: Option<f32>,
    #[serde(default)]
    rotation: Option<f32>,
    #[serde(default)]
    rotation_variance: Option<f32>,
    #[serde(default)]
    rotation_speed: Option<f32>,
    #[serde(default)]
    rotation_speed_variance: Option<f32>,
    #[serde(default)]
    dynamic_sprite: Option<bool>,
}
