use macroquad::prelude::*;
use macroquad::file::load_string;
use serde::Deserialize;
use std::path::Path;
use crate::helpers::{asset_path, data_path};

const EMPTY_TILE: u16 = u16::MAX;
const CHUNK_SIZE: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridIndex {
    pub x: i32,
    pub y: i32,
}

impl GridIndex {
    pub fn new(position: Vec2, grid_size: Vec2) -> Self {
        Self {
            x: (position.x / grid_size.x).floor() as i32,
            y: (position.y / grid_size.y).floor() as i32,
        }
    }
}

#[derive(Deserialize)]
struct TilesetFile {
    image: Option<String>,
    tile_width: u16,
    tile_height: u16,
    columns: u16,
    rows: u16,
    #[serde(default)]
    tile_count: Option<u16>,
    tiles: Vec<TileInfoFile>,
}

#[derive(Deserialize)]
struct TileInfoFile {
    id: u16,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

pub struct TileSet {
    texture: Texture2D,
    tiles: Vec<Option<Rect>>,
}

impl TileSet {
    pub async fn load(tileset_json: &str, texture_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json_path = asset_path(tileset_json);
        let texture_path = asset_path(texture_path);
        let json_content = load_string(&json_path).await?;
        let parsed: TilesetFile = serde_json::from_str(&json_content)?;

        let has_tiles = !parsed.tiles.is_empty();
        let tile_count = parsed
            .tile_count
            .map(|count| count as usize)
            .unwrap_or_else(|| parsed.tiles.len().max(1));
        let mut tiles: Vec<Option<Rect>> = vec![None; tile_count];
        for tile in parsed.tiles.into_iter() {
            let idx = tile.id as usize;
            if idx >= tiles.len() {
                tiles.resize(idx + 1, None);
            }
            tiles[idx] = Some(Rect::new(
                tile.x as f32,
                tile.y as f32,
                tile.width as f32,
                tile.height as f32,
            ));
        }

        if !has_tiles {
            let columns = parsed.columns.max(1) as usize;
            let rows = parsed.rows.max(1) as usize;
            let total = columns * rows;
            if total > 0 {
                tiles.resize(total, None);
                for i in 0..total {
                    let x = (i % columns) as f32 * parsed.tile_width as f32;
                    let y = (i / columns) as f32 * parsed.tile_height as f32;
                    tiles[i] = Some(Rect::new(
                        x,
                        y,
                        parsed.tile_width as f32,
                        parsed.tile_height as f32,
                    ));
                }
            }
        }

        let texture = load_texture(&texture_path).await?;
        texture.set_filter(FilterMode::Nearest);

        if let Some(image) = parsed.image.as_ref() {
            if !image.is_empty() && image != Path::new(&texture_path).file_name().and_then(|name| name.to_str()).unwrap_or("") {
                eprintln!("tileset.json image '{}' does not match texture path '{}'", image, texture_path);
            }
        }

        Ok(Self { texture, tiles })
    }

    fn get(&self, id: u16) -> Option<Rect> {
        if id == EMPTY_TILE {
            return None;
        }
        self.tiles.get(id as usize).and_then(|rect| *rect)
    }

    pub fn texture(&self) -> &Texture2D {
        &self.texture
    }

    pub fn count(&self) -> usize {
        self.tiles.len()
    }
}

#[derive(Clone)]
pub struct Structure {
    width: usize,
    height: usize,
    background: Vec<u16>,
    foreground: Vec<u16>,
    overlay: Vec<u16>,
    colliders: Vec<bool>,
    background_updates: Vec<(usize, usize, u16)>,
    foreground_updates: Vec<(usize, usize, u16)>,
    overlay_updates: Vec<(usize, usize, u16)>,
    occupied_offsets: Vec<(usize, usize)>,
    collider_offsets: Vec<(usize, usize)>,
}

impl Structure {
    pub fn random(width: usize, height: usize, tile_count: usize, seed: u32) -> Self {
        let len = width * height;
        let mut background = vec![EMPTY_TILE; len];
        let mut foreground = vec![EMPTY_TILE; len];
        let mut overlay = vec![EMPTY_TILE; len];
        let colliders = vec![false; len];
        let max = tile_count.max(1) as u32;

        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                let n = hash_u32(x as u32, y as u32, seed) % 100;
                if n < 85 {
                    background[i] = (hash_u32(x as u32, y as u32, seed + 11) % max) as u16;
                }
                if n < 20 {
                    foreground[i] = (hash_u32(x as u32, y as u32, seed + 23) % max) as u16;
                }
                if n < 10 {
                    overlay[i] = (hash_u32(x as u32, y as u32, seed + 37) % max) as u16;
                }
            }
        }

