use glam::{IVec3, Mat4, Vec3, Vec4};
use itertools::Itertools;

use crate::{
    terrain::{
        area::Area,
        chunk::{Chunk, CHUNK_SIZE},
        position_types::ChunkPos,
        Terrain,
    },
    util::face_index::{FaceIndex, FACE_NORMALS},
};

/// Cave-culling search from https://tomcc.github.io/2014/08/31/visibility-2.html.
///
/// Starting at the chunk containing the camera, visit each neighbouring chunk in a forwards
/// direction that is visible through that chunk, using the "visibility graphs" computed when
/// the chunks are loaded.
///
/// When a chunk is visited, `draw_fn` is called given the chunk as a parameter
///
/// This has the following effects:
/// * Obscured areas like caves are often skipped and do not even need to be meshed
/// * Chunks are naturally rendered front-to-back
/// * Chunk meshes are only updated when the chunk is visible
///
/// Returns the Vec of chunks to be drawn in order
pub fn visibility_search<'terrain>(
    terrain: &'terrain Terrain,
    loaded_area: &Area,
    view_projection_matrix: Mat4,
    camera_pos: Vec3,
) -> Vec<&'terrain Chunk> {
    // queue of chunks to render in order, along with the face they were visited from
    let mut step_queue = Vec::new();
    // index of the entry in `step_queue` that is currently being visited
    let mut step_index = 0;
    // whether each chunk in the loaded area has already been explored
    let mut explored = vec![false; loaded_area.size().product()];

    // start at the camera position
    let camera_chunk_pos = ChunkPos::from(
        (camera_pos / (CHUNK_SIZE as f32))
            .floor()
            .as_ivec3(),
    );
    let Some(camera_chunk) = loaded_area.get_chunk(terrain, &camera_chunk_pos) else {
        return Vec::new();
    };

    step_queue.push(SearchStep {
        chunk: camera_chunk,
        last_dir: None,
    });
    log::info!("{:?}", camera_chunk_pos - loaded_area.pos());
    explored[loaded_area.size().flatten(
        (camera_chunk_pos - loaded_area.pos())
            .as_ivec3()
            .as_uvec3(),
    )] = true;

    while let Some(&step) = step_queue.get(step_index) {
        // vector pointing towards the camera
        let to_camera = (camera_chunk_pos - step.chunk.pos()).as_ivec3();

        // consider whether to explore the 6 neighbouring chunks
        step_queue.extend(
            (0..6)
                // don't travel back towards the camera
                .filter(|&dir| IVec3::dot(FACE_NORMALS[dir], to_camera) <= 0)
                // only visit chunks which are connected via the current chunk from the previous
                // face to the current face
                .filter(|&dir| {
                    if let Some(last_dir) = step.last_dir {
                        step.chunk
                            .visibility_graph()
                            .connected(FaceIndex(dir), FaceIndex(last_dir).opposite())
                    } else {
                        true
                    }
                })
                // dir -> (dir, chunk_pos)
                .map(|dir| (dir, step.chunk.pos() + ChunkPos::from(FACE_NORMALS[dir])))
                .filter(|(_, chunk_pos)| loaded_area.contains_pos(chunk_pos))
                // don't visit the same chunk twice
                .filter(|(_, chunk_pos)| {
                    let index = loaded_area.size().flatten(
                        (*chunk_pos - loaded_area.pos())
                            .as_ivec3()
                            .as_uvec3(),
                    );

                    if index < explored.len() {
                        let is_explored = explored[index];
                        explored[index] = true;
                        return !is_explored;
                    } else {
                        return false;
                    }
                })
                // frustum culling
                .filter(|(_, chunk_pos)| {
                    let aabb_min = chunk_pos.as_vec3() * (CHUNK_SIZE as f32);
                    let aabb_size = Vec3::splat(CHUNK_SIZE as f32);
                    aabb_frustum_test(view_projection_matrix, aabb_min, aabb_size)
                })
                // (dir, chunk_pos) -> SearchStep
                .filter_map(|(dir, chunk_pos)| {
                    loaded_area
                        .get_chunk(terrain, &chunk_pos)
                        .map(|chunk| SearchStep {
                            chunk,
                            last_dir: Some(dir),
                        })
                }),
        );

        step_index += 1;
    }

    log::info!(
        "rendering {} out of {} chunks",
        step_queue.len(),
        terrain.chunks().len()
    );

    return step_queue
        .iter()
        .map(|&SearchStep { chunk, .. }| chunk)
        .collect_vec();
}

#[derive(Clone, Copy)]
struct SearchStep<'a> {
    chunk: &'a Chunk,
    last_dir: Option<usize>,
}

// bad frustum culling
pub fn aabb_frustum_test(view_projection_matrix: Mat4, aabb_min: Vec3, aabb_size: Vec3) -> bool {
    for i in 0..8 {
        let x = (i & 1) >> 0;
        let y = (i & 2) >> 1;
        let z = (i & 4) >> 2;

        let world_pos = aabb_min + aabb_size * Vec3::new(x as f32, y as f32, z as f32);
        let clip_pos =
            view_projection_matrix * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);

        if (-clip_pos.w..=clip_pos.w).contains(&clip_pos.x)
            && (-clip_pos.w..=clip_pos.w).contains(&clip_pos.y)
            && (0.0..=clip_pos.w).contains(&clip_pos.z)
        {
            return true;
        }
    }

    false
}
