use macroquad::prelude::*;

pub async fn load_single_texture(dir: &str, name: &str) -> Option<Texture2D> {
    let tile_path = format!("{}/{}.png", dir, name);
    load_texture(&tile_path).await.ok()
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

pub struct Entity {
    position: Vec2,
    hitbox: Rect,
}
