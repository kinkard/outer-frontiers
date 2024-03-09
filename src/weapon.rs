use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

use bevy_rapier3d::prelude::*;

use crate::GameStates;

pub(crate) struct WeaponPlugin;
impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameStates::Next), setup_projectile)
            .add_systems(Update, weapon_fire.run_if(in_state(GameStates::Next)))
            // Run `lifetime` in PostUpdate so it can despawn entities after all collisions are resolved
            .add_systems(PostUpdate, lifetime);
    }
}

/// Entity lifetime in seconds, after which entity should be destroyed
#[derive(Component, Clone)]
struct Lifetime(f32);

fn lifetime(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut Lifetime)>) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.0 -= time.delta_seconds();
        if lifetime.0 <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

#[derive(Resource)]
struct Projectile {
    collider: Collider,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,

    speed: f32,
    lifetime: Lifetime,
}

impl Projectile {
    fn new(
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> Self {
        let radius = 0.1;
        Self {
            collider: Collider::capsule_y(8.0 * radius, radius),
            mesh: meshes.add(Mesh::from(Capsule3d {
                radius,
                half_length: 8.0 * radius,
            })),
            material: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                // exclude this material from shadows calculations
                unlit: true,
                ..default()
            }),
            lifetime: Lifetime(10.0),

            speed: 100.0,
        }
    }

    fn spawn(&self, commands: &mut Commands, position: Vec3, direction: Vec3, velocity: Vec3) {
        commands.spawn((
            PbrBundle {
                mesh: self.mesh.clone(),
                material: self.material.clone(),
                transform: Transform {
                    translation: position,
                    // `Collider::capsule_y` and `shape::Capsule` are both aligned with Vec3::Y axis
                    rotation: Quat::from_rotation_arc(Vec3::Y, direction),
                    scale: Vec3::ONE,
                },
                ..default()
            },
            self.collider.clone(),
            Velocity {
                linvel: velocity + direction * self.speed,
                ..default()
            },
            self.lifetime.clone(),
            // Change to RigidBody::Dynamic if projectile should be affected by gravity or other forces
            RigidBody::KinematicVelocityBased,
            // Use intersection graph with Sensor for simplicity
            // Remove Sensor if contact graph is needed
            Sensor,
            // Exclude projectile from shadows calculations
            NotShadowCaster,
            NotShadowReceiver,
            Name::new("Projectile"),
        ));
    }
}

fn setup_projectile(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let projectile = Projectile::new(&mut meshes, &mut materials);
    commands.insert_resource(projectile);
}

#[derive(Component)]
pub(crate) struct Weapon {
    is_firing: bool,
    // Delay between shots in seconds
    fire_timeout: f32,
    seconds_since_previous: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        let rate_of_file = 20.0;

        Self {
            is_firing: false,
            fire_timeout: 1.0 / rate_of_file,
            seconds_since_previous: 0.0,
        }
    }
}

impl Weapon {
    pub(crate) fn fire(&mut self) {
        self.is_firing = true;
    }
}

fn weapon_fire(
    mut commands: Commands,
    projectile: Res<Projectile>,
    mut query: Query<(&mut Weapon, &Transform, &Velocity)>,
    time: Res<Time>,
) {
    for (mut weapon, transform, velocity) in query.iter_mut() {
        if weapon.is_firing {
            weapon.seconds_since_previous -= time.delta_seconds();
            if weapon.seconds_since_previous > 0.0 {
                weapon.is_firing = false;
            } else {
                weapon.seconds_since_previous = weapon.fire_timeout;
            }
        }

        if weapon.is_firing {
            projectile.spawn(
                &mut commands,
                transform.translation,
                transform.forward().into(),
                velocity.linvel,
            );
        }
    }
}
