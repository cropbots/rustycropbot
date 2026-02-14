use macroquad::prelude::*;
use macroquad::file::load_string;
use serde::Deserialize;

pub fn random_u32() -> u32 {
    macroquad::rand::rand()
}

pub fn random_f32() -> f32 {
    let v = random_u32();
    v as f32 / u32::MAX as f32
}

pub fn random_range(min: f32, max: f32) -> f32 {
    if max <= min {
        return min;
    }
    min + (max - min) * random_f32()
}

pub async fn load_single_texture(dir: &str, name: &str) -> Option<Texture2D> {
    let dir = asset_path(dir);
    let tile_path = format!("{}/{}.png", dir, name);
    load_texture(&tile_path).await.ok()
}

pub fn asset_root() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "assets"
    } else {
        "src/assets"
    }
}

pub fn asset_path(path: &str) -> String {
    if cfg!(target_arch = "wasm32") {
        if let Some(stripped) = path.strip_prefix("src/assets/") {
            return format!("{}/{}", asset_root(), stripped);
        }
    }
    path.to_string()
}

pub fn data_root() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "assets"
    } else {
        "src"
    }
}

pub fn data_path(path: &str) -> String {
    if cfg!(target_arch = "wasm32") {
        if let Some(stripped) = path.strip_prefix("src/") {
            return format!("{}/{}", data_root(), stripped);
        }
    }
    path.to_string()
}

pub fn asset_dir(subdir: &str) -> String {
    format!("{}/{}", asset_root(), subdir.trim_start_matches('/'))
}

#[derive(Deserialize)]
struct WasmIndexFile {
    files: Vec<String>,
}

pub async fn load_wasm_manifest_files(dir: &str, fallback: &[&str]) -> Vec<String> {
    let index_path = format!("{}/index.json", dir.trim_end_matches('/'));
    if let Ok(raw) = load_string(&index_path).await {
        if let Ok(parsed) = serde_json::from_str::<WasmIndexFile>(&raw) {
            let files: Vec<String> = parsed
                .files
                .into_iter()
                .filter(|name| !name.trim().is_empty())
                .collect();
            if !files.is_empty() {
                return files;
            }
        }
    }
    fallback.iter().map(|name| (*name).to_string()).collect()
}

pub async fn draw_hitbox(hitbox: Rect, pos: Vec2) {
    draw_rectangle(
        hitbox.x + pos.x,
        hitbox.y + pos.y,
        hitbox.w,
        hitbox.h,
        Color::from_hex(0xFF0000),
    );
}

#[derive(Clone, Copy)]
pub enum Axis {
    X,
    Y,
}

pub fn resolve_collisions_axis(
    hitbox: Rect,
    mut pos: Vec2,
    vel_axis: f32,
    colliders: &[Rect],
    axis: Axis,
) -> (Vec2, f32) {
    if vel_axis == 0.0 {
        return (pos, vel_axis);
    }

    let mut hit = false;
    let epsilon = 0.001;

    match axis {
        Axis::X => {
            let mut candidate = pos.x;
            let rect = Rect::new(
                pos.x + hitbox.x,
                pos.y + hitbox.y,
                hitbox.w,
                hitbox.h,
            );
            for collider in colliders {
                if !rect.overlaps(collider) {
                    continue;
                }
                hit = true;
                if vel_axis > 0.0 {
                    let target = collider.x - hitbox.w - hitbox.x - epsilon;
                    if target < candidate {
                        candidate = target;
                    }
                } else {
                    let target = collider.x + collider.w - hitbox.x + epsilon;
                    if target > candidate {
                        candidate = target;
                    }
                }
            }
            if hit {
                pos.x = candidate;
                return (pos, 0.0);
            }
        }
        Axis::Y => {
            let mut candidate = pos.y;
            let rect = Rect::new(
                pos.x + hitbox.x,
                pos.y + hitbox.y,
                hitbox.w,
                hitbox.h,
            );
            for collider in colliders {
                if !rect.overlaps(collider) {
                    continue;
                }
                hit = true;
                if vel_axis > 0.0 {
                    let target = collider.y - hitbox.h - hitbox.y - epsilon;
                    if target < candidate {
                        candidate = target;
                    }
                } else {
                    let target = collider.y + collider.h - hitbox.y + epsilon;
                    if target > candidate {
                        candidate = target;
                    }
                }
            }
            if hit {
                pos.y = candidate;
                return (pos, 0.0);
            }
        }
    }

    (pos, vel_axis)
}

pub fn resolve_collision_with_velocity(
    hitbox: Rect,
    pos: Vec2,
    vel: Vec2,
    other_hitbox: Rect,
    other_pos: Vec2,
) -> Vec2 {
    if vel.x == 0.0 && vel.y == 0.0 {
        return pos;
    }

    let ax = hitbox.x + pos.x;
    let ay = hitbox.y + pos.y;
    let bx = other_hitbox.x + other_pos.x;
    let by = other_hitbox.y + other_pos.y;

    if ax >= bx + other_hitbox.w
        || ax + hitbox.w <= bx
        || ay >= by + other_hitbox.h
        || ay + hitbox.h <= by
    {
        return pos;
    }

    let left = (bx + other_hitbox.w) - ax;
    let right = (ax + hitbox.w) - bx;
    let top = (by + other_hitbox.h) - ay;
    let bottom = (ay + hitbox.h) - by;

    let abs_vx = vel.x.abs();
    let abs_vy = vel.y.abs();
    let resolve_x = if abs_vx == 0.0 {
        false
    } else if abs_vy == 0.0 {
        true
    } else {
        abs_vx > abs_vy
    };

    if resolve_x {
        if vel.x > 0.0 {
            vec2(pos.x - right, pos.y)
        } else {
            vec2(pos.x + left, pos.y)
        }
    } else if vel.y > 0.0 {
        vec2(pos.x, pos.y - bottom)
    } else {
        vec2(pos.x, pos.y + top)
    }
}

pub fn clamp_hitbox_to_rect(hitbox: Rect, pos: Vec2, bounds: Rect) -> Vec2 {
    let min_x = bounds.x - hitbox.x;
    let max_x = bounds.x + bounds.w - hitbox.w - hitbox.x;
    let min_y = bounds.y - hitbox.y;
    let max_y = bounds.y + bounds.h - hitbox.h - hitbox.y;

    vec2(pos.x.clamp(min_x, max_x), pos.y.clamp(min_y, max_y))
}
