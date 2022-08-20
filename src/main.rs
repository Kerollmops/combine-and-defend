use std::f32::consts::PI;
use std::time::Duration;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_tweening::lens::TransformRotateZLens;
use bevy_tweening::*;
use ordered_float::OrderedFloat;
use rand::prelude::*;

const ASTEROID_SPAWN_RADIUS_DISTANCE: f32 = 800.0;
const ASTEROID_RADIUS: f32 = 10.0;
const ASTEROID_SPEED: f32 = 1200.0; // by second
const ASTEROID_SPAWN_TIME: u64 = 1; // in second

const SHIP_SPEED: f32 = 2400.0; // by second
const SHIP_TRIGGER_MAX_DISTANCE: f32 = 400.0;
const SHIP_BUMP_FORCE: f32 = 400.0;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugin(TweeningPlugin)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Msaa::default())
        .init_collection::<ImageAssets>()
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .insert_resource(RapierConfiguration { gravity: Vec2::ZERO, ..default() });

    #[cfg(feature = "debug-render")]
    app.add_plugin(RapierDebugRenderPlugin::default());

    app.add_startup_system(setup_graphics)
        .add_startup_system(setup_planet)
        .add_startup_system(setup_asteroid_spawning)
        .add_startup_system(setup_ships)
        .add_system(spawn_asteroids)
        .add_system(move_asteroids)
        .add_system(setup_ships_target_lock)
        .add_system(move_ships)
        .add_system(despawn_asteroids_on_planet_collision)
        .add_system(bump_asteroids_on_ship_collision_with_bump_power)
        .add_system(destroy_asteroids_on_ship_collision_with_destroy_power)
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
        .insert(Collider::ball(planet_radius))
        .insert(ActiveEvents::COLLISION_EVENTS);
}

/// Configure our asteroid spawning algorithm
fn setup_asteroid_spawning(mut commands: Commands) {
    commands.insert_resource(AsteroidSpawnConfig {
        // create the repeating timer
        timer: Timer::new(Duration::from_secs(ASTEROID_SPAWN_TIME), true),
    })
}

/// Spawn one simple ship
fn setup_ships(mut commands: Commands) {
    let x = 100.0;
    let y = 100.0;

    let a = Vec2::new(0.0, 10.0);
    let b = Vec2::new(-5.0, 0.0);
    let c = Vec2::new(5.0, 0.0);

    commands
        .spawn_bundle(TransformBundle::from(Transform::from_xyz(x, y, 0.0)))
        .insert(Ship)
        .insert(ContactBumpPower)
        .insert(ShipTarget(None))
        .insert(RigidBody::Dynamic)
        .insert(Collider::triangle(a, b, c))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::default())
        .insert(Sleeping::disabled());

    let x = 100.0;
    let y = -100.0;

    let a = Vec2::new(0.0, 10.0);
    let b = Vec2::new(-5.0, 0.0);
    let c = Vec2::new(5.0, 0.0);

    commands
        .spawn_bundle(TransformBundle::from(Transform::from_xyz(x, y, 0.0)))
        .insert(Ship)
        .insert(ContactDestroyPower)
        .insert(ShipTarget(None))
        .insert(RigidBody::Dynamic)
        .insert(Collider::triangle(a, b, c))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::default())
        .insert(Sleeping::disabled());
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
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(Velocity::default())
            .insert(ExternalForce::default())
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

fn despawn_asteroids_on_planet_collision(
    mut commands: Commands,
    planet: Query<(), With<Planet>>,
    asteroids: Query<Entity, With<Asteroid>>,
    mut collision_events: EventReader<CollisionEvent>,
) {
    for event in collision_events.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            if let (Ok(_), Ok(entity)) = (planet.get(*e1), asteroids.get(*e2)) {
                commands.entity(entity).despawn();
            } else if let (Ok(_), Ok(entity)) = (planet.get(*e2), asteroids.get(*e1)) {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn bump_asteroids_on_ship_collision_with_bump_power(
    mut ships: Query<&Transform, (With<Ship>, With<ContactBumpPower>)>,
    mut asteroids: Query<(&Transform, &mut ExternalForce), With<Asteroid>>,
    mut collision_events: EventReader<CollisionEvent>,
) {
    for event in collision_events.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            let components = if let (Ok(ship_transform), Ok(comps)) =
                (ships.get_mut(*e1), asteroids.get_mut(*e2))
            {
                Some((ship_transform, comps))
            } else if let (Ok(ship_transform), Ok(comps)) =
                (ships.get_mut(*e2), asteroids.get_mut(*e1))
            {
                Some((ship_transform, comps))
            } else {
                None
            };

            if let Some((ship_transform, (transform, mut ext_force))) = components {
                let diff = transform.translation - ship_transform.translation;
                let direction = diff.normalize_or_zero();
                ext_force.force = direction.xy() * SHIP_BUMP_FORCE;
                ext_force.torque = 0.01;
            }
        }
    }
}

