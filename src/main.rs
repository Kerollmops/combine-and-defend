use std::array;
use std::collections::vec_deque::{self, VecDeque};
use std::f32::consts::PI;
use std::time::Duration;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::MaterialMesh2dBundle;
use bevy_asset_loader::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_tweening::lens::TransformRotateZLens;
use bevy_tweening::*;
use ordered_float::OrderedFloat;
use rand::prelude::*;

const ASTEROID_SPAWN_RADIUS_DISTANCE: f32 = 800.0;
const ASTEROID_RADIUS: f32 = 10.0;
const ASTEROID_SPEED: f32 = 1.0; // by second
const ASTEROID_SPAWN_TIME: u64 = 1; // in second
const ASTERIOD_COLORS: [Color; 5] = [
    Color::rgb(0.663, 0.663, 0.663),
    Color::rgb(0.502, 0.502, 0.502),
    Color::rgb(0.424, 0.275, 0.0),
    Color::rgb(0.325, 0.208, 0.0),
    Color::rgb(0.231, 0.318, 0.369),
];

const SHIP_SPEED: f32 = 2400.0; // by second
const SHIP_TRIGGER_MAX_DISTANCE: f32 = 400.0;
const SHIP_BUMP_FORCE: f32 = 4.0;
const SHIP_MAX_DISTANCE_FROM_PLANET_INTEREST: f32 = 500.0;
const SHIP_PLANET_SIGHT: f32 = 100.0;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugin(TweeningPlugin)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Msaa::default())
        .insert_resource(DiceBag::default())
        .add_event::<DiceOwnedEvent>()
        .add_event::<DiceLostEvent>()
        .init_collection::<ImageAssets>()
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .insert_resource(RapierConfiguration { gravity: Vec2::ZERO, ..default() });

    #[cfg(feature = "debug-render")]
    app.add_plugin(RapierDebugRenderPlugin::default());

    app.add_startup_system(setup_graphics)
        .add_startup_system(setup_planet)
        // .add_startup_system(setup_debug)
        .add_startup_system(setup_asteroid_spawning)
        .add_startup_system(setup_ships)
        .add_system(spawn_asteroids)
        .add_system(setup_ships_target_lock)
        .add_system(move_ships)
        .add_system(despawn_asteroids_on_planet_collision)
        .add_system(remove_dice_from_bag_on_planet_collision)
        .add_system(bump_asteroids_on_ship_collision_with_bump_power)
        .add_system(destroy_asteroids_on_ship_collision_with_destroy_power)
        .add_system(collect_dices_by_mouse_clicking)
        .add_system(manage_dice_events)
        .add_system(draw_dice_bag)
        .run();
}

fn setup_graphics(mut commands: Commands) {
    commands.spawn_bundle(Camera2dBundle::default()).insert(SpaceCamera);
}

/// Configure the main planet to defend
fn setup_planet(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Planet Earth
    let planet_radius = 50.0;

    commands
        .spawn_bundle(MaterialMesh2dBundle {
            mesh: meshes
                .add(Mesh::from(shape::Icosphere { radius: planet_radius, subdivisions: 30 }))
                .into(),
            material: materials.add(ColorMaterial::from(Color::rgb(0.302, 0.302, 1.0))),
            ..default()
        })
        .insert(Planet)
        .insert(Collider::ball(planet_radius))
        .insert(ActiveEvents::COLLISION_EVENTS);
}

#[allow(unused)]
fn setup_debug(mut dice_writer: EventWriter<DiceOwnedEvent>) {
    let mut rng = thread_rng();
    for _ in 0..rng.gen_range(2..5) {
        dice_writer.send(DiceOwnedEvent(DiceNumber::from_rng(&mut rng)));
    }
}

/// Configure our asteroid spawning algorithm
fn setup_asteroid_spawning(mut commands: Commands) {
    commands.insert_resource(AsteroidSpawnConfig {
        // create the repeating timer
        timer: Timer::new(Duration::from_secs(ASTEROID_SPAWN_TIME), true),
    })
}

