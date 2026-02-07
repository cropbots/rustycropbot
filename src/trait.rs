use crate::entity::{EntityContext, EntityInstance, MovementParams, BehaviorRuntime};
use macroquad::prelude::*;

pub fn movement_idle(
    entity: &mut EntityInstance,
    _behavior: &mut BehaviorRuntime,
    _dt: f32,
    _params: &MovementParams,
    _ctx: &EntityContext,
) {
    entity.vel = Vec2::ZERO;
}

pub fn movement_wander(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let speed = params.get("speed").copied().unwrap_or(entity.speed);
    let interval = params.get("interval").copied().unwrap_or(1.5);

    behavior.timer -= dt;
    if behavior.timer <= 0.0 || behavior.dir.length_squared() == 0.0 {
        behavior.timer = interval.max(0.1);
        let angle = macroquad::rand::gen_range(0.0, std::f32::consts::TAU);
        behavior.dir = vec2(angle.cos(), angle.sin());
    }

    entity.vel += behavior.dir * speed;
}

pub fn movement_seek(
    entity: &mut EntityInstance,
    _behavior: &mut BehaviorRuntime,
    _dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
) {
    let speed = params.get("speed").copied().unwrap_or(entity.speed);
    let Some(target) = ctx.target else {
        return;
    };

    let dir = target - entity.pos;
    if dir.length_squared() > 0.0001 {
        entity.vel += dir.normalize() * speed;
    }
}