use std::time::Duration;

use kiss3d::glamx::Vec2;
use rapier2d::{control::KinematicCharacterController, prelude::*};

/// 60 TPS (ish)
pub const GAME_TIME_STEP_MS: u32 = 17;
// this is fine to cast as a u64, we know the time step isn't negative
pub const GAME_TIME_STEP: Duration = Duration::from_millis(GAME_TIME_STEP_MS as u64);
pub const GAME_TIME_DELTA: f32 = GAME_TIME_STEP.as_secs_f32();

pub const PHYSICS_TO_PIXEL_SCALE: f32 = 50.0; // 1 meter in physics engine equals 50 pixels
pub const PIXEL_TO_PHYSICS_SCALE: f32 = 1.0 / PHYSICS_TO_PIXEL_SCALE;
pub const PLAYER_GAME_WIDTH: f32 = 10.; // game scale
pub const PLAYER_GAME_HEIGHT: f32 = 10.;
pub const PLAYER_GAME_RECTANGLE_EXTENTS: GameRectangleExtents =
    GameRectangleExtents::new(PLAYER_GAME_WIDTH, PLAYER_GAME_HEIGHT);

pub const PLAYER_TRAPPED_BOX_WIDTH: f32 = 100.;
pub const PLAYER_TRAPPED_BOX_HEIGHT: f32 = 50.;
pub const PLAYER_TRAPPED_BOX_WALL_WIDTH: f32 = 10.;

pub const PLAYER_STARTING_POSITION: Vec2 = Vec2::new(0., 0.);
/// Player movement speed in game (pixel) space, i.e. pixels per second.
pub const PLAYER_SPEED: f32 = 100.;
/// Where the enemy sits in game (pixel) space; projectiles spawn from here.
pub const ENEMY_POSITION: Vec2 = Vec2::new(0.0, 70.0);

// Collision groups: players collide with obstacles but not with each other
pub const PLAYER_GROUP: Group = Group::GROUP_1;
pub const STATIC_PLATFORM_GROUP: Group = Group::GROUP_2;

pub fn convert_vec2_pixel_to_physics(position: Vec2) -> Vec2 {
    Vec2 {
        x: position.x * PIXEL_TO_PHYSICS_SCALE,
        y: position.y * PIXEL_TO_PHYSICS_SCALE,
    }
}

pub fn convert_vec2_physics_to_pixel(position: Vec2) -> Vec2 {
    Vec2 {
        x: position.x * PHYSICS_TO_PIXEL_SCALE,
        y: position.y * PHYSICS_TO_PIXEL_SCALE,
    }
}

// tag
pub struct StaticPlatform {}

// this is the rectangles height and width is displayed in the game world
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct GameRectangleExtents {
    pub width: f32,
    pub height: f32,
}

impl From<Aabb> for GameRectangleExtents {
    fn from(aabb: Aabb) -> Self {
        let min = convert_vec2_physics_to_pixel(aabb.mins);
        let max = convert_vec2_physics_to_pixel(aabb.maxs);
        GameRectangleExtents {
            width: max.x - min.x,
            height: max.y - min.y,
        }
    }
}

impl GameRectangleExtents {
    pub const fn new(width: f32, height: f32) -> Self {
        GameRectangleExtents { width, height }
    }

    pub const fn get_half_extents_for_physics(&self) -> Vec2 {
        Vec2::new(
            (self.width * 0.5) * PIXEL_TO_PHYSICS_SCALE,
            (self.height * 0.5) * PIXEL_TO_PHYSICS_SCALE,
        )
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MoveInputState {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}
impl Default for MoveInputState {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
        }
    }
}

impl MoveInputState {
    pub fn as_normalized_vec(&self) -> Vec2 {
        Vec2::new(
            (self.right as i8 - self.left as i8) as f32,
            (self.up as i8 - self.down as i8) as f32,
        )
        .normalize_or_zero()
    }
}

pub struct GameLogic {
    world: hecs::World,
    physics: PhysicsWorld,
    player: hecs::Entity,
}

