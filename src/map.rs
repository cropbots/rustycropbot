use macroquad::prelude::*;

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

pub struct TileSet {
    tiles: Vec<Texture2D>,
}

impl TileSet {
    pub async fn load(dir: &str, count: usize) -> Self {
        let mut tiles = Vec::with_capacity(count);
        for i in 0..count {
            let path = format!("{}/{}.png", dir, i);
            let tex = load_texture(&path)
                .await
                .unwrap_or_else(|err| panic!("Failed to load {}: {}", path, err));
            tex.set_filter(FilterMode::Nearest);
            tiles.push(tex);
        }
        Self { tiles }
    }

    fn get(&self, id: u16) -> Option<&Texture2D> {
        if id == EMPTY_TILE {
            return None;
        }
        self.tiles.get(id as usize)
    }

    pub fn count(&self) -> usize {
        self.tiles.len()
    }
}

pub struct Structure {
    width: usize,
    height: usize,
    background: Vec<u16>,
    foreground: Vec<u16>,
    overlay: Vec<u16>,
}

impl Structure {
    pub fn random(width: usize, height: usize, tile_count: usize, seed: u32) -> Self {
        let len = width * height;
        let mut background = vec![EMPTY_TILE; len];
        let mut foreground = vec![EMPTY_TILE; len];
        let mut overlay = vec![EMPTY_TILE; len];
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

        Self {
            width,
            height,
            background,
            foreground,
            overlay,
        }
    }

    pub fn new(
        width: usize,
        height: usize,
        background: Vec<u16>,
        foreground: Vec<u16>,
        overlay: Vec<u16>,
    ) -> Self {
        Self {
            width,
            height,
            background,
            foreground,
            overlay,
        }
    }

    fn tile_at(&self, layer: LayerKind, x: usize, y: usize) -> u16 {
        let i = y * self.width + x;
        match layer {
            LayerKind::Background => self.background[i],
            LayerKind::Foreground => self.foreground[i],
            LayerKind::Overlay => self.overlay[i],
        }
    }
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
    grid_size: Vec2,
}

impl TileMap {
    pub fn demo(width: usize, height: usize, tile_size: f32, tile_count: usize) -> Self {
        let mut map = Self::new(width, height, tile_size, Vec2::new(tile_size, tile_size));

        if tile_count > 0 {
            map.fill_layer(LayerKind::Background, 24);
        }

        map
    }

    pub fn new(width: usize, height: usize, tile_size: f32, grid_size: Vec2) -> Self {
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
            grid_size,
        }
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
        for sy in 0..structure.height {
            for sx in 0..structure.width {
                let tx = x + sx;
                let ty = y + sy;
                if tx >= self.width || ty >= self.height {
                    continue;
                }
                let bg = structure.tile_at(LayerKind::Background, sx, sy);
                let fg = structure.tile_at(LayerKind::Foreground, sx, sy);
                let ov = structure.tile_at(LayerKind::Overlay, sx, sy);

                if bg != EMPTY_TILE {
                    self.set_tile(LayerKind::Background, tx, ty, bg);
                }
                if fg != EMPTY_TILE {
                    self.set_tile(LayerKind::Foreground, tx, ty, fg);
                }
                if ov != EMPTY_TILE {
                    self.set_tile(LayerKind::Overlay, tx, ty, ov);
                }
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
                let Some(tex) = tileset.get(tile) else {
                    continue;
                };

                let local_x = (tx - origin_x) as f32 * self.tile_size;
                let local_y = (ty - origin_y) as f32 * self.tile_size;
                draw_texture_ex(
                    tex,
                    local_x,
                    local_y,
                    WHITE,
                    DrawTextureParams {
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
                if self.is_solid(ux, uy) {
                    hitboxes.push(self.tile_bounds(ux, uy));
                }
            }
        }

        hitboxes
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

fn hash_u32(x: u32, y: u32, seed: u32) -> u32 {
    let mut v = x.wrapping_mul(0x9E3779B1) ^ y.wrapping_mul(0x85EBCA6B) ^ seed;
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB352D);
    v ^= v >> 15;
    v
}
