use macroquad::prelude::*;

use crate::helpers::{clamp_hitbox_to_rect, resolve_collisions_axis, Axis};
use crate::map::TileMap;

pub struct Player {
    pos: Vec2,
    vel: Vec2,
    hitbox: Rect,
    radius: f32,
    pub texture: Texture2D,
    last_move_dir: Vec2,
    dash_timer: f32,
    dash_cooldown: f32,
    dash_dir: Vec2,
    collision_scratch: Vec<Rect>,
    hp: f32,
}

impl Player {
    pub fn new(pos: Vec2, texture: Texture2D, hitbox: Rect) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            hitbox,
            radius: 5.0,
            texture,
            last_move_dir: Vec2::ZERO,
            dash_timer: 0.0,
            dash_cooldown: 0.0,
            dash_dir: Vec2::ZERO,
            collision_scratch: Vec::with_capacity(25),
            hp: 100.0,
        }
    }

    pub fn update(&mut self, map: &TileMap) {
        let dt = get_frame_time();

        let mut input = vec2(0.0, 0.0);
        if is_key_down(KeyCode::D) {
            input.x += 1.0;
        }
        if is_key_down(KeyCode::A) {
            input.x -= 1.0;
        }
        if is_key_down(KeyCode::W) {
            input.y -= 1.0;
        }
        if is_key_down(KeyCode::S) {
            input.y += 1.0;
        }

        if input.length_squared() > 0.0 {
            input = input.normalize();
            self.last_move_dir = input;
        }

        let accel = 1800.0;
        let max_speed = 640.0;
        let damping = 8.0;
        let dash_speed = 1100.0;
        let dash_duration = 0.07;
        let dash_cooldown = 0.5;

        if self.dash_cooldown > 0.0 {
            self.dash_cooldown = (self.dash_cooldown - dt).max(0.0);
        }

        if self.dash_timer > 0.0 {
            self.dash_timer = (self.dash_timer - dt).max(0.0);
        }

        if self.dash_timer <= 0.0
            && self.dash_cooldown <= 0.0
            && is_key_pressed(KeyCode::Space)
        {
            let dir = if input.length_squared() > 0.0 {
                input
            } else {
                self.last_move_dir
            };
            if dir.length_squared() > 0.0 {
                self.dash_dir = dir.normalize();
                self.dash_timer = dash_duration;
                self.dash_cooldown = dash_cooldown;
            }
        }

        if self.dash_timer > 0.0 {
            self.vel = self.dash_dir * dash_speed;
        } else {
            self.vel += input * accel * dt;
        }

        let speed = self.vel.length();
        if speed > max_speed {
            self.vel = self.vel / speed * max_speed;
        }

        if self.dash_timer <= 0.0 {
            let decay = (1.0 - damping * dt).clamp(0.0, 1.0);
            self.vel *= decay;
        }

        let mut pos = self.pos;
        let mut vel = self.vel;

        pos.x += vel.x * dt;
        if !self.is_dashing() {
            if let Some(grid) = map.grid_index(pos) {
                let radius = collision_radius(map, vel, dt);
                map.fill_hitboxes_around_grid(grid, radius, &mut self.collision_scratch);
                let (resolved, vx) = resolve_collisions_axis(
                    self.hitbox,
                    pos,
                    vel.x,
                    &self.collision_scratch,
                    Axis::X,
                );
                pos = resolved;
                vel.x = vx;
            }
        }

        pos.y += vel.y * dt;
        if !self.is_dashing() {
            if let Some(grid) = map.grid_index(pos) {
                let radius = collision_radius(map, vel, dt);
                map.fill_hitboxes_around_grid(grid, radius, &mut self.collision_scratch);
                let (resolved, vy) = resolve_collisions_axis(
                    self.hitbox,
                    pos,
                    vel.y,
                    &self.collision_scratch,
                    Axis::Y,
                );
                pos = resolved;
                vel.y = vy;
            }
        }

        self.pos = pos;
        self.vel = vel;

        let border = map.get_border_hitbox();
        self.pos = clamp_hitbox_to_rect(self.hitbox, self.pos, border);
    }


    pub fn draw(&self) {
        let scale = 0.5;
        let center_x = self.texture.width() as f32 * scale / 2.0;
        let center_y = self.texture.height() as f32 * scale / 2.0;
        draw_texture_ex(
            &self.texture,
            self.pos.x - center_x / 2.0,
            self.pos.y - center_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(self.texture.width() / 2 as f32 * scale, self.texture.height() / 2 as f32 * scale)),
                flip_y: false,
                ..Default::default()
            },
        );
    }

    pub fn position(&self) -> Vec2 {
        self.pos
    }

    pub fn world_hitbox(&self) -> Rect {
        Rect::new(
            self.pos.x + self.hitbox.x,
            self.pos.y + self.hitbox.y,
            self.hitbox.w,
            self.hitbox.h,
        )
    }

    pub fn apply_damage(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        self.hp = (self.hp - amount).max(0.0);
    }

    pub fn velocity(&self) -> Vec2 {
        self.vel
    }

    pub fn is_dashing(&self) -> bool {
        self.dash_timer > 0.0
    }

    pub fn is_moving(&self, deadzone: f32) -> bool {
        self.vel.length() > deadzone
    }
}

fn collision_radius(map: &TileMap, vel: Vec2, dt: f32) -> i32 {
    let speed = vel.length();
    let tiles = (speed * dt / map.tile_size().max(1.0)).ceil() as i32;
    (1 + tiles).clamp(1, 4)
}
