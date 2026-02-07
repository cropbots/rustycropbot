use macroquad::prelude::*;
use serde::Deserialize;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::path::Path;

use crate::r#trait::*;

pub type MovementFn = fn(
    entity: &mut EntityInstance,
    behavior: &mut BehaviorRuntime,
    dt: f32,
    params: &MovementParams,
    ctx: &EntityContext,
);

pub type MovementParams = HashMap<String, f32>;

#[derive(Debug)]
pub enum EntityLoadError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    Texture(String),
    MissingDefinition(String),
}

impl std::fmt::Display for EntityLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Yaml(err) => write!(f, "yaml error: {err}"),
            Self::Texture(err) => write!(f, "texture error: {err}"),
            Self::MissingDefinition(err) => write!(f, "missing definition: {err}"),
        }
    }
}

impl std::error::Error for EntityLoadError {}

impl From<std::io::Error> for EntityLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_yaml::Error> for EntityLoadError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Yaml(err)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    Enemy,
    Friend,
    Misc,
}

impl EntityKind {
    fn from_dir(name: &str) -> Option<Self> {
        match name {
            "enemy" => Some(Self::Enemy),
            "friend" => Some(Self::Friend),
            "misc" => Some(Self::Misc),
            _ => None,
        }
    }
}

#[derive(Default, Clone)]
pub struct StatBlock {
    values: HashMap<String, f32>,
}

impl StatBlock {
    pub fn add(&mut self, key: &str, value: f32) {
        *self.values.entry(key.to_string()).or_insert(0.0) += value;
    }

    pub fn merge(&mut self, other: &StatBlock) {
        for (key, value) in &other.values {
            *self.values.entry(key.clone()).or_insert(0.0) += value;
        }
    }

    pub fn get(&self, key: &str, default: f32) -> f32 {
        self.values.get(key).copied().unwrap_or(default)
    }
}

#[derive(Clone)]
pub struct TraitDef {
    pub id: String,
    pub stats: StatBlock,
    pub flags: Vec<String>,
    pub tags: HashMap<String, YamlValue>,
}

#[derive(Clone)]
pub struct BehaviorDef {
    pub id: String,
    pub tree: BehaviorNode,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BehaviorNode {
    Selector { children: Vec<BehaviorNode> },
    Sequence { children: Vec<BehaviorNode> },
    Condition { name: String, value: Option<f32> },
    Action { name: String },
}

#[derive(Clone)]
pub struct TextureInfo {
    pub texture: Texture2D,
    pub draw: DrawParams,
}

#[derive(Clone)]
pub struct DrawParams {
    pub dest_size: Option<Vec2>,
    pub rotation: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub pivot: Option<Vec2>,
    pub color: Color,
    pub offset: Vec2,
}

#[derive(Clone)]
pub struct EntityDef {
    pub id: String,
    pub name: String,
    pub kind: EntityKind,
    pub texture: TextureInfo,
    pub hitbox: Rect,
    pub traits: Vec<usize>,
    pub trait_tags: HashMap<String, YamlValue>,
    pub behavior_tree: Option<BehaviorNode>,
    pub base_stats: StatBlock,
    pub speed: f32,
}

impl EntityDef {
    pub fn draw(&self, pos: Vec2) {
        let tex = &self.texture.texture;
        let draw = &self.texture.draw;

        let dest = draw.dest_size.or_else(|| Some(vec2(tex.width(), tex.height())));
        let params = DrawTextureParams {
            dest_size: dest,
            rotation: draw.rotation,
            flip_x: draw.flip_x,
            flip_y: draw.flip_y,
            pivot: draw.pivot,
            ..Default::default()
        };

        draw_texture_ex(
            tex,
            pos.x + draw.offset.x,
            pos.y + draw.offset.y,
            draw.color,
            params,
        );
    }