fn create_triangle(a: Vec2, b: Vec2, c: Vec2) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![a.extend(0.0).to_array(), b.extend(0.0).to_array(), c.extend(0.0).to_array()],
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[1.0, 1.0], [1.0, 1.0], [1.0, 1.0]]);
    mesh.set_indices(Some(Indices::U32(vec![0, 2, 1, 0, 3, 2])));
    mesh
}

/// Spawn two basic ships
fn setup_ships(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let x = 100.0;
    let y = 100.0;

    let a = Vec2::new(-0.5, 0.0);
    let b = Vec2::new(0.0, 1.0);
    let c = Vec2::new(0.5, 0.0);

    commands
        .spawn_bundle(MaterialMesh2dBundle {
            mesh: meshes.add(create_triangle(a, b, c)).into(),
            transform: Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(10.)),
            material: materials.add(ColorMaterial::from(Color::PURPLE)),
            ..default()
        })
        .insert(Ship)
        .insert(ContactBumpPower)
        .insert(ShipTarget(None))
        .insert(RigidBody::Dynamic)
        .insert(Collider::triangle(a, b, c))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::default());

    let x = 100.0;
    let y = -100.0;

    commands
        .spawn_bundle(MaterialMesh2dBundle {
            mesh: meshes.add(create_triangle(a, b, c)).into(),
            transform: Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(10.)),
            material: materials.add(ColorMaterial::from(Color::PURPLE)),
            ..default()
        })
        .insert(Ship)
        .insert(ContactDestroyPower)
        .insert(ShipTarget(None))
        .insert(RigidBody::Dynamic)
        .insert(Collider::triangle(a, b, c))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::default());
}

fn spawn_asteroids(
    mut commands: Commands,
    time: Res<Time>,
    planet: Query<&Transform, With<Planet>>,
    mut config: ResMut<AsteroidSpawnConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    config.timer.tick(time.delta());

    if config.timer.finished() {
        let planet_transform = planet.single();
        let planet_translation = planet_transform.translation;

        let mut rng = thread_rng();
        let angle = rng.gen::<f32>() * PI * 2.0;
        let x = angle.cos() * ASTEROID_SPAWN_RADIUS_DISTANCE + planet_translation.x;
        let y = angle.sin() * ASTEROID_SPAWN_RADIUS_DISTANCE + planet_translation.y;
        let translation = Vec3::new(x, y, 0.0);
        let color = ASTERIOD_COLORS.choose(&mut rng).unwrap().clone();

        let diff = planet_translation - translation;
        let direction = diff.normalize_or_zero().xy();

        commands
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: meshes
                    .add(Mesh::from(shape::Icosphere { radius: ASTEROID_RADIUS, subdivisions: 30 }))
                    .into(),
                material: materials.add(ColorMaterial::from(color)),
                transform: Transform::from_translation(translation),
                ..default()
            })
            .insert(Asteroid)
            .insert(RigidBody::Dynamic)
            .insert(ExternalImpulse { impulse: direction * ASTEROID_SPEED, torque_impulse: 0.0 })
            .insert(Collider::ball(ASTEROID_RADIUS))
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(Sleeping::disabled());
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

fn remove_dice_from_bag_on_planet_collision(
    planet: Query<(), With<Planet>>,
    asteroids: Query<(), With<Asteroid>>,
    mut collision_events: EventReader<CollisionEvent>,
    mut dice_lost: EventWriter<DiceLostEvent>,
) {
    for event in collision_events.iter() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            if let (Ok(_), Ok(_)) = (planet.get(*e1), asteroids.get(*e2)) {
                dice_lost.send(DiceLostEvent);
            } else if let (Ok(_), Ok(_)) = (planet.get(*e2), asteroids.get(*e1)) {
                dice_lost.send(DiceLostEvent);
            }
        }
    }
}

