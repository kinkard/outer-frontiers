use bevy::{
    ecs::system::Command,
    prelude::*,
    render::{
        mesh::VertexAttributeValues, renderer::RenderDevice, texture::CompressedImageFormats,
    },
    utils::HashMap,
};
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::GameStates;

pub(crate) struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(GameStates::AssetLoading)
                .continue_to_state(GameStates::Next)
                .load_collection::<Models>()
                .load_collection::<Environment>(),
        )
        .add_systems(OnEnter(GameStates::AssetLoading), check_supported_formats)
        .add_systems(OnExit(GameStates::AssetLoading), extract_model_colliders)
        .init_resource::<ModelColliders>()
        // From bevy 0.12 scene_spawner runs between Update and PostUpdate so we can set colliders
        // in the same frame scene was spawned
        .add_systems(PostUpdate, set_model_collider);
    }
}

#[derive(AssetCollection, Resource)]
pub(crate) struct Environment {
    // Cubemap is generated by https://github.com/petrocket/spacescape, http://alexcpeterson.com/spacescape/
    // And encoded to ktx2 with ASTC encoding and zstd compression using https://github.com/KhronosGroup/KTX-Software:
    // `toktx --encode astc --astc_blk_d 4x4 --zcmp 19 --cubemap background posx.png negx.png posy.png negy.png posz.png negz.png`
    // This compression saves 50Mb of RAM usage during runtime comparing to the simple PNG.
    #[asset(path = "textures/space_cubemap_astc.ktx2")]
    pub(crate) skybox_image: Handle<Image>,
}

fn check_supported_formats(render_device: Res<RenderDevice>) {
    assert!(
        CompressedImageFormats::from_features(render_device.features())
            .contains(CompressedImageFormats::ASTC_LDR),
        "ASTC_LDR compression format is not supported, skybox image cannot be loaded"
    );
}

#[derive(AssetCollection, Resource)]
pub(crate) struct Models {
    #[asset(path = "models/zenith_station.glb#Scene0")]
    pub(crate) zenith_station: Handle<Scene>,
    #[asset(path = "models/praetor.glb#Scene0")]
    pub(crate) praetor: Handle<Scene>,
    #[asset(path = "models/infiltrator.glb#Scene0")]
    pub(crate) infiltrator: Handle<Scene>,
    #[asset(path = "models/dragoon.glb#Scene0")]
    pub(crate) dragoon: Handle<Scene>,
}

fn extract_mesh_vertices(mesh: &Mesh) -> Option<Vec<Vec3>> {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32(vtx) => {
            Some(vtx.chunks(3).map(|v| Vec3::new(v[0], v[1], v[2])).collect())
        }
        VertexAttributeValues::Float32x3(vtx) => {
            Some(vtx.iter().map(|v| Vec3::new(v[0], v[1], v[2])).collect())
        }
        _ => None,
    }
}

// fn extract_mesh_indices(mesh: &Mesh) -> Option<Vec<[u32; 3]>> {
//     match mesh.indices() {
//         Some(Indices::U16(idx)) => Some(
//             idx.chunks_exact(3)
//                 .map(|i| [i[0] as u32, i[1] as u32, i[2] as u32])
//                 .collect(),
//         ),
//         Some(Indices::U32(idx)) => Some(idx.chunks_exact(3).map(|i| [i[0], i[1], i[2]]).collect()),
//         None => None,
//     }
// }

/// A workaround for rapier Colliders that are built on the game startup.
/// This collection is filled right after all scenes are loaded and then used
/// every time corresponding scene is spawned.
#[derive(Default, Resource)]
struct ModelColliders(HashMap<AssetId<Scene>, Collider>);

/// Extracts hulls (meshed with `_hull` or `_hull_<some number>` suffix),
/// builds rapier Collider from them and stores in the `ModelColliders`
fn extract_model_colliders(
    mut scenes: ResMut<Assets<Scene>>,
    meshes: Res<Assets<Mesh>>,
    mut model_colliders: ResMut<ModelColliders>,
) {
    for (scene_id, scene) in scenes.iter_mut() {
        // Find all hulls in the scene
        let hulls = scene
            .world
            // There are two entities in the scene for each hull - mesh itself and parent Node.
            // Transforms are stored inside Node (which is parent to the Mesh)
            .query::<(Entity, &Name, Without<Handle<Mesh>>)>()
            .iter(&scene.world)
            .filter_map(|(entity, name, _)| {
                if name.ends_with("_hull") || name.contains("_hull_") {
                    Some(entity)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let colliders = hulls
            .iter()
            .filter_map(|hull| {
                // todo: transforms should be combined from the root to handle nested hulls
                let transform = scene.world.get::<Transform>(*hull)?;
                let children = scene.world.get::<Children>(*hull)?;
                Some((transform.compute_affine(), children))
            })
            .flat_map(|(affine, children)| {
                children
                    .iter()
                    .filter_map(|entity| scene.world.get::<Handle<Mesh>>(*entity))
                    .map(|handle| meshes.get(handle).expect("broken mesh handle"))
                    .filter_map(extract_mesh_vertices)
                    // Transform Mesh points into world coordinates
                    .map(move |mut vertices| {
                        vertices
                            .iter_mut()
                            .for_each(|v| *v = affine.transform_point3(*v));
                        vertices
                    })
            })
            .map(|points| Collider::convex_hull(&points).unwrap())
            .map(|collider| (Vec3::ZERO, Quat::IDENTITY, collider))
            .collect::<Vec<_>>();

        if !colliders.is_empty() {
            model_colliders
                .0
                .insert(scene_id, Collider::compound(colliders));
        }

        // todo: we also want to clean up other resources as well, like Meshes
        for entity in hulls {
            // Don't forget to clean parent-child relations
            RemoveParent { child: entity }.apply(&mut scene.world);
            DespawnRecursive { entity }.apply(&mut scene.world);
        }
    }
}

/// Attaches rapier Collider to the scene entity once it is spawned
fn set_model_collider(
    mut commands: Commands,
    colliders: Res<ModelColliders>,
    spawned_scenes: Query<(Entity, &Handle<Scene>), Changed<Handle<Scene>>>,
) {
    for (entity, scene) in spawned_scenes.iter() {
        if let Some(collider) = colliders.0.get(&scene.id()) {
            commands.entity(entity).insert(collider.clone());
        }
    }
}