    pub fn world_hitbox(&self, pos: Vec2) -> Rect {
        Rect::new(
            pos.x + self.hitbox.x,
            pos.y + self.hitbox.y,
            self.hitbox.w,
            self.hitbox.h,
        )
    }
}

pub struct BehaviorRuntime {
    pub func: MovementFn,
    pub params: MovementParams,
    pub timer: f32,
    pub dir: Vec2,
}

pub struct EntityInstance {
    pub def: usize,
    pub pos: Vec2,
    pub vel: Vec2,
    pub speed: f32,
    pub behaviors: Vec<BehaviorRuntime>,
    pub stats: StatBlock,
}

impl EntityInstance {
    pub fn update(&mut self, dt: f32, db: &EntityDatabase, ctx: &EntityContext) {
        self.vel = Vec2::ZERO;
        let mut behaviors = std::mem::take(&mut self.behaviors);
        for behavior in behaviors.iter_mut() {
            let func = behavior.func;
            let params = std::mem::take(&mut behavior.params);
            (func)(self, behavior, dt, &params, ctx);
            behavior.params = params;
        }
        self.behaviors = behaviors;
        self.pos += self.vel * dt;

        let def = &db.entities[self.def];
        let max_speed = def.speed.max(1.0);
        let speed = self.vel.length();
        if speed > max_speed {
            self.vel = self.vel / speed * max_speed;
        }
    }

    pub fn draw(&self, db: &EntityDatabase) {
        db.entities[self.def].draw(self.pos);
    }

    pub fn hitbox(&self, db: &EntityDatabase) -> Rect {
        db.entities[self.def].world_hitbox(self.pos)
    }
}

#[derive(Default)]
pub struct MovementRegistry {
    fns: HashMap<String, MovementFn>,
}

impl MovementRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            fns: HashMap::new(),
        };
        registry.register("idle", movement_idle);
        registry.register("wander", movement_wander);
        registry.register("seek", movement_seek);
        registry
    }

    pub fn register(&mut self, name: &str, func: MovementFn) {
        self.fns.insert(name.to_string(), func);
    }

    pub fn resolve(&self, name: &str) -> MovementFn {
        self.fns
            .get(name)
            .copied()
            .unwrap_or(movement_idle)
    }
}

#[derive(Clone, Copy, Default)]
pub struct EntityContext {
    pub target: Option<Vec2>,
}

pub struct EntityDatabase {
    pub traits: Vec<TraitDef>,
    pub behaviors: Vec<BehaviorDef>,
    pub entities: Vec<EntityDef>,
    trait_lookup: HashMap<String, usize>,
    behavior_lookup: HashMap<String, usize>,
    entity_lookup: HashMap<String, usize>,
}

impl EntityDatabase {
    pub async fn load_from(root: impl AsRef<Path>) -> Result<Self, EntityLoadError> {
        let root = root.as_ref();
        let behavior_dir = root.join("behaviour");
        let trait_dir = root.join("trait");
        let enemy_dir = root.join("enemy");
        let friend_dir = root.join("friend");
        let misc_dir = root.join("misc");

        let behaviors = load_behaviors(&behavior_dir)?;
        let traits = load_traits(&trait_dir)?;
        let (trait_lookup, behavior_lookup) = build_lookups(&traits, &behaviors);

        let mut entities = Vec::new();
        let mut entity_lookup = HashMap::new();
        load_entities_from_dir(
            &enemy_dir,
            EntityKind::Enemy,
            &trait_lookup,
            &behavior_lookup,
            &traits,
            &behaviors,
            &mut entities,
            &mut entity_lookup,
        )
        .await?;
        load_entities_from_dir(
            &friend_dir,
            EntityKind::Friend,
            &trait_lookup,
            &behavior_lookup,
            &traits,
            &behaviors,
            &mut entities,
            &mut entity_lookup,
        )
        .await?;
        load_entities_from_dir(
            &misc_dir,
            EntityKind::Misc,
            &trait_lookup,
            &behavior_lookup,
            &traits,
            &behaviors,
            &mut entities,
            &mut entity_lookup,
        )
        .await?;

        Ok(Self {
            traits,
            behaviors,
            entities,
            trait_lookup,
            behavior_lookup,
            entity_lookup,
        })
    }

