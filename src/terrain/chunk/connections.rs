use std::collections::VecDeque;

use glam::UVec3;

use super::super::{
    block::{BlockId, BLOCKS},
    chunk::{CHUNK_SIZE_CUBED, CHUNK_SIZE_U32},
    position_types::LocalBlockPosition,
};
use crate::util::face::{FaceIndex, FACE_NORMALS};

/// "Visibility graph" from https://tomcc.github.io/2014/08/31/visibility-1.html
/// For each pair of faces, stores whether the faces are connected by non-solid blocks
#[derive(Clone, Copy, Debug)]
pub struct ChunkConnections(u16);

impl ChunkConnections {
    /// Compute the connections for the given block array
    pub fn compute(blocks: &[BlockId]) -> Self {
        let mut connection_bits: u16 = 0;
        let mut explored = [false; CHUNK_SIZE_CUBED];
        let mut frontier = VecDeque::new();

        // iterate over all blocks on the edge of the chunk to start the flood fills
        // the article recommends to iterate over every block in the chunk, but I think this might
        // waste time exploring enclosed regions? maybe I'm missing something
        for chunk_face in 0..6 {
            for v in 0..CHUNK_SIZE_U32 {
                for u in 0..CHUNK_SIZE_U32 {
                    // whether the search escaped the chunk in each direction
                    let mut escaped = [false; 6];

                    // position to start the search at
                    let start_pos = LocalBlockPosition::from(
                        FACE_START[chunk_face]
                            + FACE_DIR_U[chunk_face] * u
                            + FACE_DIR_V[chunk_face] * v,
                    );
                    let array_index = start_pos.get_array_index();

                    // skip this search if the start position was already explored
                    if explored[array_index] {
                        continue;
                    }

                    // skip opaque blocks
                    if BLOCKS[blocks[array_index].0 as usize].model.is_opaque() {
                        continue;
                    }

                    // flood fill starting at `start_pos`
                    frontier.push_back(start_pos);
                    while let Some(block_pos) = frontier.pop_front() {
                        if explored[block_pos.get_array_index()] {
                            continue;
                        }
                        explored[block_pos.get_array_index()] = true;

                        for block_face in 0..6 {
                            if let Some(neighbour_pos) = block_pos.try_add(FACE_NORMALS[block_face])
                            {
                                let array_index = neighbour_pos.get_array_index();

                                // if this position is unexplored and non-opaque, add it to the
                                // frontier
                                if explored[array_index] {
                                    continue;
                                }
                                if BLOCKS[blocks[array_index].0 as usize].model.is_opaque() {
                                    continue;
                                }
                                frontier.push_back(neighbour_pos);
                            } else {
                                // escaped in this direction!
                                escaped[block_face] = true;
                            }
                        }
                    }

                    // update the connection bits
                    // +x +y
                    connection_bits |= ((escaped[0] && escaped[1]) as u16) << 0;
                    // +x +z
                    connection_bits |= ((escaped[0] && escaped[2]) as u16) << 1;
                    // +x -x
                    connection_bits |= ((escaped[0] && escaped[3]) as u16) << 2;
                    // +x -y
                    connection_bits |= ((escaped[0] && escaped[4]) as u16) << 3;
                    // +x -z
                    connection_bits |= ((escaped[0] && escaped[5]) as u16) << 4;
                    // +y +z
                    connection_bits |= ((escaped[1] && escaped[2]) as u16) << 5;
                    // +y -x
                    connection_bits |= ((escaped[1] && escaped[3]) as u16) << 6;
                    // +y -y
                    connection_bits |= ((escaped[1] && escaped[4]) as u16) << 7;
                    // +y -z
                    connection_bits |= ((escaped[1] && escaped[5]) as u16) << 8;
                    // +z -x
                    connection_bits |= ((escaped[2] && escaped[3]) as u16) << 9;
                    // +z -y
                    connection_bits |= ((escaped[2] && escaped[4]) as u16) << 10;
                    // +z -z
                    connection_bits |= ((escaped[2] && escaped[5]) as u16) << 11;
                    // -x -y
                    connection_bits |= ((escaped[3] && escaped[4]) as u16) << 12;
                    // -x -z
                    connection_bits |= ((escaped[3] && escaped[5]) as u16) << 13;
                    // -y -z
                    connection_bits |= ((escaped[4] && escaped[5]) as u16) << 14;

                    // NB: `frontier` should already be empty at this point, so we don't need to
                    // clear it between usages
                }
            }
        }

        Self(connection_bits)
    }

    /// True if face A is connected to face B through non-opaque blocks.
    /// We assume that if ¬connected(face_a, face_b) then face_b cannot be visible through face_a
    /// and vice-versa
    pub fn connected(&self, face_a: FaceIndex, face_b: FaceIndex) -> bool {
        let connection_index = CONNECTION_INDICES[face_a.as_usize() * 6 + face_b.as_usize()];

        (self.0 & (1 << connection_index)) != 0
    }
}

// formulae for the block positions in each face:
// FACE_START[i] + FACE_DIR_U[i] * u + FACE_DIR_V[i] * v
const FACE_START: [UVec3; 6] = [
    UVec3::ZERO,
    UVec3::ZERO,
    UVec3::ZERO,
    UVec3::new(0, 0, CHUNK_SIZE_U32 - 1),
    UVec3::new(0, CHUNK_SIZE_U32 - 1, 0),
    UVec3::new(CHUNK_SIZE_U32 - 1, 0, 0),
];
const FACE_DIR_U: [UVec3; 6] = [
    UVec3::new(1, 0, 0),
    UVec3::new(1, 0, 0),
    UVec3::new(0, 1, 0),
    UVec3::new(1, 0, 0),
    UVec3::new(1, 0, 0),
    UVec3::new(0, 1, 0),
];
const FACE_DIR_V: [UVec3; 6] = [
    UVec3::new(0, 1, 0),
    UVec3::new(0, 0, 1),
    UVec3::new(0, 0, 1),
    UVec3::new(0, 1, 0),
    UVec3::new(0, 0, 1),
    UVec3::new(0, 0, 1),
];

// bit indices in the connection flags for each possible pair of faces
#[rustfmt::skip]
const CONNECTION_INDICES: [u16; 36] = [
    // +x
    15, 0, 1, 2, 3, 4,
    // +y
    0, 15, 5, 6, 7, 8,
    // +z
    1, 5, 15, 9, 10, 11,
    // -x
    2, 6, 9, 15, 12, 13,
    // -y
    3, 7, 10, 12, 15, 14,
    // -z
    4, 8, 11, 13, 14, 15,
];
