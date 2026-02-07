use macroquad::prelude::*;

mod map;
mod player;
mod helpers;
mod entity;
mod r#trait;

use map::{TileMap, TileSet};
use player::Player;

const CAMERA_DRAG: f32 = 5.0;
const TILE_COUNT: usize = 223;
const TILE_SIZE: f32 = 16.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Cropbots".to_owned(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut player = Player::new(
        vec2(200.0, 300.0 + 16.0 / 2.0),
        helpers::load_single_texture("src/assets/objects", "player08").await.unwrap(),
        Rect::new(-6.5 / 2.0, -8.0, 6.5, 8.0)
    );

    let tileset = TileSet::load("src/assets/tiles", TILE_COUNT).await;
    let mut map = TileMap::demo(512, 512, TILE_SIZE, tileset.count());

    let mut camera = Camera2D {
        target: player.position(),
        zoom: vec2(1.0 / 400.0, 1.0 / 300.0),
        ..Default::default()
    };

    let mut i: f32 = 0.0;
    let mut fps: i32 = 0;
    loop {
        player.update();

        camera.zoom = vec2(6.0 / screen_width(), 6.0 / screen_height());
        let follow = 1.0 - (-CAMERA_DRAG * get_frame_time()).exp();
        camera.target += (player.position() - camera.target) * follow;

        set_camera(&camera);
        clear_background(LIGHTGRAY);

        map.draw_background(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );
        map.draw_foreground(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );
        player.draw();
        map.draw_overlay(
            &tileset,
            camera.target,
            camera.zoom,
            screen_width(),
            screen_height(),
        );

        set_default_camera();

        i += get_frame_time();
        if i >= 1.0 {
            fps = get_fps();
            i = 0.0;
        } 
        draw_text(
            &format!("FPS: {:.0}", fps),
            20.0,
            40.0,
            30.0, // font size
            WHITE
        );

        next_frame().await;
    }
}
