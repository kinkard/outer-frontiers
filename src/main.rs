use std::f32::consts::PI;

use bevy::{core_pipeline::Skybox, prelude::*};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier3d::prelude::*;

mod assets;
mod weapon;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameStates {
    #[default]
    AssetLoading,
    Next,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        .insert_resource(RapierConfiguration {
            gravity: Vec3::ZERO, // disable gravity at all
            ..default()
        })
        .add_plugins(assets::AssetsPlugin)
        .add_plugins(weapon::WeaponPlugin)
        .add_state::<GameStates>()
        .init_resource::<ControlsConfig>()
        .add_systems(OnEnter(GameStates::Next), (setup_light, setup))
        .add_systems(
            Update,
            (player_controller, weapon_fire, animate_light_direction)
                .run_if(in_state(GameStates::Next)),
        )
        .add_systems(Update, bevy::window::close_on_esc)
        .run();
}

// todo: replace by EnvironmentMapLight
fn setup_light(mut commands: Commands) {
    // directional 'sun' light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 32000.0,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 2.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-PI / 4.)),
        ..default()
    });

    // environment map, use an appropriate colour and brightness to match
    commands.insert_resource(AmbientLight {
        color: Color::rgb_u8(210, 220, 240),
        brightness: 0.3,
    });
}

/// Marker component for the player spaceship
#[derive(Component)]
struct Player;

fn setup(
    mut commands: Commands,
    models: Res<assets::Models>,
    environment: Res<assets::Environment>,
) {
    commands
        .spawn(SceneBundle {
            scene: models.zenith_station.clone(),
            ..default()
        })
        .insert(TransformBundle::from(Transform {
            translation: -200.0 * Vec3::Z,
            ..default()
        }))
        .insert(Name::new("Zenith station"));

    commands
        .spawn(SceneBundle {
            scene: models.praetor.clone(),
            ..default()
        })
        .insert(TransformBundle::from(Transform {
            translation: Vec3::new(5.0, 5.0, -20.0),
            ..default()
        }))
        .insert(Player)
        .insert(RigidBody::Dynamic)
        .insert(Restitution::coefficient(0.7))
        .insert(Damping {
            linear_damping: 0.0,
            angular_damping: 1.0,
        })
        .insert(ExternalForce::default())
        .insert(Velocity::default())
        .with_children(|parent| {
            parent.spawn((
                Camera3dBundle::default(),
                Skybox(environment.skybox_image.clone()),
                // todo: specify environment light according to the skybox
                // see the scene_viewer example for more details:
                // EnvironmentMapLight {
                //     diffuse_map: asset_server.load("assets/environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
                //     specular_map: asset_server
                //         .load("assets/environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
                // },
            ));
        })
        .insert(Name::new("Praetor"));

    commands
        .spawn(SceneBundle {
            scene: models.infiltrator.clone(),
            ..default()
        })
        .insert(TransformBundle::from(Transform {
            translation: Vec3::new(-5.0, 5.0, -20.0),
            ..default()
        }))
        .insert(RigidBody::Dynamic)
        .insert(Restitution::coefficient(0.7))
        .insert(Name::new("Infiltrator"));

    commands
        .spawn(SceneBundle {
            scene: models.dragoon.clone(),
            ..default()
        })
        .insert(TransformBundle::from(Transform {
            translation: Vec3::new(0.0, 5.0, 150.0),
            ..default()
        }))
        .insert(weapon::Weapon::default())
        .insert(Name::new("Dragoon"));
}

fn animate_light_direction(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_seconds() * 0.5);
    }
}

#[derive(Resource)]
struct ControlsConfig {
    key_accelerate: KeyCode,
    key_decelerate: KeyCode,
    key_strafe_left: KeyCode,
    key_strafe_right: KeyCode,
    key_strafe_up: KeyCode,
    key_strage_down: KeyCode,
    key_rotate_clockwise: KeyCode,
    key_rotate_counter_clockwise: KeyCode,

    key_primary_weapon_fire: KeyCode,
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            key_accelerate: KeyCode::X,
            key_decelerate: KeyCode::Z,
            key_strafe_left: KeyCode::A,
            key_strafe_right: KeyCode::D,
            key_strafe_up: KeyCode::W,
            key_strage_down: KeyCode::S,
            key_rotate_clockwise: KeyCode::E,
            key_rotate_counter_clockwise: KeyCode::Q,

            key_primary_weapon_fire: KeyCode::Space,
        }
    }
}

fn player_controller(
    config: Res<ControlsConfig>,
    keys: Res<Input<KeyCode>>,
    mouse: Res<Input<MouseButton>>,
    mut mouse_guidance: Local<bool>,
    mut windows: Query<&mut Window>,
    mut egui: bevy_inspector_egui::bevy_egui::EguiContexts,
    mut player: Query<(&Transform, &mut ExternalForce), With<Player>>,
) {
    let (transform, mut force) = player.single_mut();

    force.force = Vec3::ZERO;
    if keys.pressed(config.key_strafe_up) {
        force.force += transform.up() * 100.0;
    }
    if keys.pressed(config.key_strage_down) {
        force.force += transform.down() * 100.0;
    }
    if keys.pressed(config.key_strafe_left) {
        force.force += transform.left() * 100.0;
    }
    if keys.pressed(config.key_strafe_right) {
        force.force += transform.right() * 100.0;
    }
    if keys.pressed(config.key_accelerate) {
        force.force += transform.forward() * 1000.0;
    }
    if keys.pressed(config.key_decelerate) {
        force.force += transform.back() * 1000.0;
    }

    force.torque = Vec3::ZERO;
    if keys.pressed(config.key_rotate_counter_clockwise) {
        force.torque += transform.back() * 300.0;
    }
    if keys.pressed(config.key_rotate_clockwise) {
        force.torque += transform.forward() * 300.0;
    }

    // Enable mouse guidance if Space is pressed
    if keys.just_released(KeyCode::Space) {
        *mouse_guidance = !*mouse_guidance;
    }

    let click_guidance = !egui.ctx_mut().is_pointer_over_area()
        && !egui.ctx_mut().is_using_pointer()
        && mouse.pressed(MouseButton::Left);
    if *mouse_guidance || click_guidance {
        let window = windows.single_mut();

        if let Some(pos) = window.cursor_position() {
            let center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
            let offset = center - pos;

            // Safe zone around screen center for mouse_guidance mode
            if click_guidance || offset.length_squared() > 400.0 {
                force.torque += transform.up() * offset.x;
                force.torque += transform.right() * offset.y;
            }
        }
    }
}

fn weapon_fire(
    config: Res<ControlsConfig>,
    keys: Res<Input<KeyCode>>,
    mut weapon: Query<&mut weapon::Weapon, With<Player>>,
) {
    if keys.pressed(config.key_primary_weapon_fire) {
        for mut weapon in &mut weapon {
            weapon.fire();
        }
    }
}
