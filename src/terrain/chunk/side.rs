use std::sync::Arc;

use generational_arena::Index;

use super::{
    super::{
        block::BLOCKS,
        position_types::{ChunkPosition, LocalBlockPosition},
        Terrain,
    },
    Chunk, CHUNK_SIZE_SQUARED, CHUNK_SIZE_U32,
};
use crate::util::face::FaceIndex;

/// Represents a side of a chunk, storing whether each tile is solid (false) or empty (true).
/// The tiles are indexed as follows:
///   CHUNK_SIZE * V + U
/// where U goes in the direction of the first texture coordinate and V goes in the direction of
/// the second texture coordinate
#[derive(Clone, Debug)]
pub struct ChunkSide {
    pub faces: Arc<[bool; CHUNK_SIZE_SQUARED]>,
}

impl ChunkSide {
    pub fn px(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(CHUNK_SIZE_U32 - 1, v, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::POS_X).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    pub fn py(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(v, CHUNK_SIZE_U32 - 1, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::POS_Y).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    pub fn pz(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(u, v, CHUNK_SIZE_U32 - 1);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::POS_Z).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    pub fn nx(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(0, v, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::NEG_X).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    pub fn ny(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(v, 0, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::NEG_Y).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    pub fn nz(chunk: &Chunk) -> Self {
        let mut faces = [false; CHUNK_SIZE_SQUARED];
        let mut index = 0;

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(u, v, 0);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block.model.face(FaceIndex::NEG_Z).is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }

    /// Returns the sides of all chunks surrounding `chunk_pos`
    pub fn get_surrounding_sides(
        center_pos: ChunkPosition,
        terrain: &Terrain,
        load_area_index: Index,
    ) -> Vec<Option<ChunkSide>> {
        let side_px = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(1, 0, 0)))
            .map(ChunkSide::nx);
        let side_py = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 1, 0)))
            .map(ChunkSide::ny);
        let side_pz = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 0, 1)))
            .map(ChunkSide::nz);
        let side_nx = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(-1, 0, 0)),
            )
            .map(ChunkSide::px);
        let side_ny = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, -1, 0)),
            )
            .map(ChunkSide::py);
        let side_nz = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, 0, -1)),
            )
            .map(ChunkSide::pz);

        vec![side_px, side_py, side_pz, side_nx, side_ny, side_nz]
    }
}
