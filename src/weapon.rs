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
            self.lifetime.clone(),
            // Change to RigidBody::Dynamic if projectile should be affected by gravity or other forces
            RigidBody::KinematicVelocityBased,
            Velocity {
                linvel: velocity,
                ..default()
            },
            self.collider.clone(),
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
    /// Interval between shots in seconds
    shot_interval: f32,
    /// Weapon cooldown timer in seconds. Cannot be negative outside of [`weapon_fire`] system.
    cooldown: f32,
}

impl Weapon {
    pub(crate) fn new(rate_of_fire: f32) -> Self {
        Self {
            is_firing: false,
            shot_interval: 1.0 / rate_of_fire,
            cooldown: 0.0,
        }
    }

    pub(crate) fn fire(&mut self) {
        self.is_firing = true;
    }
}

fn weapon_fire(
    mut commands: Commands,
    projectile: Res<Projectile>,
    mut query: Query<(Entity, &mut Weapon, &GlobalTransform)>,
    time: Res<Time>,
    velocity_query: Query<&Velocity>,
    parent_query: Query<&Parent>,
) {
    for (entity, mut weapon, transform) in query.iter_mut() {
        if weapon.cooldown > 0.0 {
            // Tick cooldown only if greater than zero to avoid negative value on first frame of firing.
            // Negative values than are used to calculate offset time for projectile spawn to keep constant fire rate.
            weapon.cooldown -= time.delta_seconds();
        }
        if !weapon.is_firing {
            weapon.cooldown = weapon.cooldown.max(0.0);
            continue;
        }
        // `weapon.is_firing` should be set each frame by input system
        weapon.is_firing = false;

        // resolve own velocity from parent if any
        let gun_velocity = parent_query
            .iter_ancestors(entity)
            .filter_map(|parent| velocity_query.get(parent).ok())
            .map(|velocity| velocity.linvel)
            .next()
            .unwrap_or(Vec3::ZERO);

        while weapon.cooldown <= 0.0 {
            // time in past from the current frame when projectile should be spawned
            let offset_time = -weapon.cooldown;
            weapon.cooldown += weapon.shot_interval;

            let direction = transform.forward().as_vec3();
            let velocity = direction * projectile.speed + gun_velocity;
            // move projectile spawn point forward to handle case when multiple projectiles are spawned
            let position = transform.translation() + velocity * offset_time;

            projectile.spawn(&mut commands, position, direction, velocity);
        }
    }
}
