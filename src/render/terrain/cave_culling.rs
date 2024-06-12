use std::collections::VecDeque;

use glam::{IVec3, Vec3};
use itertools::Itertools;
use rustc_hash::FxHashSet;

use crate::{
    terrain::{
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
pub fn visibility_search(
    terrain: &Terrain,
    camera_pos: Vec3,
    camera_look_dir: Vec3,
) -> Vec<&Chunk> {
    // queue of chunks to render in order, along with the face they were visited from
    let mut render_queue = Vec::<(usize, &Chunk)>::new();
    // index of the chunk in `render_queue` that is currently being visited
    let mut visited_index = 1;
    // index of the chunk in `render_queue` that is currently being visited
    // will track the positions of chunks that have already been added to the frontier
    let mut explored = FxHashSet::default();

    // start at the camera position
    let camera_chunk_pos = ChunkPos::from(
        (camera_pos / (CHUNK_SIZE as f32))
            .floor()
            .as_ivec3(),
    );
    let Some(camera_chunk) = terrain.get_chunk(camera_chunk_pos) else {
        return render_queue
            .iter()
            .map(|&(_, chunk)| chunk)
            .collect_vec();
    };

    render_queue.push((0, camera_chunk));
    explored.insert(camera_chunk_pos);

    // add the initial neighbours
    render_queue.extend(
        (0..6)
            // face -> (face, chunk_pos)
            .map(|face| {
                (
                    face,
                    camera_chunk.pos() + ChunkPos::from(FACE_NORMALS[face]),
                )
            })
            // don't visit the same chunk twice
            .filter(|&(_, chunk_pos)| explored.insert(chunk_pos))
            // (face, chunk_pos) -> (face, chunk) if the chunk exists
            .filter_map(|(face, chunk_pos)| {
                terrain
                    .get_chunk(chunk_pos)
                    .map(|chunk| (face, chunk))
            }),
    );

    // breadth-first search
    while let Some(&(from_face, chunk)) = render_queue.get(visited_index) {
        // consider whether to explore the 6 neighbouring chunks
        render_queue.extend(
            (0..6)
                // don't travel backwards
                .filter(|&face| IVec3::dot(FACE_NORMALS[face], FACE_NORMALS[from_face]) >= 0)
                // only visit chunks which are connected via the current chunk from the previous
                // face to the current face
                .filter(|&face| {
                    chunk
                        .visibility_graph()
                        .connected(FaceIndex(face), FaceIndex(from_face).opposite())
                })
                // face -> (face, chunk_pos)
                .map(|face| (face, chunk.pos() + ChunkPos::from(FACE_NORMALS[face])))
                // don't visit the same chunk twice
                .filter(|&(_, chunk_pos)| explored.insert(chunk_pos))
                // (face, chunk_pos) -> (face, chunk) if the chunk exists
                .filter_map(|(face, chunk_pos)| {
                    terrain
                        .get_chunk(chunk_pos)
                        .map(|chunk| (face, chunk))
                }),
        );

        visited_index += 1;
    }

    log::info!(
        "rendering {} out of {} chunks",
        render_queue.len(),
        terrain.loaded_chunks().len()
    );

    return render_queue
        .iter()
        .map(|&(_, chunk)| chunk)
        .collect_vec();
}