    pub fn entity_id(&self, id: &str) -> Option<usize> {
        self.entity_lookup.get(id).copied()
    }

    pub fn spawn(
        &self,
        id: &str,
        pos: Vec2,
        registry: &MovementRegistry,
    ) -> Option<EntityInstance> {
        let index = self.entity_lookup.get(id).copied()?;
        let def = &self.entities[index];

        let mut stats = def.base_stats.clone();
        for &trait_idx in &def.traits {
            stats.merge(&self.traits[trait_idx].stats);
        }

        let mut behaviors = Vec::new();
        if let Some(tree) = &def.behavior_tree {
            let _ = tree;
        }

        behaviors.push(BehaviorRuntime {
            func: registry.resolve("idle"),
            params: MovementParams::new(),
            timer: 0.0,
            dir: Vec2::ZERO,
        });

        Some(EntityInstance {
            def: index,
            pos,
            vel: Vec2::ZERO,
            speed: def.speed,
            behaviors,
            stats,
        })
    }
}

fn build_lookups(
    traits: &[TraitDef],
    behaviors: &[BehaviorDef],
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut trait_lookup = HashMap::new();
    for (i, def) in traits.iter().enumerate() {
        trait_lookup.insert(def.id.clone(), i);
    }

    let mut behavior_lookup = HashMap::new();
    for (i, def) in behaviors.iter().enumerate() {
        behavior_lookup.insert(def.id.clone(), i);
    }

    (trait_lookup, behavior_lookup)
}

fn load_behaviors(dir: &Path) -> Result<Vec<BehaviorDef>, EntityLoadError> {
    let mut behaviors = Vec::new();
    if !dir.exists() {
        return Ok(behaviors);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_yaml(&path) {
            continue;
        }
        let raw: BehaviorFile = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
        behaviors.push(BehaviorDef {
            id: raw.id,
            tree: raw.behavior,
        });
    }

    Ok(behaviors)
}

fn load_traits(dir: &Path) -> Result<Vec<TraitDef>, EntityLoadError> {
    let mut traits = Vec::new();
    if !dir.exists() {
        return Ok(traits);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_yaml(&path) {
            continue;
        }
        let raw: TraitFile = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
        let mut stats = StatBlock::default();
        for (key, value) in raw.stats {
            stats.add(&key, value);
        }
        traits.push(TraitDef {
            id: raw.id,
            stats,
            flags: raw.flags,
            tags: raw.tags,
        });
    }

    Ok(traits)
}

