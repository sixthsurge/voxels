use glam::{IVec3, Vec3};
use itertools::Itertools;

use crate::{
    render::frustum_culling::FrustumCullingRegions,
    terrain::{
        chunk::{Chunk, CHUNK_SIZE},
        load_area::LoadArea,
        position_types::ChunkPos,
        Terrain,
    },
    util::face_index::{FaceIndex, FACE_NORMALS},
};

/// Cave-culling search based on https://tomcc.github.io/2014/08/31/visibility-2.html.
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
    load_area: &LoadArea,
    frustum_culling: &FrustumCullingRegions,
    camera_pos: Vec3,
) -> Vec<&'terrain Chunk> {
    // queue of chunks to render in order, along with the face they were visited from
    let mut step_queue = Vec::new();
    // index of the entry in `step_queue` that is currently being visited
    let mut step_index = 0;
    // whether each chunk in the loaded area has been added to the queue
    let mut seen = vec![false; load_area.size().product()];

    // start at the camera position
    let camera_chunk_pos = ChunkPos::from(
        (camera_pos / (CHUNK_SIZE as f32))
            .floor()
            .as_ivec3(),
    );
    let Some(camera_chunk) = load_area.get_chunk(terrain, &camera_chunk_pos) else {
        return Vec::new();
    };

    step_queue.push(SearchStep {
        chunk: camera_chunk,
        last_dir: None,
    });
    mark_seen(&mut seen, load_area, &camera_chunk_pos);

    while let Some(&step) = step_queue.get(step_index) {
        // vector pointing towards the camera
        let to_camera = (camera_chunk_pos - step.chunk.pos()).as_ivec3();

        // consider whether to explore the 6 neighbouring chunks
        step_queue.extend(
            (0..6)
                .filter(|&dir| {
                    if let Some(last_dir) = step.last_dir {
                        // only travel away from the camera
                        if IVec3::dot(FACE_NORMALS[dir] + FACE_NORMALS[last_dir], to_camera) < 0 {
                            // only visit chunks which are connected via the current chunk from the previous
                            // face to the current face
                            step.chunk
                                .visibility_graph()
                                .connected(FaceIndex(dir), FaceIndex(last_dir).opposite())
                        } else {
                            false
                        }
                    } else {
                        // explore all neighbours of the first chunk
                        true
                    }
                })
                // dir -> (dir, chunk_pos)
                .map(|dir| (dir, step.chunk.pos() + ChunkPos::from(FACE_NORMALS[dir])))
                .filter(|(_, chunk_pos)| load_area.contains_pos(chunk_pos))
                // don't visit the same chunk twice
                .filter(|(_, chunk_pos)| mark_seen(&mut seen, load_area, chunk_pos))
                // frustum culling
                .filter(|(_, chunk_pos)| frustum_culling.is_chunk_within_frustum(chunk_pos))
                // (dir, chunk_pos) -> SearchStep
                .filter_map(|(dir, chunk_pos)| {
                    load_area
                        .get_chunk(terrain, &chunk_pos)
                        .map(|chunk| SearchStep {
                            chunk,
                            last_dir: Some(dir),
                        })
                }),
        );

        step_index += 1;
    }

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

/// Returns true if this is the first time the chunk was seen
fn mark_seen(seen: &mut Vec<bool>, load_area: &LoadArea, chunk_pos: &ChunkPos) -> bool {
    let position_in_grid = (*chunk_pos - load_area.pos()).as_ivec3();

    if load_area
        .size()
        .contains_ivec3(position_in_grid)
    {
        let index = load_area
            .size()
            .flatten(position_in_grid.as_uvec3());

        let previously_visited = seen[index];
        seen[index] = true;
        !previously_visited
    } else {
        false
    }
}