/// GameLogic should NEVER leak any physics space vectors outside of the physics world.
/// As far as the rest of the codebase is concerned, it's ONLY worried about pixel (game) space.
impl GameLogic {
    pub fn new() -> Self {
        // TODO: for now, we hardcode left and right side players and require there to be only one of each
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new();
        // The physics default timestep is 1/60s but the game ticks at 30 TPS;
        // velocity-driven bodies (projectiles) would move at half speed if the
        // integration step didn't match the game tick.
        physics.integration_parameters.dt = GAME_TIME_DELTA;

        // Distinct starting points on opposite sides, clear of the obstacle (spawned
        // below) and of each other, so neither player starts already overlapping it.
        let player = Self::spawn_player(&mut world, &mut physics, PLAYER_STARTING_POSITION);

        // spawn collision box for the players to be trapped in while bullets are shooting at them
        // left wall
        Self::spawn_static_platform(
            &mut world,
            &mut physics,
            -PLAYER_TRAPPED_BOX_WIDTH / 2.,
            0.,
            PLAYER_TRAPPED_BOX_WALL_WIDTH,
            PLAYER_TRAPPED_BOX_HEIGHT,
        );
        // right wall
        Self::spawn_static_platform(
            &mut world,
            &mut physics,
            PLAYER_TRAPPED_BOX_WIDTH / 2.,
            0.,
            PLAYER_TRAPPED_BOX_WALL_WIDTH,
            PLAYER_TRAPPED_BOX_HEIGHT,
        );
        // top wall
        Self::spawn_static_platform(
            &mut world,
            &mut physics,
            0.,
            PLAYER_TRAPPED_BOX_HEIGHT / 2.,
            PLAYER_TRAPPED_BOX_WIDTH,
            PLAYER_TRAPPED_BOX_WALL_WIDTH,
        );
        // bottom wall
        Self::spawn_static_platform(
            &mut world,
            &mut physics,
            0.,
            -PLAYER_TRAPPED_BOX_HEIGHT / 2.,
            PLAYER_TRAPPED_BOX_WIDTH,
            PLAYER_TRAPPED_BOX_WALL_WIDTH,
        );

        // The broad-phase spatial index used by scene queries (incl. the character
        // controller's move_shape) isn't populated until a step runs; without this,
        // the very first update_position_with_input call queries a stale/empty index
        // and misses newly-inserted colliders entirely.
        physics.step();

        Self {
            world,
            player,
            physics,
        }
    }

    fn spawn_static_platform(
        world: &mut hecs::World,
        physics: &mut PhysicsWorld,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        // spawn physics object for players to collide with
        // create a rectangle in the game world
        let rectangle = GameRectangleExtents::new(width, height);
        // GameRectangle's x/y is its center (matching kiss3d's center-based
        // set_position), so the physics collider sits at the same point.
        let physics_position = Vec2::new(x, y) * PIXEL_TO_PHYSICS_SCALE;
        let rectangle_half_extents = rectangle.get_half_extents_for_physics();
        let collider = ColliderBuilder::cuboid(rectangle_half_extents.x, rectangle_half_extents.y)
            .collision_groups(InteractionGroups::new(
                STATIC_PLATFORM_GROUP,
                PLAYER_GROUP,
                InteractionTestMode::And,
            ))
            .position(Pose2 {
                rotation: Rot2::default(),
                translation: physics_position,
            })
            .build();
        let collider_handle = physics.colliders.insert(collider);
        world.spawn((rectangle, StaticPlatform {}, collider_handle));
    }

    fn spawn_player(
        world: &mut hecs::World,
        physics: &mut PhysicsWorld,
        spawn_position: Vec2,
    ) -> hecs::Entity {
        let rigid_body = RigidBodyBuilder::kinematic_position_based()
            .translation(convert_vec2_pixel_to_physics(spawn_position))
            .build();

        let half_extents = PLAYER_GAME_RECTANGLE_EXTENTS.get_half_extents_for_physics();

        let collider = ColliderBuilder::cuboid(half_extents.x, half_extents.y)
            .collision_groups(InteractionGroups::new(
                PLAYER_GROUP,
                STATIC_PLATFORM_GROUP,
                InteractionTestMode::And,
            ))
            .build();

        let (body_handle, collider_handle) = physics.insert(rigid_body, collider);

        // Top-down game, no floor/gravity, so ground-snapping doesn't apply here.
        let controller = KinematicCharacterController {
            snap_to_ground: None,
            ..Default::default()
        };

        world.spawn((
            body_handle,
            collider_handle,
            controller,
            PLAYER_GAME_RECTANGLE_EXTENTS.clone(),
        ))
    }

