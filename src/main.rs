use std::f32::consts::PI;
use std::time::Duration;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use rand::prelude::*;

const ASTEROID_SPAWN_RADIUS_DISTANCE: f32 = 800.0;
const ASTEROID_RADIUS: f32 = 10.0;
const ASTEROID_SPEED: f32 = 1200.0; // by second
const ASTEROID_SPAWN_TIME: u64 = 10; // in second

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Msaa::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .insert_resource(RapierConfiguration { gravity: Vec2::ZERO, ..default() });

    #[cfg(feature = "debug-render")]
    app.add_plugin(RapierDebugRenderPlugin::default());

    app.add_startup_system(setup_graphics)
        .add_startup_system(setup_planet)
        .add_startup_system(setup_asteroid_spawning)
        .add_system(spawn_asteroids)
        .add_system(move_asteroids)
        .run();
}

fn setup_graphics(mut commands: Commands) {
    commands.spawn_bundle(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 20.0, 0.0),
        ..default()
    });
}

/// Configure the main planet to defend
fn setup_planet(mut commands: Commands) {
    // Planet Earth
    let planet_radius = 50.0;

    commands
        .spawn_bundle(TransformBundle::from(Transform::default()))
        .insert(Planet)
        .insert(Collider::ball(planet_radius));
}

/// Configure our asteroid spawning algorithm
fn setup_asteroid_spawning(mut commands: Commands) {
    commands.insert_resource(AsteroidSpawnConfig {
        // create the repeating timer
        timer: Timer::new(Duration::from_secs(ASTEROID_SPAWN_TIME), true),
    })
}

fn spawn_asteroids(
    mut commands: Commands,
    time: Res<Time>,
    planet: Query<&Transform, With<Planet>>,
    mut config: ResMut<AsteroidSpawnConfig>,
) {
    config.timer.tick(time.delta());

    if config.timer.finished() {
        let planet_transform = planet.single();
        let planet_translation = planet_transform.translation;

        let angle = random::<f32>() * PI * 2.0;
        let x = angle.cos() * ASTEROID_SPAWN_RADIUS_DISTANCE + planet_translation.x;
        let y = angle.sin() * ASTEROID_SPAWN_RADIUS_DISTANCE + planet_translation.y;

        commands
            .spawn_bundle(TransformBundle::from(Transform::from_xyz(x, y, 0.0)))
            .insert(Asteroid)
            .insert(RigidBody::Dynamic)
            .insert(Collider::ball(ASTEROID_RADIUS))
            .insert(Velocity::default())
            .insert(Sleeping::disabled());
    }
}

fn move_asteroids(
    time: Res<Time>,
    planet: Query<&Transform, With<Planet>>,
    mut asteroids: Query<(&Transform, &mut Velocity), With<Asteroid>>,
) {
    let planet_transform = planet.single();

    for (transform, mut velocity) in &mut asteroids {
        let diff = planet_transform.translation - transform.translation;
        let direction = diff.normalize_or_zero();
        velocity.linvel = direction.xy() * ASTEROID_SPEED * time.delta_seconds();
    }
}

#[derive(Component, Debug)]
struct Asteroid;

struct AsteroidSpawnConfig {
    /// How often to spawn a new asteroid (repeating timer)
    timer: Timer,
}

#[derive(Component, Debug)]
struct Planet;