fn bump_asteroids_on_ship_collision_with_bump_power(
    mut ships: Query<&Transform, (With<Ship>, With<ContactBumpPower>)>,
    mut asteroids: Query<(&Transform, &mut ExternalImpulse), With<Asteroid>>,
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

            if let Some((ship_transform, (transform, mut ext_impl))) = components {
                let diff = transform.translation - ship_transform.translation;
                let direction = diff.normalize_or_zero();
                ext_impl.impulse = direction.xy() * SHIP_BUMP_FORCE;
                ext_impl.torque_impulse = 0.001;
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
    planet: Query<&Transform, With<Planet>>,
    asteroids: Query<(Entity, &Transform), With<Asteroid>>,
    mut ships: Query<(&Transform, &mut ShipTarget), With<Ship>>,
) {
    if !asteroids.is_empty() {
        let planet_transform = planet.single();

        for (ship_transform, mut ship_target) in &mut ships {
            match ship_target.0.map(|e| asteroids.get(e)) {
                Some(Ok((_entity, transform))) => {
                    let planet_distance =
                        planet_transform.translation.distance(transform.translation);
                    if planet_distance > SHIP_MAX_DISTANCE_FROM_PLANET_INTEREST {
                        ship_target.0 = None;
                    }
                }
                _otherwise => {
                    let nearest = asteroids.iter().min_by_key(|(_, transform)| {
                        OrderedFloat(
                            transform.translation.distance_squared(ship_transform.translation),
                        )
                    });

                    if let Some((entity, transform)) = nearest {
                        let distance = transform.translation.distance(ship_transform.translation);
                        let planet_distance =
                            planet_transform.translation.distance(transform.translation);
                        if distance <= SHIP_TRIGGER_MAX_DISTANCE
                            && planet_distance <= SHIP_MAX_DISTANCE_FROM_PLANET_INTEREST
                        {
                            ship_target.0 = Some(entity);
                        }
                    }
                }
            }
        }
    }
}

/// Move the ships to collide with the targeted asteroids and
/// toward the planet when there is no target.
fn move_ships(
    time: Res<Time>,
    planet: Query<&Transform, With<Planet>>,
    asteroids: Query<&Transform, With<Asteroid>>,
    mut ships: Query<(&Transform, &mut Velocity, &ShipTarget), With<Ship>>,
) {
    for (ship_transform, mut ship_velocity, ship_target) in &mut ships {
        match ship_target.0.map(|e| asteroids.get(e)) {
            Some(Ok(transform)) => {
                let diff = transform.translation - ship_transform.translation;
                let direction = diff.normalize_or_zero();
                ship_velocity.linvel = direction.xy() * SHIP_SPEED * time.delta_seconds();
            }
            _otherwise => {
                let planet_transform = planet.single();
                let distance = planet_transform.translation.distance(ship_transform.translation);
                if distance >= SHIP_PLANET_SIGHT {
                    let diff = planet_transform.translation - ship_transform.translation;
                    let direction = diff.normalize_or_zero();
                    ship_velocity.linvel = direction.xy() * SHIP_SPEED * time.delta_seconds();
                } else {
                    ship_velocity.linvel = Vec2::ZERO;
                }
            }
        }
    }
}

fn collect_dices_by_mouse_clicking(
    mut commands: Commands,
    mut dice_owned: EventWriter<DiceOwnedEvent>,
    wnds: Res<Windows>,
    camera: Query<(&Camera, &GlobalTransform), With<SpaceCamera>>,
    dices: Query<(Entity, &Sprite, &GlobalTransform, &DiceLoot), With<DiceLoot>>,
    buttons: Res<Input<MouseButton>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        let (camera, camera_transform) = camera.single();
        let wnd = if let RenderTarget::Window(id) = camera.target {
            wnds.get(id).unwrap()
        } else {
            wnds.get_primary().unwrap()
        };

        // check if the cursor is inside the window and get its position
        if let Some(screen_pos) = wnd.cursor_position() {
            // get the size of the window
            let window_size = Vec2::new(wnd.width() as f32, wnd.height() as f32);
            // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
            let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
            // matrix for undoing the projection and camera transform
            let ndc_to_world =
                camera_transform.compute_matrix() * camera.projection_matrix().inverse();
            // use it to convert ndc to world-space coordinates
            let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));
            // reduce it to a 2D value
            let world_pos: Vec2 = world_pos.truncate();

            for (entity, sprite, transform, dice_loot) in &dices {
                if let Some(size) = sprite.custom_size {
                    let translation = transform.translation().xy();
                    let p = world_pos;

                    let b_left = translation.x - size.x;
                    let b_right = translation.x + size.x;
                    let b_top = translation.y - size.y;
                    let b_bottom = translation.y + size.y;

                    if (p.x >= b_left && p.x <= b_right) && (p.y >= b_top && p.y <= b_bottom) {
                        dice_owned.send(DiceOwnedEvent(dice_loot.number));
                        commands.entity(entity).despawn();
                    }
                }
            }
        }
    }
}

