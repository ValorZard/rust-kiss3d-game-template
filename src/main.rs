use crate::{
    asset_fetch::fetch_asset_bytes,
    game::{ENEMY_POSITION, GameLogic, MoveInputState},
    timestepper::FixedTimestepper,
};
use kiss3d::{egui, prelude::*};

mod asset_fetch;
mod game;
mod timestepper;

/// A 2D camera that can be zoomed and panned.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MyCamera2d {
    at: Vec2,
    /// Distance from the camera to the `at` focus point.
    zoom: f32,

    /// Increment of the zoom per unit scrolling. The default value is 40.0.
    zoom_step: f32,
    zoom_modifier: Option<Modifiers>,

    view: Mat3,
    proj: Mat3,
    scaled_proj: Mat3,
    inv_scaled_proj: Mat3,
    last_cursor_pos: Vec2,
}

impl Default for MyCamera2d {
    fn default() -> Self {
        Self::new(Vec2::ZERO, 1.0)
    }
}

impl MyCamera2d {
    /// Create a new arc-ball camera.
    pub fn new(eye: Vec2, zoom: f32) -> MyCamera2d {
        let mut res = MyCamera2d {
            at: eye,
            zoom,
            zoom_step: 0.9,
            zoom_modifier: None,
            view: Mat3::IDENTITY,
            proj: Mat3::IDENTITY,
            scaled_proj: Mat3::IDENTITY,
            inv_scaled_proj: Mat3::IDENTITY,
            last_cursor_pos: Vec2::ZERO,
        };

        res.update_projviews();

        res
    }

    /// The point the arc-ball is looking at.
    pub fn at(&self) -> Vec2 {
        self.at
    }

    /// Get a mutable reference to the point the camera is looking at.
    pub fn set_at(&mut self, at: Vec2) {
        self.at = at;
        self.update_projviews();
    }

    /// Gets the zoom of the camera.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Sets the zoom of the camera.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        self.update_restrictions();
        self.update_projviews();
    }

    /// Gets the zoom step of the camera.
    pub fn zoom_step(&self) -> f32 {
        self.zoom_step
    }

    /// Sets the zoom step of the camera.
    pub fn set_zoom_step(&mut self, new_zoom_step: f32) {
        self.zoom_step = new_zoom_step;
    }

    /// Move the camera such that it is centered on a specific point.
    pub fn look_at(&mut self, at: Vec2, zoom: f32) {
        self.at = at;
        self.zoom = zoom;
        self.update_projviews();
    }

    /// Transformation applied by the camera without perspective.
    fn update_restrictions(&mut self) {
        if self.zoom < 0.00001 {
            self.zoom = 0.00001
        }
    }

    /// The modifier used to zoom the PanZoomCamera2d camera.
    pub fn zoom_modifier(&self) -> Option<Modifiers> {
        self.zoom_modifier
    }

    /// Set the modifier used to zoom the PanZoomCamera2d camera.
    pub fn rebind_zoom_modifier(&mut self, new_modifier: Option<Modifiers>) {
        self.zoom_modifier = new_modifier;
    }

    fn update_projviews(&mut self) {
        // Create translation matrix: translate by -at
        self.view = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(-self.at.x, -self.at.y, 1.0),
        );

        self.scaled_proj = self.proj;
        // Scale x and y components (first two diagonal elements)
        self.scaled_proj.col_mut(0)[0] *= self.zoom;
        self.scaled_proj.col_mut(1)[1] *= self.zoom;

        self.inv_scaled_proj.col_mut(0)[0] = 1.0 / self.scaled_proj.col(0)[0];
        self.inv_scaled_proj.col_mut(1)[1] = 1.0 / self.scaled_proj.col(1)[1];
    }
}

impl Camera2d for MyCamera2d {
    fn handle_event(&mut self, _canvas: &Canvas, event: &WindowEvent) {
        let scale = 1.0; // canvas.scale_factor();

        match *event {
            WindowEvent::FramebufferSize(w, h) => {
                self.proj = Mat3::from_cols(
                    Vec3::new(2.0 * (scale as f32) / (w as f32), 0.0, 0.0),
                    Vec3::new(0.0, 2.0 * (scale as f32) / (h as f32), 0.0),
                    Vec3::new(0.0, 0.0, 1.0),
                );
                self.update_projviews();
            }
            _ => {}
        }
    }

    #[inline]
    fn view_transform_pair(&self) -> (Mat3, Mat3) {
        (self.view, self.scaled_proj)
    }

    fn update(&mut self, _: &Canvas) {}