    pub fn get_static_platforms_with_position(&mut self) -> Vec<(&GameRectangleExtents, Vec2)> {
        // The collider is the source of truth for where the wall sits, so read
        // its position straight from the physics world (converted to pixel space)
        // rather than duplicating it as a separate component.
        let colliders = &self.physics.colliders;
        self.world
            .query_mut::<(&GameRectangleExtents, &ColliderHandle, &StaticPlatform)>()
            .into_iter()
            .map(|(rectangle, collider_handle, _)| {
                let position = convert_vec2_physics_to_pixel(
                    colliders[*collider_handle].position().translation,
                );
                (rectangle, position)
            })
            .collect()
    }

    fn get_player_physics_data(&mut self) -> (&RigidBodyHandle, &KinematicCharacterController) {
        self.world
            .query_one_mut::<(&RigidBodyHandle, &KinematicCharacterController)>(self.player)
            .expect("Player should exist here")
    }

    pub fn setup_game(&mut self) {
        // Easier way to do this is to just totally reset all state
        *self = Self::new();
    }

    // returns position in game space not physics space
    pub fn update_position_with_input(&mut self, input: &MoveInputState) -> Vec2 {
        let (handle, controller) = self.get_player_physics_data();
        let handle = *handle;
        let controller = *controller;
        // following code is based off of:
        // https://github.com/dimforge/rapier/blob/c13133ad293ee70c7f9cec9e498eac016c362169/examples2d/utils/character.rs#L125

        let character_body = &self.physics.bodies[handle];
        let character_collider = &self.physics.colliders[character_body.colliders()[0]];
        let character_rotation = character_collider.position().rotation;
        let character_shape = character_collider.shared_shape().clone();
        let character_mass = character_body.mass();
        // this is basically like physics masks in other game engines
        // so who can collide with the character collider, and what the character collider can collide against
        // (those may not be the same thing.)
        let character_groups = character_collider.collision_groups();

        // Reconciliation may apply multiple inputs before a physics step;
        // use the pending kinematic target so updates accumulate deterministically.
        let current_position = character_body.next_position().translation;
        // PLAYER_SPEED is px/s, we need to convert it to rapier's format
        let desired_movement =
            input.as_normalized_vec() * PLAYER_SPEED * PIXEL_TO_PHYSICS_SCALE * GAME_TIME_DELTA;

        // check all collisions in the physics world against this character body
        let mut query_pipeline = self.physics.broad_phase.as_query_pipeline_mut(
            self.physics.narrow_phase.query_dispatcher(),
            &mut self.physics.bodies,
            &mut self.physics.colliders,
            QueryFilter::new()
                .exclude_rigid_body(handle)
                .groups(character_groups),
        );

        let mut collisions = vec![];
        let movement = controller.move_shape(
            GAME_TIME_DELTA,
            &query_pipeline.as_ref(),
            &*character_shape,
            &Pose2 {
                rotation: character_rotation,
                translation: current_position,
            },
            desired_movement,
            |c| collisions.push(c),
        );

        controller.solve_character_collision_impulses(
            GAME_TIME_DELTA,
            &mut query_pipeline,
            &*character_shape,
            character_mass,
            &collisions,
        );

        let new_pos = current_position + movement.translation;

        // Kinematic bodies don't move until you tell the physics step
        // where they're going next.
        self.physics.bodies[handle].set_next_kinematic_translation(new_pos);
        let new_pixel_position = convert_vec2_physics_to_pixel(new_pos);
        new_pixel_position
    }

    /// `new_position` is in game (pixel) space, like every other public position.
    pub fn update_position_with_vec(&mut self, new_position: Vec2) -> Vec2 {
        let (handle, _) = self.get_player_physics_data();
        let handle = handle.clone();
        let body = &mut self.physics.bodies[handle];
        body.set_next_kinematic_translation(convert_vec2_pixel_to_physics(new_position));
        new_position
    }

    /// Returns position in game (pixel) space, not physics space.
    pub fn get_player_position(&mut self) -> Vec2 {
        let (handle, _) = self.get_player_physics_data();
        let handle = handle.clone();
        let body = &mut self.physics.bodies[handle];
        body.position().translation * PHYSICS_TO_PIXEL_SCALE
    }

    /// Advances the physics simulation. Call once per tick, after inputs
    /// have been applied via update_position_with_input/_vec.
    pub fn step_physics(&mut self) {
        self.physics.step();
    }
}