fn manage_dice_events(
    mut dice_lost: EventReader<DiceLostEvent>,
    mut dice_owned: EventReader<DiceOwnedEvent>,
    mut dice_bag: ResMut<DiceBag>,
) {
    for DiceLostEvent in dice_lost.iter() {
        dice_bag.try_consume::<1>();
    }

    for DiceOwnedEvent(number) in dice_owned.iter() {
        dice_bag.push(*number);
    }
}

// We need to rewrite this part and not clear and recreate the UI from scratch,
// it makes it impossible to animate stuff and things...
fn draw_dice_bag(
    mut commands: Commands,
    dice_bag: Res<DiceBag>,
    mut dice_bag_numbers: Query<Entity, With<DiceBagNumbers>>,
    image_assets: Res<ImageAssets>,
) {
    // We clear the screen of the bag dice numbers list.
    dice_bag_numbers.for_each_mut(|entity| commands.entity(entity).despawn_recursive());

    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            color: Color::NONE.into(),
            ..default()
        })
        .insert(DiceBagNumbers)
        .with_children(|parent| {
            for (i, dice_number) in dice_bag.iter().enumerate() {
                parent
                    .spawn_bundle(NodeBundle {
                        style: Style {
                            size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                            position_type: PositionType::Absolute,
                            position: UiRect {
                                left: Val::Px(20.0),
                                bottom: Val::Px(30.0 * i as f32 + 20.0),
                                ..default()
                            },
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::FlexStart,
                            ..default()
                        },
                        color: Color::NONE.into(),
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_bundle(ImageBundle {
                            style: Style { size: Size::new(Val::Px(25.0), Val::Auto), ..default() },
                            image: image_assets.handle_for_dice_number(*dice_number).clone().into(),
                            ..default()
                        });
                    });
            }
        });
}

#[derive(Debug, Default)]
struct DiceBag {
    bag: VecDeque<DiceNumber>,
}

impl DiceBag {
    fn push(&mut self, dice: DiceNumber) {
        self.bag.push_back(dice);
    }

    fn try_consume<const N: usize>(&mut self) -> Option<[DiceNumber; N]> {
        if self.bag.len() >= N {
            Some(array::from_fn(|_| self.bag.pop_front().unwrap()))
        } else {
            None
        }
    }

    fn iter(&self) -> vec_deque::Iter<DiceNumber> {
        self.bag.iter()
    }
}

#[derive(Component, Debug)]
struct SpaceCamera;

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

/// The list of dice numbers displayed on the left of the screen.
#[derive(Component, Debug)]
struct DiceBagNumbers;

struct DiceOwnedEvent(DiceNumber);

struct DiceLostEvent;

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