        Self::new(width, height, background, foreground, overlay, colliders)
    }

    pub fn new(
        width: usize,
        height: usize,
        background: Vec<u16>,
        foreground: Vec<u16>,
        overlay: Vec<u16>,
        colliders: Vec<bool>,
    ) -> Self {
        let mut structure = Self {
            width,
            height,
            background,
            foreground,
            overlay,
            colliders,
            background_updates: Vec::new(),
            foreground_updates: Vec::new(),
            overlay_updates: Vec::new(),
            occupied_offsets: Vec::new(),
            collider_offsets: Vec::new(),
        };
        structure.rebuild_cache();
        structure
    }

    fn rebuild_cache(&mut self) {
        self.background_updates.clear();
        self.foreground_updates.clear();
        self.overlay_updates.clear();
        self.occupied_offsets.clear();
        self.collider_offsets.clear();

        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width + x;
                let mut occupied = false;

                let bg = self.background[i];
                if bg != EMPTY_TILE && bg != 0 {
                    self.background_updates.push((x, y, bg));
                    occupied = true;
                }

                let fg = self.foreground[i];
                if fg != EMPTY_TILE && fg != 0 {
                    self.foreground_updates.push((x, y, fg));
                    occupied = true;
                }

                let ov = self.overlay[i];
                if ov != EMPTY_TILE && ov != 0 {
                    self.overlay_updates.push((x, y, ov));
                    occupied = true;
                }

                let collider = self.colliders.get(i).copied().unwrap_or(false);
                if collider {
                    self.collider_offsets.push((x, y));
                    occupied = true;
                }

                if occupied {
                    self.occupied_offsets.push((x, y));
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.background_updates.is_empty()
            && self.foreground_updates.is_empty()
            && self.overlay_updates.is_empty()
            && self.collider_offsets.is_empty()
    }
}

#[derive(Clone)]
pub struct StructureDef {
    pub id: String,
    pub structure: Structure,
    pub frequency: f32,
    pub max_per_map: usize,
    pub min_distance: f32,
}

#[derive(Clone, Copy)]
pub enum LayerKind {
    Background,
    Foreground,
    Overlay,
}

struct Chunk {
    background: RenderTarget,
    foreground: RenderTarget,
    overlay: RenderTarget,
    dirty_background: bool,
    dirty_foreground: bool,
    dirty_overlay: bool,
}

struct StructureApplyState {
    defs: Vec<StructureDef>,
    seed: u32,
    occupied: Vec<bool>,
    placed_rects: Vec<Rect>,
    spatial: Vec<Vec<usize>>,
    cell_size: f32,
    cell_cols: usize,
    cell_rows: usize,
    def_index: usize,
    attempt_index: usize,
    target: usize,
    attempts: usize,
    max_x: usize,
    max_y: usize,
    count: usize,
    done: bool,
}

impl StructureApplyState {
    fn new(map: &TileMap, defs: Vec<StructureDef>, seed: u32) -> Self {
        let world_w = map.width as f32 * map.tile_size;
        let world_h = map.height as f32 * map.tile_size;
        let cell_size = map.chunk_pixel_size.max(map.tile_size);
        let cell_cols = ((world_w / cell_size).ceil() as usize).max(1);
        let cell_rows = ((world_h / cell_size).ceil() as usize).max(1);
        let spatial = vec![Vec::new(); cell_cols * cell_rows];

        let mut state = Self {
            defs,
            seed,
            occupied: vec![false; map.width * map.height],
            placed_rects: Vec::new(),
            spatial,
            cell_size,
            cell_cols,
            cell_rows,
            def_index: 0,
            attempt_index: 0,
            target: 0,
            attempts: 0,
            max_x: 0,
            max_y: 0,
            count: 0,
            done: false,
        };
        state.advance_def(map);
        state
    }

    fn progress(&self) -> f32 {
        if self.defs.is_empty() {
            return 1.0;
        }
        let total_defs = self.defs.len().max(1) as f32;
        let base = (self.def_index.min(self.defs.len())) as f32 / total_defs;
        let step = if self.attempts > 0 {
            (self.attempt_index.min(self.attempts)) as f32 / self.attempts as f32 / total_defs
        } else {
            0.0
        };
        (base + step).clamp(0.0, 1.0)
    }

    fn step(&mut self, map: &mut TileMap, time_budget_s: f32) -> bool {
        if self.done {
            return true;
        }
        let budget = time_budget_s.max(0.0001) as f64;
        let start = get_time();

        while (get_time() - start) < budget {
            if self.done {
                return true;
            }
            if self.attempt_index >= self.attempts || self.count >= self.target {
                self.def_index += 1;
                self.advance_def(map);
                continue;
            }

            let def = &self.defs[self.def_index];
            let i = self.attempt_index;
            self.attempt_index += 1;

            let rx = hash_u32(i as u32, self.seed ^ (self.def_index as u32 * 2654435761), 31);
            let ry = hash_u32(i as u32, self.seed ^ (self.def_index as u32 * 2246822519), 47);
            let x = (rx as usize % (self.max_x + 1)).min(self.max_x);
            let y = (ry as usize % (self.max_y + 1)).min(self.max_y);

            let pos = vec2(x as f32 * map.tile_size, y as f32 * map.tile_size);
            let size = vec2(
                def.structure.width as f32 * map.tile_size,
                def.structure.height as f32 * map.tile_size,
            );
            let rect = Rect::new(pos.x, pos.y, size.x, size.y);
            let padded = if def.min_distance > 0.0 {
                Rect::new(
                    rect.x - def.min_distance,
                    rect.y - def.min_distance,
                    rect.w + def.min_distance * 2.0,
                    rect.h + def.min_distance * 2.0,
                )
            } else {
                rect
            };

            if spatial_overlaps(
                &padded,
                &self.placed_rects,
                &self.spatial,
                self.cell_size,
                self.cell_cols,
                self.cell_rows,
            ) {
                continue;
            }

            let mut blocked = false;
            for &(sx, sy) in def.structure.occupied_offsets.iter() {
                let idx = map.idx(x + sx, y + sy);
                if self.occupied[idx] {
                    blocked = true;
                    break;
                }
            }
            if blocked {
                continue;
            }

            map.place_structure_unchecked(&def.structure, x, y);
            for &(sx, sy) in def.structure.occupied_offsets.iter() {
                let idx = map.idx(x + sx, y + sy);
                self.occupied[idx] = true;
            }

            self.placed_rects.push(padded);
            let rect_index = self.placed_rects.len() - 1;
            spatial_insert(
                rect_index,
                &padded,
                &mut self.spatial,
                self.cell_size,
                self.cell_cols,
                self.cell_rows,
            );

            self.count += 1;
        }

        self.done
    }

