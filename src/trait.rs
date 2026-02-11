use crate::entity::{
    BehaviorRuntime,
    EntityContext,
    EntityInstance,
    MovementParams,
    StatBlock,
    TraitDef,
    Target,
};
use macroquad::prelude::*;

pub fn append_builtin_traits(traits: &mut Vec<TraitDef>) {
    let mut push_trait = |id: &str, flags: &[&str]| {
        if traits.iter().any(|t| t.id == id) {
            return;
        }
        traits.push(TraitDef {
            id: id.to_string(),
            stats: StatBlock::default(),
            flags: flags.iter().map(|s| s.to_string()).collect(),
            tags: Default::default(),
        });
    };

    push_trait("target_player", &["target_player"]);
    push_trait("no_map_collision", &["no_map_collision"]);
}

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
    let accel = params.get("accel").copied().unwrap_or(10.0);
    let interval = params.get("interval").copied().unwrap_or(1.5);

    behavior.timer -= dt;
    if behavior.timer <= 0.0 || behavior.dir.length_squared() == 0.0 {
        behavior.timer = interval.max(0.1);
        let angle = macroquad::rand::gen_range(0.0, std::f32::consts::TAU);
        behavior.dir = vec2(angle.cos(), angle.sin());
    }

    let current_dir = if entity.vel.length_squared() > 0.0001 {
        entity.vel.normalize()
    } else {
        behavior.dir
    };
    let t = (accel * dt).clamp(0.0, 1.0);
    let smooth_dir = current_dir.lerp(behavior.dir, t);
    if smooth_dir.length_squared() > 0.0001 {
        entity.vel = smooth_dir.normalize() * speed;
    }
}

pub fn movement_seek(
    entity: &mut EntityInstance,
    _behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let speed = params.get("speed").copied().unwrap_or(entity.speed);
    let accel = params.get("accel").copied().unwrap_or(12.0);
    let Some(target) = entity.current_target.as_ref().map(Target::position) else {
        return;
    };

    let dir = target - entity.pos;
    if dir.length_squared() > 0.0001 {
        let desired_dir = dir.normalize();
        let current_dir = if entity.vel.length_squared() > 0.0001 {
            entity.vel.normalize()
        } else {
            desired_dir
        };
        let t = (accel * dt).clamp(0.0, 1.0);
        let smooth_dir = current_dir.lerp(desired_dir, t);
        if smooth_dir.length_squared() > 0.0001 {
            entity.vel = smooth_dir.normalize() * speed;
        }
    }
}

pub fn movement_flee(
    entity: &mut EntityInstance,
    _behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let speed = params.get("speed").copied().unwrap_or(entity.speed);
    let accel = params.get("accel").copied().unwrap_or(12.0);
    let Some(target) = entity.current_target.as_ref().map(Target::position) else {
        return;
    };

    let dir = entity.pos - target;
    if dir.length_squared() > 0.0001 {
        let desired_dir = dir.normalize();
        let current_dir = if entity.vel.length_squared() > 0.0001 {
            entity.vel.normalize()
        } else {
            desired_dir
        };
        let t = (accel * dt).clamp(0.0, 1.0);
        let smooth_dir = current_dir.lerp(desired_dir, t);
        if smooth_dir.length_squared() > 0.0001 {
            entity.vel = smooth_dir.normalize() * speed;
        }
    }
}

pub fn movement_dash_at_target(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    _ctx: &EntityContext,
) {
    let dash_speed = params.get("dash_speed").copied().unwrap_or(1100.0);
    let dash_duration = params.get("dash_duration").copied().unwrap_or(0.07);
    let dash_cooldown = params.get("dash_cooldown").copied().unwrap_or(0.5);

    if behavior.cooldown > 0.0 {
        behavior.cooldown = (behavior.cooldown - dt).max(0.0);
    }
    if behavior.timer > 0.0 {
        behavior.timer = (behavior.timer - dt).max(0.0);
    }

    if behavior.timer <= 0.0 && behavior.cooldown <= 0.0 {
        if let Some(target) = entity.current_target.as_ref().map(Target::position) {
            let dir = target - entity.pos;
            if dir.length_squared() > 0.0001 {
                behavior.dir = dir.normalize();
                behavior.timer = dash_duration;
                behavior.cooldown = dash_cooldown;
            }
        }
    }

    if behavior.timer > 0.0 {
        entity.vel += behavior.dir * dash_speed;
    }
}