    /// Calculate the global position of the given window coordinate
    fn unproject(&self, window_coord: Vec2, size: Vec2) -> Vec2 {
        // Convert window coordinates (origin at top left) to normalized screen coordinates
        // (origin at the center of the screen)
        let normalized_coords = Vec2::new(
            2.0 * window_coord.x / size.x - 1.0,
            2.0 * -window_coord.y / size.y + 1.0,
        );

        // Project normalized screen coordinate to screen space
        let normalized_homogeneous = Vec3::new(normalized_coords.x, normalized_coords.y, 1.0);
        let unprojected_homogeneous = self.inv_scaled_proj * normalized_homogeneous;

        // Convert from screen space to global space
        let screen_pos = unprojected_homogeneous.xy() / unprojected_homogeneous.z;
        screen_pos + self.at
    }
}

#[kiss3d::main]
async fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
    }
    let mut window = Window::new("Kiss3d: rectangle").await;
    let mut texture_manager = TextureManager::new();

    let mut camera = MyCamera2d::new(Vec2::ZERO, 5.0);
    let mut scene = SceneNode2d::empty();
    let mut player = scene.add_rectangle(10.0, 10.0).set_color(RED);

    let sheet = SpriteSheet::new(9, 3);
    let characters_bytes = fetch_asset_bytes("characters.png")
        .await
        .expect("should be able to fetch characters.png");
    let characters_texture =
        texture_manager.add_image_from_memory_pixelated(&characters_bytes, "characters");

    let frames_of_box_guy: [u32; 2] = [11, 12];
    let mut current_frame_index: usize = 0;
    let enemy_animation_length = 12;
    let mut enemy_node = scene.add_sprite(30., 30.);
    enemy_node.set_position(ENEMY_POSITION);
    enemy_node.set_texture(characters_texture);
    enemy_node.set_sprite_frame(&sheet, frames_of_box_guy[current_frame_index]);

    // timer stuff
    let mut timestepper = FixedTimestepper::default();

    // input state
    let mut input = MoveInputState::default();

    // game state
    let mut game_logic = GameLogic::new();

    // TODO: dynamically set and remove game rectangles, but for now we can just assume there's only a set number right now
    let rectangles = game_logic.get_static_platforms_with_position();
    for (rect, position) in rectangles {
        let mut scene_rect = scene
            .add_rectangle(rect.width, rect.height)
            .set_color(YELLOW);
        // Draw at the collider's actual center (in pixel space) so the visible
        // box lines up with where collision really happens.
        scene_rect.set_position(position);
    }

    let mut frame = 0u32;
    while window.render_2d(&mut scene, &mut camera).await {
        // the way OS's poll key inputs mean that there's a frame of waiting before sending in the next key input
        // see: https://stereopsis.com/keyrepeat/
        for event in window.events().iter() {
            match event.value {
                WindowEvent::Key(Key::Left, Action::Press, _) => {
                    input.left = true;
                }
                WindowEvent::Key(Key::Right, Action::Press, _) => {
                    input.right = true;
                }
                WindowEvent::Key(Key::Up, Action::Press, _) => {
                    input.up = true;
                }
                WindowEvent::Key(Key::Down, Action::Press, _) => {
                    input.down = true;
                }
                WindowEvent::Key(Key::Left, Action::Release, _) => {
                    input.left = false;
                }
                WindowEvent::Key(Key::Right, Action::Release, _) => {
                    input.right = false;
                }
                WindowEvent::Key(Key::Up, Action::Release, _) => {
                    input.up = false;
                }
                WindowEvent::Key(Key::Down, Action::Release, _) => {
                    input.down = false;
                }
                _ => {}
            }
        }

        // run actual game logic if we've hit a tick (drain accumulated frames)
        while timestepper.step() {
            game_logic.update_position_with_input(&input);
            game_logic.step_physics();
            // A few times per second, flip every character between its two frames.

            if frame.is_multiple_of(enemy_animation_length) {
                current_frame_index =
                    ((frame / enemy_animation_length) % frames_of_box_guy.len() as u32) as usize;
                enemy_node.set_sprite_frame(&sheet, frames_of_box_guy[current_frame_index]);
            }
            frame = frame.wrapping_add(1);
        }

        // render sprites
        player.set_position(game_logic.get_player_position());

        // Draw UI
        window.draw_ui(|ctx| {
            egui::Window::new("Kiss3d egui Example")
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Current Frame Time {}",
                        timestepper.get_time_since_last_step()
                    ));
                });
        });
    }
}