    fn advance_def(&mut self, map: &TileMap) {
        while self.def_index < self.defs.len() {
            let def = &self.defs[self.def_index];
            let freq = def.frequency.clamp(0.0, 1.0);
            if freq <= 0.0 || def.max_per_map == 0 || def.structure.is_empty() {
                self.def_index += 1;
                continue;
            }
            if def.structure.width == 0
                || def.structure.height == 0
                || map.width < def.structure.width
                || map.height < def.structure.height
            {
                self.def_index += 1;
                continue;
            }

            let area = (map.width * map.height) as f32;
            let target = ((area * freq).round() as usize).min(def.max_per_map);
            if target == 0 {
                self.def_index += 1;
                continue;
            }

            self.target = target;
            self.attempts = (target * 12).max(24);
            self.max_x = map.width - def.structure.width;
            self.max_y = map.height - def.structure.height;
            self.attempt_index = 0;
            self.count = 0;
            return;
        }

        self.done = true;
    }
}

pub struct TileMap {
    width: usize,
    height: usize,
    tile_size: f32,
    background: Vec<u16>,
    foreground: Vec<u16>,
    overlay: Vec<u16>,
    solid: Vec<bool>,
    collision_blocks: Vec<Rect>,
    collision_dirty: bool,
    chunk_cols: usize,
    chunk_rows: usize,
    chunk_pixel_size: f32,
    chunks: Vec<Chunk>,
    structure_apply: Option<StructureApplyState>,
    grid_size: Vec2,
    border_thickness: f32,
}

impl TileMap {
    pub fn demo(width: usize, height: usize, tile_size: f32, tile_count: usize, border_thickness: f32) -> Self {
        let mut map = Self::new(width, height, tile_size, Vec2::new(tile_size, tile_size), border_thickness);

        if tile_count > 0 {
            map.fill_layer(LayerKind::Background, 24);
        }

        map
    }