async fn load_entities_from_dir(
    dir: &Path,
    fallback_kind: EntityKind,
    trait_lookup: &HashMap<String, usize>,
    behavior_lookup: &HashMap<String, usize>,
    traits: &[TraitDef],
    behaviors: &[BehaviorDef],
    entities: &mut Vec<EntityDef>,
    entity_lookup: &mut HashMap<String, usize>,
) -> Result<(), EntityLoadError> {
    if !dir.exists() {
        return Ok(());
    }

    let kind_from_dir = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(EntityKind::from_dir)
        .unwrap_or(fallback_kind);

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_yaml(&path) {
            continue;
        }
        let raw: EntityFile = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
        let kind = raw.kind.unwrap_or(kind_from_dir);

        let mut trait_indices = Vec::with_capacity(raw.traits.len());
        for id in raw.traits {
            let idx = trait_lookup
                .get(&id)
                .copied()
                .ok_or_else(|| EntityLoadError::MissingDefinition(format!("trait {id}")))?;
            trait_indices.push(idx);
        }

        let mut tags = raw.trait_tags;
        for &trait_idx in &trait_indices {
            let trait_def = &traits[trait_idx];
            for (key, value) in &trait_def.tags {
                tags.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }

        let behavior_tree = if let Some(behavior) = raw.behavior {
            Some(behavior)
        } else if let Some(id) = raw.behavior_id {
            let idx = behavior_lookup
                .get(&id)
                .copied()
                .ok_or_else(|| EntityLoadError::MissingDefinition(format!("behavior {id}")))?;
            Some(behaviors[idx].tree.clone())
        } else {
            None
        };

        let tex = load_texture(&raw.visuals.sprite)
            .await
            .map_err(|err| EntityLoadError::Texture(err.to_string()))?;
        tex.set_filter(FilterMode::Nearest);

        let draw_params = raw.visuals.draw_params.unwrap_or_default();
        let color = Color::from_rgba(
            draw_params.color[0],
            draw_params.color[1],
            draw_params.color[2],
            draw_params.color[3],
        );

        let dest_size = draw_params
            .dest_size
            .map(|v| vec2(v[0], v[1]));
        let pivot = draw_params.pivot.map(|v| vec2(v[0], v[1]));

        let hitbox = Rect::new(
            raw.hitbox.x,
            raw.hitbox.y,
            raw.hitbox.w,
            raw.hitbox.h,
        );

        let mut base_stats = StatBlock::default();
        for (key, value) in raw.stats {
            base_stats.add(&key, value);
        }

        let def = EntityDef {
            id: raw.id.clone(),
            name: raw.name.unwrap_or_else(|| raw.id.clone()),
            kind,
            texture: TextureInfo {
                texture: tex,
                draw: DrawParams {
                    dest_size,
                    rotation: draw_params.rotation,
                    flip_x: draw_params.flip_x,
                    flip_y: draw_params.flip_y,
                    pivot,
                    color,
                    offset: vec2(draw_params.offset[0], draw_params.offset[1]),
                },
            },
            hitbox,
            traits: trait_indices,
            trait_tags: tags,
            behavior_tree,
            base_stats,
            speed: raw.speed,
        };

        let index = entities.len();
        entities.push(def);
        entity_lookup.insert(raw.id, index);
    }

    let _ = traits;

    Ok(())
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}



#[derive(Deserialize)]
struct BehaviorFile {
    id: String,
    behavior: BehaviorNode,
}

#[derive(Deserialize)]
struct TraitFile {
    id: String,
    #[serde(default)]
    stats: HashMap<String, f32>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    tags: HashMap<String, YamlValue>,
}

#[derive(Deserialize)]
struct EntityFile {
    id: String,
    name: Option<String>,
    visuals: VisualsFile,
    hitbox: HitboxFile,
    #[serde(default)]
    traits: Vec<String>,
    #[serde(default)]
    trait_tags: HashMap<String, YamlValue>,
    #[serde(default)]
    stats: HashMap<String, f32>,
    #[serde(default = "default_speed")]
    speed: f32,
    kind: Option<EntityKind>,
    #[serde(default)]
    behavior: Option<BehaviorNode>,
    #[serde(default)]
    behavior_id: Option<String>,
}

#[derive(Deserialize)]
struct VisualsFile {
    sprite: String,
    #[serde(default)]
    draw_params: Option<DrawParamsFile>,
}

#[derive(Default, Deserialize)]
struct DrawParamsFile {
    #[serde(default)]
    dest_size: Option<[f32; 2]>,
    #[serde(default)]
    rotation: f32,
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
    #[serde(default)]
    pivot: Option<[f32; 2]>,
    #[serde(default = "default_color")]
    color: [u8; 4],
    #[serde(default = "default_offset")]
    offset: [f32; 2],
}

#[derive(Deserialize)]
struct HitboxFile {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn default_offset() -> [f32; 2] {
    [0.0, 0.0]
}

fn default_color() -> [u8; 4] {
    [255, 255, 255, 255]
}

fn default_speed() -> f32 {
    80.0
}
