use macroquad::prelude::*;
use macroquad::file::load_string;
use serde::Deserialize;
use std::path::Path;
use crate::helpers::{asset_path, data_path, load_wasm_manifest_files};

const EMPTY_TILE: u8 = u8::MAX;
const CHUNK_SIZE: usize = 32;

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
        if tiles.len() > EMPTY_TILE as usize {
            eprintln!(
                "tileset has {} tiles, truncating to {} (tile id {} reserved for empty)",
                tiles.len(),
                EMPTY_TILE as usize,
                EMPTY_TILE
            );
            tiles.truncate(EMPTY_TILE as usize);
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

    fn get(&self, id: u8) -> Option<Rect> {
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
    background: Vec<u8>,
    foreground: Vec<u8>,
    overlay: Vec<u8>,
    colliders: Vec<u8>,
    interactors: Vec<u8>,
    background_updates: Vec<(usize, usize, u8)>,
    foreground_updates: Vec<(usize, usize, u8)>,
    overlay_updates: Vec<(usize, usize, u8)>,
    occupied_offsets: Vec<(usize, usize)>,
    collider_offsets: Vec<(usize, usize, u8)>,
    interactor_offsets: Vec<(usize, usize, u8)>,
}

impl Structure {
    pub fn random(width: usize, height: usize, tile_count: usize, seed: u32) -> Self {
        let len = width * height;
        let mut background = vec![EMPTY_TILE; len];
        let mut foreground = vec![EMPTY_TILE; len];
        let mut overlay = vec![EMPTY_TILE; len];
        let colliders = vec![0u8; len];
        let interactors = vec![0u8; len];
        let max = (tile_count.max(1).min(u8::MAX as usize - 1)) as u32;

        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                let n = hash_u32(x as u32, y as u32, seed) % 100;
                if n < 85 {
                    background[i] = (hash_u32(x as u32, y as u32, seed + 11) % max) as u8;
                }
                if n < 20 {
                    foreground[i] = (hash_u32(x as u32, y as u32, seed + 23) % max) as u8;
                }
                if n < 10 {
                    overlay[i] = (hash_u32(x as u32, y as u32, seed + 37) % max) as u8;
                }
            }
        }

        Self::new(
            width,
            height,
            background,
            foreground,
            overlay,
            colliders,
            interactors,
        )
    }

    pub fn new(
        width: usize,
        height: usize,
        background: Vec<u8>,
        foreground: Vec<u8>,
        overlay: Vec<u8>,
        colliders: Vec<u8>,
        interactors: Vec<u8>,
    ) -> Self {
        let mut structure = Self {
            width,
            height,
            background,
            foreground,
            overlay,
            colliders,
            interactors,
            background_updates: Vec::new(),
            foreground_updates: Vec::new(),
            overlay_updates: Vec::new(),
            occupied_offsets: Vec::new(),
            collider_offsets: Vec::new(),
            interactor_offsets: Vec::new(),
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
        self.interactor_offsets.clear();

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

                let collider = self.colliders.get(i).copied().unwrap_or(0);
                let collider = collider & 0x0F;
                if collider != 0 {
                    self.collider_offsets.push((x, y, collider));
                    occupied = true;
                }

                let interactor = self.interactors.get(i).copied().unwrap_or(0);
                let interactor = interactor & 0x0F;
                if interactor != 0 {
                    self.interactor_offsets.push((x, y, interactor));
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
            && self.interactor_offsets.is_empty()
    }
}

#[derive(Clone)]
pub struct StructureDef {
    pub id: String,
    pub structure: Structure,
    pub on_interact: Vec<String>,
    pub interact_range: f32,
    pub frequency: f32,
    pub max_per_map: usize,
    pub min_distance: f32,
}

#[derive(Clone)]
pub struct StructureInteractor {
    pub structure_id: String,
    pub rect: Rect,
    pub group_rect: Rect,
    pub on_interact: Vec<String>,
    pub interact_range_world: f32,
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
    ready_background: bool,
    ready_foreground: bool,
    ready_overlay: bool,
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

            let def_seed = (self.def_index as u32).wrapping_mul(2654435761);
            let def_seed_y = (self.def_index as u32).wrapping_mul(2246822519);
            let rx = hash_u32(i as u32, self.seed ^ def_seed, 31);
            let ry = hash_u32(i as u32, self.seed ^ def_seed_y, 47);
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
            map.register_structure_interactors(def, x, y);
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
    background: Vec<u8>,
    foreground: Vec<u8>,
    overlay: Vec<u8>,
    solid: Vec<bool>,
    collision_mask: Vec<u8>,
    collision_blocks: Vec<Rect>,
    collision_dirty: bool,
    chunk_cols: usize,
    chunk_rows: usize,
    chunk_pixel_size: f32,
    chunks: Vec<Option<Chunk>>,
    pending_dirty_background: Vec<bool>,
    pending_dirty_foreground: Vec<bool>,
    pending_dirty_overlay: Vec<bool>,
    chunk_alloc_cursor: usize,
    chunk_alloc_budget_per_frame: usize,
    chunk_rebuild_budget_per_frame: usize,
    chunk_allocs_this_frame: usize,
    chunk_rebuilds_this_frame: usize,
    structure_apply: Option<StructureApplyState>,
    structure_interactors: Vec<StructureInteractor>,
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
            chunks.push(Some(Chunk {
                background,
                foreground,
                overlay,
                dirty_background: true,
                dirty_foreground: true,
                dirty_overlay: true,
                ready_background: false,
                ready_foreground: false,
                ready_overlay: false,
            }));
        }

        let chunk_count = chunk_cols * chunk_rows;

        Self {
            width,
            height,
            tile_size,
            background: vec![EMPTY_TILE; len],
            foreground: vec![EMPTY_TILE; len],
            overlay: vec![EMPTY_TILE; len],
            solid: vec![false; len],
            collision_mask: vec![0; len],
            collision_blocks: Vec::new(),
            collision_dirty: true,
            chunk_cols,
            chunk_rows,
            chunk_pixel_size,
            chunks,
            pending_dirty_background: vec![false; chunk_count],
            pending_dirty_foreground: vec![false; chunk_count],
            pending_dirty_overlay: vec![false; chunk_count],
            chunk_alloc_cursor: 0,
            chunk_alloc_budget_per_frame: usize::MAX,
            chunk_rebuild_budget_per_frame: usize::MAX,
            chunk_allocs_this_frame: 0,
            chunk_rebuilds_this_frame: 0,
            structure_apply: None,
            structure_interactors: Vec::new(),
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

        let mut chunks = Vec::with_capacity(total_chunks);
        for _ in 0..total_chunks {
            chunks.push(None);
        }

        Self {
            width,
            height,
            tile_size,
            background: vec![EMPTY_TILE; len],
            foreground: vec![EMPTY_TILE; len],
            overlay: vec![EMPTY_TILE; len],
            solid: vec![false; len],
            collision_mask: vec![0; len],
            collision_blocks: Vec::new(),
            collision_dirty: true,
            chunk_cols,
            chunk_rows,
            chunk_pixel_size,
            chunks,
            pending_dirty_background: vec![true; total_chunks],
            pending_dirty_foreground: vec![true; total_chunks],
            pending_dirty_overlay: vec![true; total_chunks],
            chunk_alloc_cursor: 0,
            chunk_alloc_budget_per_frame: usize::MAX,
            chunk_rebuild_budget_per_frame: usize::MAX,
            chunk_allocs_this_frame: 0,
            chunk_rebuilds_this_frame: 0,
            structure_apply: None,
            structure_interactors: Vec::new(),
            grid_size,
            border_thickness,
        }
    }

    pub fn allocate_chunks_step(&mut self, time_budget_s: f32) -> bool {
        let budget = time_budget_s.max(0.0001) as f64;
        let start = get_time();
        let total = self.chunks.len();
        if total == 0 {
            return true;
        }

        let mut scanned = 0usize;
        while scanned < total && (get_time() - start) < budget {
            let idx = self.chunk_alloc_cursor;
            self.chunk_alloc_cursor = (self.chunk_alloc_cursor + 1) % total;
            scanned += 1;
            if self.chunks[idx].is_some() {
                continue;
            }
            self.create_chunk(idx);
        }

        self.chunks.iter().all(|chunk| chunk.is_some())
    }

    pub fn allocate_chunks_progress(&self) -> f32 {
        let total = (self.chunk_cols * self.chunk_rows).max(1) as f32;
        let done = self.chunks.iter().filter(|chunk| chunk.is_some()).count() as f32;
        (done / total).clamp(0.0, 1.0)
    }

    pub fn set_chunk_work_budget(&mut self, alloc_per_frame: usize, rebuild_per_frame: usize) {
        self.chunk_alloc_budget_per_frame = alloc_per_frame.max(1);
        self.chunk_rebuild_budget_per_frame = rebuild_per_frame.max(1);
    }

    pub fn begin_frame_chunk_work(&mut self) {
        self.chunk_allocs_this_frame = 0;
        self.chunk_rebuilds_this_frame = 0;
    }

    pub fn prewarm_visible_chunks(&mut self, camera_target: Vec2, camera_zoom: Vec2) {
        let (min_cx, max_cx, min_cy, max_cy) = self.visible_chunk_range(camera_target, camera_zoom);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let chunk_index = self.chunk_index(cx as usize, cy as usize);
                if !self.ensure_chunk_allocated(chunk_index) {
                    return;
                }
            }
        }
    }

    pub fn start_structure_apply(&mut self, defs: Vec<StructureDef>, seed: u32) {
        self.structure_interactors.clear();
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

    pub fn structure_interactors(&self) -> &[StructureInteractor] {
        &self.structure_interactors
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
        let mut bg_changed = false;
        let mut fg_changed = false;
        let mut ov_changed = false;

        for &(sx, sy, tile) in structure.background_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            if self.background[idx] != tile {
                self.background[idx] = tile;
                bg_changed = true;
            }
        }
        for &(sx, sy, tile) in structure.foreground_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            if self.foreground[idx] != tile {
                self.foreground[idx] = tile;
                fg_changed = true;
            }
        }
        for &(sx, sy, tile) in structure.overlay_updates.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            if self.overlay[idx] != tile {
                self.overlay[idx] = tile;
                ov_changed = true;
            }
        }
        for &(sx, sy, mask) in structure.collider_offsets.iter() {
            let tx = x + sx;
            let ty = y + sy;
            if tx >= max_x || ty >= max_y {
                continue;
            }
            let idx = self.idx(tx, ty);
            let next_mask = mask & 0x0F;
            if self.collision_mask[idx] != next_mask {
                self.collision_mask[idx] = next_mask;
                self.solid[idx] = next_mask != 0;
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
            bg_changed,
            fg_changed,
            ov_changed,
        );
    }

    fn place_structure_unchecked(&mut self, structure: &Structure, x: usize, y: usize) {
        let mut collision_changed = false;
        let mut bg_changed = false;
        let mut fg_changed = false;
        let mut ov_changed = false;

        for &(sx, sy, tile) in structure.background_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            if self.background[idx] != tile {
                self.background[idx] = tile;
                bg_changed = true;
            }
        }
        for &(sx, sy, tile) in structure.foreground_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            if self.foreground[idx] != tile {
                self.foreground[idx] = tile;
                fg_changed = true;
            }
        }
        for &(sx, sy, tile) in structure.overlay_updates.iter() {
            let idx = self.idx(x + sx, y + sy);
            if self.overlay[idx] != tile {
                self.overlay[idx] = tile;
                ov_changed = true;
            }
        }
        for &(sx, sy, mask) in structure.collider_offsets.iter() {
            let idx = self.idx(x + sx, y + sy);
            let next_mask = mask & 0x0F;
            if self.collision_mask[idx] != next_mask {
                self.collision_mask[idx] = next_mask;
                self.solid[idx] = next_mask != 0;
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
            bg_changed,
            fg_changed,
            ov_changed,
        );
    }

    pub fn apply_structures(&mut self, defs: &[StructureDef], seed: u32) {
        self.structure_interactors.clear();
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
                let def_seed = (def_index as u32).wrapping_mul(2654435761);
                let def_seed_y = (def_index as u32).wrapping_mul(2246822519);
                let rx = hash_u32(i as u32, seed ^ def_seed, 31);
                let ry = hash_u32(i as u32, seed ^ def_seed_y, 47);
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
                self.register_structure_interactors(def, x, y);
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

    fn register_structure_interactors(&mut self, def: &StructureDef, x: usize, y: usize) {
        if def.structure.interactor_offsets.is_empty() || def.on_interact.is_empty() {
            return;
        }
        let tile_size = self.tile_size;
        let mut rects: Vec<Rect> = Vec::new();
        for &(sx, sy, mask) in def.structure.interactor_offsets.iter() {
            let tile_x = (x + sx) as f32 * tile_size;
            let tile_y = (y + sy) as f32 * tile_size;
            let half_w = tile_size * 0.5;
            let half_h = tile_size * 0.5;

            if (mask & 0b0001) != 0 {
                rects.push(Rect::new(tile_x, tile_y, half_w, half_h));
            }
            if (mask & 0b0010) != 0 {
                rects.push(Rect::new(tile_x + half_w, tile_y, half_w, half_h));
            }
            if (mask & 0b0100) != 0 {
                rects.push(Rect::new(tile_x, tile_y + half_h, half_w, half_h));
            }
            if (mask & 0b1000) != 0 {
                rects.push(Rect::new(tile_x + half_w, tile_y + half_h, half_w, half_h));
            }
        }

        if rects.is_empty() {
            return;
        }
        let interact_range_world = def.interact_range * tile_size;

        let mut group = rects[0];
        for rect in rects.iter().skip(1) {
            group = merge_rect(group, *rect);
        }

        for rect in rects {
            self.structure_interactors.push(StructureInteractor {
                structure_id: def.id.clone(),
                rect,
                group_rect: group,
                on_interact: def.on_interact.clone(),
                interact_range_world,
            });
        }
    }

    pub fn fill_layer(&mut self, layer: LayerKind, id: u8) {
        let tiles = match layer {
            LayerKind::Background => &mut self.background,
            LayerKind::Foreground => &mut self.foreground,
            LayerKind::Overlay => &mut self.overlay,
        };
        if tiles.iter().all(|&tile| tile == id) {
            return;
        }
        tiles.fill(id);

        for cy in 0..self.chunk_rows {
            for cx in 0..self.chunk_cols {
                let chunk_index = self.chunk_index(cx, cy);
                if let Some(chunk) = self.chunks[chunk_index].as_mut() {
                    match layer {
                        LayerKind::Background => chunk.dirty_background = true,
                        LayerKind::Foreground => chunk.dirty_foreground = true,
                        LayerKind::Overlay => chunk.dirty_overlay = true,
                    }
                } else {
                    match layer {
                        LayerKind::Background => self.pending_dirty_background[chunk_index] = true,
                        LayerKind::Foreground => self.pending_dirty_foreground[chunk_index] = true,
                        LayerKind::Overlay => self.pending_dirty_overlay[chunk_index] = true,
                    }
                }
            }
        }
    }

    pub fn set_tile(&mut self, layer: LayerKind, x: usize, y: usize, id: u8) {
        let i = self.idx(x, y);
        let old = match layer {
            LayerKind::Background => self.background[i],
            LayerKind::Foreground => self.foreground[i],
            LayerKind::Overlay => self.overlay[i],
        };
        if old == id {
            return;
        }
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
        let next_mask = if solid { 0x0F } else { 0 };
        if self.solid[i] != solid || self.collision_mask[i] != next_mask {
            self.solid[i] = solid;
            self.collision_mask[i] = next_mask;
            self.collision_dirty = true;
        }
    }

    pub fn fill_collision(&mut self, solid: bool) {
        self.solid.fill(solid);
        self.collision_mask.fill(if solid { 0x0F } else { 0 });
        self.collision_dirty = true;
    }

    pub fn is_solid(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.solid[self.idx(x, y)]
    }

    pub fn set_collision_from_layer(&mut self, layer: LayerKind, solid_ids: &[u8]) {
        let mut max_id = 0u8;
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
                self.collision_mask[idx] = if solid { 0x0F } else { 0 };
            }
        }

        self.collision_dirty = true;
    }

    pub fn tile_at(&self, layer: LayerKind, x: usize, y: usize) -> u8 {
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
                if !self.ensure_chunk_allocated(chunk_index) {
                    continue;
                }
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
        if self.chunks.get(chunk_index).and_then(|c| c.as_ref()).is_none() {
            return;
        }
        let is_dirty = match layer {
            LayerKind::Background => self.chunks[chunk_index].as_ref().map(|c| c.dirty_background).unwrap_or(false),
            LayerKind::Foreground => self.chunks[chunk_index].as_ref().map(|c| c.dirty_foreground).unwrap_or(false),
            LayerKind::Overlay => self.chunks[chunk_index].as_ref().map(|c| c.dirty_overlay).unwrap_or(false),
        };

        if !is_dirty {
            return;
        }
        if self.chunk_rebuilds_this_frame >= self.chunk_rebuild_budget_per_frame {
            return;
        }

        let target = match layer {
            LayerKind::Background => self.chunks[chunk_index].as_ref().map(|c| c.background.clone()),
            LayerKind::Foreground => self.chunks[chunk_index].as_ref().map(|c| c.foreground.clone()),
            LayerKind::Overlay => self.chunks[chunk_index].as_ref().map(|c| c.overlay.clone()),
        };
        let Some(target) = target else {
            return;
        };

        self.render_chunk_layer(target, chunk_index, layer, tileset);
        self.chunk_rebuilds_this_frame += 1;

        let Some(chunk) = self.chunks[chunk_index].as_mut() else {
            return;
        };
        match layer {
            LayerKind::Background => {
                chunk.dirty_background = false;
                chunk.ready_background = true;
            }
            LayerKind::Foreground => {
                chunk.dirty_foreground = false;
                chunk.ready_foreground = true;
            }
            LayerKind::Overlay => {
                chunk.dirty_overlay = false;
                chunk.ready_overlay = true;
            }
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
        let chunk = match self.chunks.get(chunk_index).and_then(|c| c.as_ref()) {
            Some(chunk) => chunk,
            None => return,
        };
        let ready = match layer {
            LayerKind::Background => chunk.ready_background,
            LayerKind::Foreground => chunk.ready_foreground,
            LayerKind::Overlay => chunk.ready_overlay,
        };
        if !ready {
            return;
        }
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

    fn get_tile(&self, layer: LayerKind, x: usize, y: usize) -> u8 {
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
                let mask = self.collision_mask[self.idx(ux, uy)] & 0x0F;
                if mask == 0 {
                    continue;
                }
                let tile = self.tile_bounds(ux, uy);
                if mask == 0x0F {
                    out.push(tile);
                    continue;
                }
                let half_w = tile.w * 0.5;
                let half_h = tile.h * 0.5;
                if (mask & 0b0001) != 0 {
                    out.push(Rect::new(tile.x, tile.y, half_w, half_h));
                }
                if (mask & 0b0010) != 0 {
                    out.push(Rect::new(tile.x + half_w, tile.y, half_w, half_h));
                }
                if (mask & 0b0100) != 0 {
                    out.push(Rect::new(tile.x, tile.y + half_h, half_w, half_h));
                }
                if (mask & 0b1000) != 0 {
                    out.push(Rect::new(tile.x + half_w, tile.y + half_h, half_w, half_h));
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
                if let Some(chunk) = self.chunks[chunk_index].as_mut() {
                    if mark_background {
                        chunk.dirty_background = true;
                    }
                    if mark_foreground {
                        chunk.dirty_foreground = true;
                    }
                    if mark_overlay {
                        chunk.dirty_overlay = true;
                    }
                } else {
                    if mark_background {
                        self.pending_dirty_background[chunk_index] = true;
                    }
                    if mark_foreground {
                        self.pending_dirty_foreground[chunk_index] = true;
                    }
                    if mark_overlay {
                        self.pending_dirty_overlay[chunk_index] = true;
                    }
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
        if let Some(chunk) = self.chunks[chunk_index].as_mut() {
            match layer {
                LayerKind::Background => chunk.dirty_background = true,
                LayerKind::Foreground => chunk.dirty_foreground = true,
                LayerKind::Overlay => chunk.dirty_overlay = true,
            }
        } else {
            match layer {
                LayerKind::Background => self.pending_dirty_background[chunk_index] = true,
                LayerKind::Foreground => self.pending_dirty_foreground[chunk_index] = true,
                LayerKind::Overlay => self.pending_dirty_overlay[chunk_index] = true,
            }
        }
    }

    fn chunk_index(&self, cx: usize, cy: usize) -> usize {
        cy * self.chunk_cols + cx
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn ensure_chunk_allocated(&mut self, chunk_index: usize) -> bool {
        if self.chunks.get(chunk_index).and_then(|c| c.as_ref()).is_some() {
            return true;
        }
        if self.chunk_allocs_this_frame >= self.chunk_alloc_budget_per_frame {
            return false;
        }
        self.create_chunk(chunk_index);
        if self.chunks.get(chunk_index).and_then(|c| c.as_ref()).is_some() {
            self.chunk_allocs_this_frame += 1;
            true
        } else {
            false
        }
    }

    fn create_chunk(&mut self, chunk_index: usize) {
        let chunk_size_u32 = self.chunk_pixel_size.round().max(1.0) as u32;
        let background = render_target(chunk_size_u32, chunk_size_u32);
        let foreground = render_target(chunk_size_u32, chunk_size_u32);
        let overlay = render_target(chunk_size_u32, chunk_size_u32);
        background.texture.set_filter(FilterMode::Nearest);
        foreground.texture.set_filter(FilterMode::Nearest);
        overlay.texture.set_filter(FilterMode::Nearest);
        let dirty_background = self.pending_dirty_background.get(chunk_index).copied().unwrap_or(true);
        let dirty_foreground = self.pending_dirty_foreground.get(chunk_index).copied().unwrap_or(true);
        let dirty_overlay = self.pending_dirty_overlay.get(chunk_index).copied().unwrap_or(true);
        if let Some(slot) = self.chunks.get_mut(chunk_index) {
            *slot = Some(Chunk {
                background,
                foreground,
                overlay,
                dirty_background,
                dirty_foreground,
                dirty_overlay,
                ready_background: false,
                ready_foreground: false,
                ready_overlay: false,
            });
        }
        if let Some(flag) = self.pending_dirty_background.get_mut(chunk_index) {
            *flag = false;
        }
        if let Some(flag) = self.pending_dirty_foreground.get_mut(chunk_index) {
            *flag = false;
        }
        if let Some(flag) = self.pending_dirty_overlay.get_mut(chunk_index) {
            *flag = false;
        }
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

fn merge_rect(a: Rect, b: Rect) -> Rect {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = (a.x + a.w).max(b.x + b.w);
    let max_y = (a.y + a.h).max(b.y + b.h);
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

pub async fn load_structures_from_dir(dir: impl AsRef<Path>) -> Result<Vec<StructureDef>, std::io::Error> {
    let mut defs = Vec::new();

    if cfg!(target_arch = "wasm32") {
        let dir = data_path(&dir.as_ref().to_string_lossy());
        let files = load_wasm_manifest_files(&dir, &["tree_plains.json", "bush_plains.json"]).await;
        for file in files {
            let path = format!("{}/{}", dir, file);
            let raw_str = load_string(&path)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            let raw: StructureFile = serde_json::from_str(&raw_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let tile_len = raw.width * raw.height;
            let colliders = normalized_collider_pins(raw.colliders, tile_len);
            let interactors = normalized_collider_pins(raw.interactors, tile_len);
            let structure = Structure::new(
                raw.width,
                raw.height,
                raw.background,
                raw.foreground,
                raw.overlay,
                colliders,
                interactors,
            );

            defs.push(StructureDef {
                id: raw.id,
                structure,
                on_interact: raw.on_interact.unwrap_or_default(),
                interact_range: raw.interact_range.unwrap_or(0.0).max(0.0),
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
        if path.file_name().and_then(|n| n.to_str()) == Some("index.json") {
            continue;
        }
        let raw: StructureFile = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tile_len = raw.width * raw.height;
        let colliders = normalized_collider_pins(raw.colliders, tile_len);
        let interactors = normalized_collider_pins(raw.interactors, tile_len);
        let structure = Structure::new(
            raw.width,
            raw.height,
            raw.background,
            raw.foreground,
            raw.overlay,
            colliders,
            interactors,
        );

        defs.push(StructureDef {
            id: raw.id,
            structure,
            on_interact: raw.on_interact.unwrap_or_default(),
            interact_range: raw.interact_range.unwrap_or(0.0).max(0.0),
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
    background: Vec<u8>,
    #[serde(default)]
    foreground: Vec<u8>,
    #[serde(default)]
    overlay: Vec<u8>,
    #[serde(default)]
    colliders: Option<ColliderPinsFile>,
    #[serde(default)]
    interactors: Option<ColliderPinsFile>,
    #[serde(default)]
    on_interact: Option<Vec<String>>,
    #[serde(default)]
    interact_range: Option<f32>,
    #[serde(default)]
    frequency: Option<f32>,
    #[serde(default)]
    max_per_map: Option<usize>,
    #[serde(default)]
    min_distance: Option<f32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ColliderPinsFile {
    Bool(Vec<bool>),
    Pins(Vec<u8>),
}

fn normalized_collider_pins(raw: Option<ColliderPinsFile>, tile_len: usize) -> Vec<u8> {
    let mut out = match raw {
        Some(ColliderPinsFile::Pins(v)) => v.into_iter().map(|m| m & 0x0F).collect(),
        Some(ColliderPinsFile::Bool(v)) => v
            .into_iter()
            .map(|solid| if solid { 0x0F } else { 0 })
            .collect(),
        None => Vec::new(),
    };

    if out.len() != tile_len {
        out = vec![0; tile_len];
    }
    out
}