    pub fn new(width: usize, height: usize, tile_size: f32, grid_size: Vec2, border_thickness: f32) -> Self {
        let len = width * height;
        let chunk_cols = (width + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunk_rows = (height + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunk_pixel_size = tile_size * CHUNK_SIZE as f32;
        let chunk_size_u32 = chunk_pixel_size.round().max(1.0) as u32;
        let mut chunks = Vec::with_capacity(chunk_cols * chunk_rows);
        for _ in 0..chunk_cols * chunk_rows {
            let background = render_target(chunk_size_u32, chunk_size_u32);
            let foreground = render_target(chunk_size_u32, chunk_size_u32);
            let overlay = render_target(chunk_size_u32, chunk_size_u32);
            background.texture.set_filter(FilterMode::Nearest);
            foreground.texture.set_filter(FilterMode::Nearest);
            overlay.texture.set_filter(FilterMode::Nearest);
            chunks.push(Chunk {
                background,
                foreground,
                overlay,
                dirty_background: true,
                dirty_foreground: true,
                dirty_overlay: true,
            });
        }

        Self {
            width,
            height,
            tile_size,
            background: vec![EMPTY_TILE; len],
            foreground: vec![EMPTY_TILE; len],
            overlay: vec![EMPTY_TILE; len],
            solid: vec![false; len],
            collision_blocks: Vec::new(),
            collision_dirty: true,
            chunk_cols,
            chunk_rows,
            chunk_pixel_size,
            chunks,
            structure_apply: None,
            grid_size,
            border_thickness,
        }
    }

    pub fn new_deferred(width: usize, height: usize, tile_size: f32, grid_size: Vec2, border_thickness: f32) -> Self {
        let len = width * height;
        let chunk_cols = (width + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunk_rows = (height + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let chunk_pixel_size = tile_size * CHUNK_SIZE as f32;
        let total_chunks = chunk_cols * chunk_rows;

        Self {
            width,
            height,
            tile_size,
            background: vec![EMPTY_TILE; len],
            foreground: vec![EMPTY_TILE; len],
            overlay: vec![EMPTY_TILE; len],
            solid: vec![false; len],
            collision_blocks: Vec::new(),
            collision_dirty: true,
            chunk_cols,
            chunk_rows,
            chunk_pixel_size,
            chunks: Vec::with_capacity(total_chunks),
            structure_apply: None,
            grid_size,
            border_thickness,
        }
    }

    pub fn allocate_chunks_step(&mut self, time_budget_s: f32) -> bool {
        let total = self.chunk_cols * self.chunk_rows;
        if self.chunks.len() >= total {
            return true;
        }

        let chunk_size_u32 = self.chunk_pixel_size.round().max(1.0) as u32;
        let budget = time_budget_s.max(0.0001) as f64;
        let start = get_time();
        while self.chunks.len() < total && (get_time() - start) < budget {
            let background = render_target(chunk_size_u32, chunk_size_u32);
            let foreground = render_target(chunk_size_u32, chunk_size_u32);
            let overlay = render_target(chunk_size_u32, chunk_size_u32);
            background.texture.set_filter(FilterMode::Nearest);
            foreground.texture.set_filter(FilterMode::Nearest);
            overlay.texture.set_filter(FilterMode::Nearest);
            self.chunks.push(Chunk {
                background,
                foreground,
                overlay,
                dirty_background: true,
                dirty_foreground: true,
                dirty_overlay: true,
            });
        }

        self.chunks.len() >= total
    }

    pub fn allocate_chunks_progress(&self) -> f32 {
        let total = (self.chunk_cols * self.chunk_rows).max(1) as f32;
        (self.chunks.len() as f32 / total).clamp(0.0, 1.0)
    }

    pub fn start_structure_apply(&mut self, defs: Vec<StructureDef>, seed: u32) {
        self.structure_apply = Some(StructureApplyState::new(self, defs, seed));
    }

    pub fn apply_structures_step(&mut self, time_budget_s: f32) -> bool {
        let Some(mut state) = self.structure_apply.take() else {
            return true;
        };
        let done = state.step(self, time_budget_s);
        if !done {
            self.structure_apply = Some(state);
        }
        done
    }

    pub fn structure_apply_progress(&self) -> f32 {
        self.structure_apply
            .as_ref()
            .map(|state| state.progress())
            .unwrap_or(1.0)
    }

    pub fn get_border_hitbox(&self) -> Rect {
        let world_w = self.width as f32 * self.tile_size;
        let world_h = self.height as f32 * self.tile_size;
        Rect::new(
            -self.border_thickness,
            -self.border_thickness,
            world_w + self.border_thickness * 2.0,
            world_h + self.border_thickness * 2.0,
        )
    }

    pub fn draw_background(
        &mut self,
        tileset: &TileSet,
        camera_target: Vec2,
        camera_zoom: Vec2,
        screen_w: f32,
        screen_h: f32,
    ) {
        self.draw_visible_layer(
            LayerKind::Background,
            tileset,
            camera_target,
            camera_zoom,
            screen_w,
            screen_h,
        );
    }

    pub fn draw_foreground(
        &mut self,
        tileset: &TileSet,
        camera_target: Vec2,
        camera_zoom: Vec2,
        screen_w: f32,
        screen_h: f32,
    ) {
        self.draw_visible_layer(
            LayerKind::Foreground,
            tileset,
            camera_target,
            camera_zoom,
            screen_w,
            screen_h,
        );
    }

    pub fn draw_overlay(
        &mut self,
        tileset: &TileSet,
        camera_target: Vec2,
        camera_zoom: Vec2,
        screen_w: f32,
        screen_h: f32,
    ) {
        self.draw_visible_layer(
            LayerKind::Overlay,
            tileset,
            camera_target,
            camera_zoom,
            screen_w,
            screen_h,
        );
    }

    pub fn place_structure(&mut self, structure: &Structure, x: usize, y: usize) {
        if x >= self.width || y >= self.height || structure.is_empty() {
            return;
        }

        if x + structure.width <= self.width && y + structure.height <= self.height {
            self.place_structure_unchecked(structure, x, y);
            return;
        }

        let max_x = (x + structure.width).min(self.width);
        let max_y = (y + structure.height).min(self.height);
        let mut collision_changed = false;

        for &(sx, sy, tile) in structure.background_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            self.background[idx] = tile;
        }
        for &(sx, sy, tile) in structure.foreground_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            self.foreground[idx] = tile;
        }
        for &(sx, sy, tile) in structure.overlay_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            self.overlay[idx] = tile;
        }
        for &(sx, sy) in structure.collider_offsets.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            if !self.solid[idx] {
                self.solid[idx] = true;
                collision_changed = true;
            }
        }

        if collision_changed {
            self.collision_dirty = true;
        }

        let width = max_x.saturating_sub(x);
        let height = max_y.saturating_sub(y);
        self.mark_chunks_dirty_rect(
            x,
            y,
            width,
            height,
            !structure.background_updates.is_empty(),
            !structure.foreground_updates.is_empty(),
            !structure.overlay_updates.is_empty(),
        );
    }

