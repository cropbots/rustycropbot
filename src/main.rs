use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "FPS Unlimited Demo".to_owned(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut camera = Camera2D {
        target: vec2(0.0, 0.0),   // what the camera looks at (world coords)
        zoom: vec2(1.0 / 400.0, 1.0 / 300.0),
        ..Default::default()
    };
    set_camera(&camera);
    let mut x = 200.0;
    let mut y = 300.0;
    let mut vx = 0.0;
    let mut vy = 0.0;

    loop {
        clear_background(LIGHTGRAY);

        if is_key_down(KeyCode::D) 
        { vx += 0.3; }
        if is_key_down(KeyCode::A) 
        { vx -= 0.3; }
        if is_key_down(KeyCode::W) 
        { vy -= 0.3; }
        if is_key_down(KeyCode::S)  
        { vy += 0.3; }

        x += vx;
        y += vy;

        camera.target = vec2(x, y);

        vx *= 0.9;
        vy *= 0.9;
        draw_text(
            &format!("FPS: {:.0}", get_fps()),
            20.0,
            40.0,
            30.0, // font size
            WHITE
        );

        draw_circle(camera.target.x + vx, camera.target.y + vy, 30.0, RED);

        next_frame().await;
    }
}