fn destroy_asteroids_on_ship_collision_with_destroy_power(
    mut commands: Commands,
    mut ships: Query<(), (With<Ship>, With<ContactDestroyPower>)>,
    mut asteroids: Query<(Entity, &Transform), With<Asteroid>>,
    mut collision_events: EventReader<CollisionEvent>,
    image_assets: Res<ImageAssets>,
) {
    for event in collision_events.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            let comps = if let (Ok(()), Ok(comps)) = (ships.get_mut(*e1), asteroids.get_mut(*e2)) {
                Some(comps)
            } else if let (Ok(_), Ok(comps)) = (ships.get_mut(*e2), asteroids.get_mut(*e1)) {
                Some(comps)
            } else {
                None
            };

            if let Some((entity, transform)) = comps {
                let mut rng = thread_rng();
                let dice_number = DiceNumber::from_rng(&mut rng);
                let translation = transform.translation;
                commands.entity(entity).despawn();
                commands
                    .spawn_bundle(SpriteBundle {
                        sprite: Sprite { custom_size: Some(Vec2::splat(25.0)), ..default() },
                        transform: Transform::from_translation(translation),
                        texture: image_assets.handle_for_dice_number(dice_number).clone(),
                        ..default()
                    })
                    .insert(DiceLoot { number: dice_number })
                    .insert(Animator::new(Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::PingPong,
                        Duration::from_millis(150),
                        TransformRotateZLens { start: 0.0, end: PI / 6.0 },
                    )));
            }
        }
    }
}

fn setup_ships_target_lock(
    asteroids: Query<(Entity, &Transform), With<Asteroid>>,
    mut ships: Query<(&Transform, &mut ShipTarget), With<Ship>>,
) {
    if !asteroids.is_empty() {
        for (ship_transform, mut ship_target) in &mut ships {
            if ship_target.0.map_or(true, |e| asteroids.get(e).is_err()) {
                let nearest = asteroids.iter().min_by_key(|(_, transform)| {
                    OrderedFloat(transform.translation.distance_squared(ship_transform.translation))
                });

                if let Some((entity, transform)) = nearest {
                    let distance = transform.translation.distance(ship_transform.translation);
                    if distance <= SHIP_TRIGGER_MAX_DISTANCE {
                        ship_target.0 = Some(entity);
                    }
                }
            }
        }
    }
}

fn move_ships(
    time: Res<Time>,
    asteroids: Query<&Transform, With<Asteroid>>,
    mut ships: Query<(&Transform, &mut Velocity, &ShipTarget), With<Ship>>,
) {
    for (ship_transform, mut ship_velocity, ship_target) in &mut ships {
        if let Some(Ok(transform)) = ship_target.0.map(|e| asteroids.get(e)) {
            let diff = transform.translation - ship_transform.translation;
            let direction = diff.normalize_or_zero();
            ship_velocity.linvel = direction.xy() * SHIP_SPEED * time.delta_seconds();
        }
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

#[derive(Component, Debug)]
struct Ship;

#[derive(Component, Debug)]
struct ContactBumpPower;

#[derive(Component, Debug)]
struct ContactDestroyPower;

#[derive(Component, Debug)]
struct ShipTarget(Option<Entity>);

#[derive(Component, Debug)]
struct DiceLoot {
    number: DiceNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum DiceNumber {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}

impl DiceNumber {
    fn from_rng<R: Rng>(rng: &mut R) -> DiceNumber {
        match rng.gen_range(0..6) {
            1 => DiceNumber::One,
            2 => DiceNumber::Two,
            3 => DiceNumber::Three,
            4 => DiceNumber::Four,
            5 => DiceNumber::Five,
            _ => DiceNumber::Six,
        }
    }
}

#[derive(AssetCollection)]
struct ImageAssets {
    #[asset(path = "images/dice_1.png")]
    pub dice_1: Handle<Image>,
    #[asset(path = "images/dice_2.png")]
    pub dice_2: Handle<Image>,
    #[asset(path = "images/dice_3.png")]
    pub dice_3: Handle<Image>,
    #[asset(path = "images/dice_4.png")]
    pub dice_4: Handle<Image>,
    #[asset(path = "images/dice_5.png")]
    pub dice_5: Handle<Image>,
    #[asset(path = "images/dice_6.png")]
    pub dice_6: Handle<Image>,
}

impl ImageAssets {
    fn handle_for_dice_number(&self, dice: DiceNumber) -> &Handle<Image> {
        match dice {
            DiceNumber::One => &self.dice_1,
            DiceNumber::Two => &self.dice_2,
            DiceNumber::Three => &self.dice_3,
            DiceNumber::Four => &self.dice_4,
            DiceNumber::Five => &self.dice_5,
            DiceNumber::Six => &self.dice_6,
        }
    }
}