    fn place_structure_unchecked(&mut self, structure: &Structure, x: usize, y: usize) {
        let mut collision_changed = false;

        for &(sx, sy, tile) in structure.background_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            self.background[idx] = tile;
        }
        for &(sx, sy, tile) in structure.foreground_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            self.foreground[idx] = tile;
        }
        for &(sx, sy, tile) in structure.overlay_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            self.overlay[idx] = tile;
        }
        for &(sx, sy) in structure.collider_offsets.iter() {
            let idx = self.idx(x + sx, y + sy);
            if !self.solid[idx] {
                self.solid[idx] = true;
                collision_changed = true;
            }
        }

        if collision_changed {
            self.collision_dirty = true;
        }

        self.mark_chunks_dirty_rect(
            x,
            y,
            structure.width,
            structure.height,
            !structure.background_updates.is_empty(),
            !structure.foreground_updates.is_empty(),
            !structure.overlay_updates.is_empty(),
        );
    }

    pub fn apply_structures(&mut self, defs: &[StructureDef], seed: u32) {
        let mut occupied = vec![false; self.width * self.height];
        let mut placed_rects: Vec<Rect> = Vec::new();

        let world_w = self.width as f32 * self.tile_size;
        let world_h = self.height as f32 * self.tile_size;
        let cell_size = self.chunk_pixel_size.max(self.tile_size);
        let cell_cols = ((world_w / cell_size).ceil() as usize).max(1);
        let cell_rows = ((world_h / cell_size).ceil() as usize).max(1);
        let mut spatial: Vec<Vec<usize>> = vec![Vec::new(); cell_cols * cell_rows];

        let area = (self.width * self.height) as f32;
        for (def_index, def) in defs.iter().enumerate() {
            let freq = def.frequency.clamp(0.0, 1.0);
            if freq <= 0.0 || def.max_per_map == 0 || def.structure.is_empty() {
                continue;
            }

            let target = ((area * freq).round() as usize).min(def.max_per_map);
            if target == 0 {
                continue;
            }

            let attempts = (target * 12).max(24);
            if def.structure.width == 0
                || def.structure.height == 0
                || self.width < def.structure.width
                || self.height < def.structure.height
            {
                continue;
            }
            let max_x = self.width - def.structure.width;
            let max_y = self.height - def.structure.height;

            let mut count = 0usize;
            for i in 0..attempts {
                if count >= target {
                    break;
                }
                let rx = hash_u32(i as u32, seed ^ (def_index as u32 * 2654435761), 31);
                let ry = hash_u32(i as u32, seed ^ (def_index as u32 * 2246822519), 47);
                let x = (rx as usize % (max_x + 1)).min(max_x);
                let y = (ry as usize % (max_y + 1)).min(max_y);

                let pos = vec2(x as f32 * self.tile_size, y as f32 * self.tile_size);
                let size = vec2(
                    def.structure.width as f32 * self.tile_size,
                    def.structure.height as f32 * self.tile_size,
                );
                let rect = Rect::new(pos.x, pos.y, size.x, size.y);
                let padded = if def.min_distance > 0.0 {
                    Rect::new(
                        rect.x - def.min_distance,
                        rect.y - def.min_distance,
                        rect.w + def.min_distance * 2.0,
                        rect.h + def.min_distance * 2.0,
                    )
                } else {
                    rect
                };

                if spatial_overlaps(&padded, &placed_rects, &spatial, cell_size, cell_cols, cell_rows) {
                    continue;
                }

                let mut blocked = false;
                for &(sx, sy) in def.structure.occupied_offsets.iter() {
                    let idx = self.idx(x + sx, y + sy);
                    if occupied[idx] {
                        blocked = true;
                        break;
                    }
                }

                if blocked {
                    continue;
                }

                self.place_structure_unchecked(&def.structure, x, y);
                for &(sx, sy) in def.structure.occupied_offsets.iter() {
                    let idx = self.idx(x + sx, y + sy);
                    occupied[idx] = true;
                }

                placed_rects.push(padded);
                let rect_index = placed_rects.len() - 1;
                spatial_insert(rect_index, &padded, &mut spatial, cell_size, cell_cols, cell_rows);
                count += 1;
            }
        }
    }

    pub fn fill_layer(&mut self, layer: LayerKind, id: u16) {
        let tiles = match layer {
            LayerKind::Background => &mut self.background,
            LayerKind::Foreground => &mut self.foreground,
            LayerKind::Overlay => &mut self.overlay,
        };
        tiles.fill(id);

        for cy in 0..self.chunk_rows {
            for cx in 0..self.chunk_cols {
                let chunk_index = self.chunk_index(cx, cy);
                let chunk = &mut self.chunks[chunk_index];
                match layer {
                    LayerKind::Background => chunk.dirty_background = true,
                    LayerKind::Foreground => chunk.dirty_foreground = true,
                    LayerKind::Overlay => chunk.dirty_overlay = true,
                }
            }
        }
    }

    pub fn set_tile(&mut self, layer: LayerKind, x: usize, y: usize, id: u16) {
        let i = self.idx(x, y);
        match layer {
            LayerKind::Background => self.background[i] = id,
            LayerKind::Foreground => self.foreground[i] = id,
            LayerKind::Overlay => self.overlay[i] = id,
        }
        self.mark_chunk_dirty(x, y, layer);
    }

    pub fn set_collision(&mut self, x: usize, y: usize, solid: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = self.idx(x, y);
        if self.solid[i] != solid {
            self.solid[i] = solid;
            self.collision_dirty = true;
        }
    }

    pub fn fill_collision(&mut self, solid: bool) {
        self.solid.fill(solid);
        self.collision_dirty = true;
    }

    pub fn is_solid(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.solid[self.idx(x, y)]
    }

    pub fn set_collision_from_layer(&mut self, layer: LayerKind, solid_ids: &[u16]) {
        let mut max_id = 0u16;
        for &id in solid_ids {
            if id > max_id {
                max_id = id;
            }
        }
        let mut lookup = vec![false; max_id as usize + 1];
        for &id in solid_ids {
            if (id as usize) < lookup.len() {
                lookup[id as usize] = true;
            }
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let tile = self.get_tile(layer, x, y);
                let solid = tile != EMPTY_TILE
                    && (tile as usize) < lookup.len()
                    && lookup[tile as usize];
                let idx = self.idx(x, y);
                self.solid[idx] = solid;
            }
        }

        self.collision_dirty = true;
    }

    pub fn tile_at(&self, layer: LayerKind, x: usize, y: usize) -> u16 {
        self.get_tile(layer, x, y)
    }

    pub fn collision_blocks(&mut self) -> &[Rect] {
        if self.collision_dirty {
            self.rebuild_collision_blocks();
        }
        &self.collision_blocks
    }

    pub fn grid_index(&self, position: Vec2) -> Option<GridIndex> {
        let idx = GridIndex::new(position, self.grid_size);
        if idx.x < 0 || idx.y < 0 {
            return None;
        }
        let (x, y) = (idx.x as usize, idx.y as usize);
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(idx)
    }

    pub fn grid_to_world(&self, grid: GridIndex) -> Vec2 {
        vec2(
            grid.x as f32 * self.grid_size.x,
            grid.y as f32 * self.grid_size.y,
        )
    }

    pub fn tile_bounds(&self, x: usize, y: usize) -> Rect {
        Rect::new(
            x as f32 * self.tile_size,
            y as f32 * self.tile_size,
            self.tile_size,
            self.tile_size,
        )
    }

    fn draw_visible_layer(
        &mut self,
        layer: LayerKind,
        tileset: &TileSet,
        camera_target: Vec2,
        camera_zoom: Vec2,
        _screen_w: f32,
        _screen_h: f32,
    ) {
        let (min_cx, max_cx, min_cy, max_cy) =
            self.visible_chunk_range(camera_target, camera_zoom);

        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let chunk_index = self.chunk_index(cx as usize, cy as usize);
                self.rebuild_chunk_layer_if_dirty(chunk_index, layer, tileset);
                self.draw_chunk_layer(chunk_index, layer, cx as usize, cy as usize);
            }
        }
    }

    fn visible_chunk_range(&self, camera_target: Vec2, camera_zoom: Vec2) -> (i32, i32, i32, i32) {
        let half_w = 1.0 / camera_zoom.x.abs().max(0.0001);
        let half_h = 1.0 / camera_zoom.y.abs().max(0.0001);

        let min_x = camera_target.x - half_w;
        let max_x = camera_target.x + half_w;
        let min_y = camera_target.y - half_h;
        let max_y = camera_target.y + half_h;

        let tile_min_x = (min_x / self.tile_size).floor() as i32;
        let tile_max_x = (max_x / self.tile_size).ceil() as i32;
        let tile_min_y = (min_y / self.tile_size).floor() as i32;
        let tile_max_y = (max_y / self.tile_size).ceil() as i32;

        let min_cx = tile_min_x.div_euclid(CHUNK_SIZE as i32).clamp(0, self.chunk_cols as i32 - 1);
        let max_cx = tile_max_x.div_euclid(CHUNK_SIZE as i32).clamp(0, self.chunk_cols as i32 - 1);
        let min_cy = tile_min_y.div_euclid(CHUNK_SIZE as i32).clamp(0, self.chunk_rows as i32 - 1);
        let max_cy = tile_max_y.div_euclid(CHUNK_SIZE as i32).clamp(0, self.chunk_rows as i32 - 1);

        ((min_cx as i32 - 1).max(0).min(self.chunk_cols as i32 - 1),
        (max_cx as i32 + 1).max(0).min(self.chunk_cols as i32 - 1),
        (min_cy as i32 - 1).max(0).min(self.chunk_rows as i32 - 1),
        (max_cy as i32 + 1).max(0).min(self.chunk_rows as i32 - 1))
    }

    fn rebuild_chunk_layer_if_dirty(
        &mut self,
        chunk_index: usize,
        layer: LayerKind,
        tileset: &TileSet,
    ) {
        let is_dirty = match layer {
            LayerKind::Background => self.chunks[chunk_index].dirty_background,
            LayerKind::Foreground => self.chunks[chunk_index].dirty_foreground,
            LayerKind::Overlay => self.chunks[chunk_index].dirty_overlay,
        };

        if !is_dirty {
            return;
        }

        let target = match layer {
            LayerKind::Background => self.chunks[chunk_index].background.clone(),
            LayerKind::Foreground => self.chunks[chunk_index].foreground.clone(),
            LayerKind::Overlay => self.chunks[chunk_index].overlay.clone(),
        };

        self.render_chunk_layer(target, chunk_index, layer, tileset);

        match layer {
            LayerKind::Background => self.chunks[chunk_index].dirty_background = false,
            LayerKind::Foreground => self.chunks[chunk_index].dirty_foreground = false,
            LayerKind::Overlay => self.chunks[chunk_index].dirty_overlay = false,
        }
    }

    fn render_chunk_layer(
        &self,
        target: RenderTarget,
        chunk_index: usize,
        layer: LayerKind,
        tileset: &TileSet,
    ) {
        let chunk_x = chunk_index % self.chunk_cols;
        let chunk_y = chunk_index / self.chunk_cols;

        let origin_x = chunk_x * CHUNK_SIZE;
        let origin_y = chunk_y * CHUNK_SIZE;
        let max_x = (origin_x + CHUNK_SIZE).min(self.width);
        let max_y = (origin_y + CHUNK_SIZE).min(self.height);

        let mut cam = Camera2D::from_display_rect(Rect::new(
            0.0,
            0.0,
            self.chunk_pixel_size,
            self.chunk_pixel_size,
        ));
        cam.render_target = Some(target.clone());

        push_camera_state();
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        let dest = Some(vec2(self.tile_size, self.tile_size));
        for ty in origin_y..max_y {
            for tx in origin_x..max_x {
                let tile = self.get_tile(layer, tx, ty);
                let Some(source) = tileset.get(tile) else {
                    continue;
                };

                let local_x = (tx - origin_x) as f32 * self.tile_size;
                let local_y = (ty - origin_y) as f32 * self.tile_size;
                draw_texture_ex(
                    tileset.texture(),
                    local_x,
                    local_y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(source),
                        dest_size: dest,
                        ..Default::default()
                    },
                );
            }
        }

        pop_camera_state();
    }

    fn draw_chunk_layer(&self, chunk_index: usize, layer: LayerKind, cx: usize, cy: usize) {
        let chunk = &self.chunks[chunk_index];
        let texture = match layer {
            LayerKind::Background => &chunk.background.texture,
            LayerKind::Foreground => &chunk.foreground.texture,
            LayerKind::Overlay => &chunk.overlay.texture,
        };

        let world_x = cx as f32 * self.chunk_pixel_size;
        let world_y = cy as f32 * self.chunk_pixel_size;
        let dest = Some(vec2(self.chunk_pixel_size, self.chunk_pixel_size));

        draw_texture_ex(
            texture,
            world_x,
            world_y,
            WHITE,
            DrawTextureParams {
                dest_size: dest,
                flip_y: true,
                ..Default::default()
            },
        );
    }

    fn get_tile(&self, layer: LayerKind, x: usize, y: usize) -> u16 {
        let i = self.idx(x, y);
        match layer {
            LayerKind::Background => self.background[i],
            LayerKind::Foreground => self.foreground[i],
            LayerKind::Overlay => self.overlay[i],
        }
    }

    fn rebuild_collision_blocks(&mut self) {
        self.collision_blocks.clear();
        let mut visited = vec![false; self.solid.len()];

        for y in 0..self.height {
            for x in 0..self.width {
                let i = self.idx(x, y);
                if visited[i] || !self.solid[i] {
                    continue;
                }

                let mut max_w = 0;
                while x + max_w < self.width {
                    let idx = self.idx(x + max_w, y);
                    if self.solid[idx] && !visited[idx] {
                        max_w += 1;
                    } else {
                        break;
                    }
                }

                let mut max_h = 1;
                'height: loop {
                    if y + max_h >= self.height {
                        break;
                    }
                    for tx in 0..max_w {
                        let idx = self.idx(x + tx, y + max_h);
                        if !self.solid[idx] || visited[idx] {
                            break 'height;
                        }
                    }
                    max_h += 1;
                }

                for dy in 0..max_h {
                    for dx in 0..max_w {
                        visited[self.idx(x + dx, y + dy)] = true;
                    }
                }

                self.collision_blocks.push(Rect::new(
                    x as f32 * self.tile_size,
                    y as f32 * self.tile_size,
                    max_w as f32 * self.tile_size,
                    max_h as f32 * self.tile_size,
                ));
            }
        }

        self.collision_dirty = false;
    }

    pub fn hitboxes_around_grid(&self, grid: GridIndex, radius: i32) -> Vec<Rect> {
        let mut hitboxes = Vec::new();
        self.fill_hitboxes_around_grid(grid, radius, &mut hitboxes);
        hitboxes
    }

    pub fn fill_hitboxes_around_grid(&self, grid: GridIndex, radius: i32, out: &mut Vec<Rect>) {
        out.clear();
        let start_x = grid.x - radius;
        let end_x = grid.x + radius;
        let start_y = grid.y - radius;
        let end_y = grid.y + radius;

        for y in start_y..=end_y {
            for x in start_x..=end_x {
                if x < 0 || y < 0 {
                    continue;
                }
                let (ux, uy) = (x as usize, y as usize);
                if ux >= self.width || uy >= self.height {
                    continue;
                }
                if self.solid[self.idx(ux, uy)] {
                    out.push(self.tile_bounds(ux, uy));
                }
            }
        }
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }

    fn mark_chunks_dirty_rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        mark_background: bool,
        mark_foreground: bool,
        mark_overlay: bool,
    ) {
        if width == 0 || height == 0 || (!mark_background && !mark_foreground && !mark_overlay) {
            return;
        }

        let end_x = (x + width - 1).min(self.width.saturating_sub(1));
        let end_y = (y + height - 1).min(self.height.saturating_sub(1));
        let start_cx = x / CHUNK_SIZE;
        let start_cy = y / CHUNK_SIZE;
        let end_cx = end_x / CHUNK_SIZE;
        let end_cy = end_y / CHUNK_SIZE;

        for cy in start_cy..=end_cy {
            for cx in start_cx..=end_cx {
                let chunk_index = self.chunk_index(cx, cy);
                let chunk = &mut self.chunks[chunk_index];
                if mark_background {
                    chunk.dirty_background = true;
                }
                if mark_foreground {
                    chunk.dirty_foreground = true;
                }
                if mark_overlay {
                    chunk.dirty_overlay = true;
                }
            }
        }
    }

    fn mark_chunk_dirty(&mut self, x: usize, y: usize, layer: LayerKind) {
        let cx = x / CHUNK_SIZE;
        let cy = y / CHUNK_SIZE;
        if cx >= self.chunk_cols || cy >= self.chunk_rows {
            return;
        }
        let chunk_index = self.chunk_index(cx, cy);
        let chunk = &mut self.chunks[chunk_index];
        match layer {
            LayerKind::Background => chunk.dirty_background = true,
            LayerKind::Foreground => chunk.dirty_foreground = true,
            LayerKind::Overlay => chunk.dirty_overlay = true,
        }
    }

    fn chunk_index(&self, cx: usize, cy: usize) -> usize {
        cy * self.chunk_cols + cx
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

fn spatial_cell_range(
    rect: &Rect,
    cell_size: f32,
    cols: usize,
    rows: usize,
) -> (usize, usize, usize, usize) {
    let max_col = cols.saturating_sub(1) as i32;
    let max_row = rows.saturating_sub(1) as i32;
    let min_cx = (rect.x / cell_size).floor() as i32;
    let max_cx = ((rect.x + rect.w) / cell_size).floor() as i32;
    let min_cy = (rect.y / cell_size).floor() as i32;
    let max_cy = ((rect.y + rect.h) / cell_size).floor() as i32;

    let min_cx = min_cx.clamp(0, max_col);
    let max_cx = max_cx.clamp(0, max_col);
    let min_cy = min_cy.clamp(0, max_row);
    let max_cy = max_cy.clamp(0, max_row);

    (min_cx as usize, max_cx as usize, min_cy as usize, max_cy as usize)
}

fn spatial_overlaps(
    rect: &Rect,
    placed: &[Rect],
    grid: &[Vec<usize>],
    cell_size: f32,
    cols: usize,
    rows: usize,
) -> bool {
    if placed.is_empty() {
        return false;
    }

    let (min_cx, max_cx, min_cy, max_cy) = spatial_cell_range(rect, cell_size, cols, rows);
    for cy in min_cy..=max_cy {
        for cx in min_cx..=max_cx {
            let cell = &grid[cy * cols + cx];
            for &idx in cell {
                if placed[idx].overlaps(rect) {
                    return true;
                }
            }
        }
    }
    false
}

fn spatial_insert(
    index: usize,
    rect: &Rect,
    grid: &mut [Vec<usize>],
    cell_size: f32,
    cols: usize,
    rows: usize,
) {
    let (min_cx, max_cx, min_cy, max_cy) = spatial_cell_range(rect, cell_size, cols, rows);
    for cy in min_cy..=max_cy {
        for cx in min_cx..=max_cx {
            grid[cy * cols + cx].push(index);
        }
    }
}

fn hash_u32(x: u32, y: u32, seed: u32) -> u32 {
    let mut v = x.wrapping_mul(0x9E3779B1) ^ y.wrapping_mul(0x85EBCA6B) ^ seed;
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB352D);
    v ^= v >> 15;
    v
}

pub async fn load_structures_from_dir(dir: impl AsRef<Path>) -> Result<Vec<StructureDef>, std::io::Error> {
    let mut defs = Vec::new();

    if cfg!(target_arch = "wasm32") {
        let dir = data_path(&dir.as_ref().to_string_lossy());
        let files = ["tree_plains.json", "bush_plains.json"];
        for file in files {
            let path = format!("{}/{}", dir, file);
            let raw_str = load_string(&path)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            let raw: StructureFile = serde_json::from_str(&raw_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let tile_len = raw.width * raw.height;
            let mut colliders = raw.colliders.unwrap_or_default();
            if colliders.len() != tile_len {
                colliders = vec![false; tile_len];
            }
            let structure = Structure::new(
                raw.width,
                raw.height,
                raw.background,
                raw.foreground,
                raw.overlay,
                colliders,
            );

            defs.push(StructureDef {
                id: raw.id,
                structure,
                frequency: raw.frequency.unwrap_or(0.05),
                max_per_map: raw.max_per_map.unwrap_or(10),
                min_distance: raw.min_distance.unwrap_or(64.0),
            });
        }
        return Ok(defs);
    }

    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(defs);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw: StructureFile = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tile_len = raw.width * raw.height;
        let mut colliders = raw.colliders.unwrap_or_default();
        if colliders.len() != tile_len {
            colliders = vec![false; tile_len];
        }
        let structure = Structure::new(
            raw.width,
            raw.height,
            raw.background,
            raw.foreground,
            raw.overlay,
            colliders,
        );

        defs.push(StructureDef {
            id: raw.id,
            structure,
            frequency: raw.frequency.unwrap_or(0.05),
            max_per_map: raw.max_per_map.unwrap_or(10),
            min_distance: raw.min_distance.unwrap_or(64.0),
        });
    }

    Ok(defs)
}

#[derive(Deserialize)]
struct StructureFile {
    id: String,
    width: usize,
    height: usize,
    background: Vec<u16>,
    #[serde(default)]
    foreground: Vec<u16>,
    #[serde(default)]
    overlay: Vec<u16>,
    #[serde(default)]
    colliders: Option<Vec<bool>>,
    #[serde(default)]
    frequency: Option<f32>,
    #[serde(default)]
    max_per_map: Option<usize>,
    #[serde(default)]
    min_distance: Option<f32>,
}
