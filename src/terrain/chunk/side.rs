use std::sync::Arc;

use super::{Chunk, CHUNK_SIZE_SQUARED, CHUNK_SIZE_U32};
use crate::{block::BLOCKS, terrain::position_types::LocalBlockPos, util::face_index::FaceIndex};

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
                let pos_in_chunk = LocalBlockPos::new(CHUNK_SIZE_U32 - 1, v, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::POS_X)
                    .is_none();
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
                let pos_in_chunk = LocalBlockPos::new(v, CHUNK_SIZE_U32 - 1, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::POS_Y)
                    .is_none();
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
                let pos_in_chunk = LocalBlockPos::new(u, v, CHUNK_SIZE_U32 - 1);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::POS_Z)
                    .is_none();
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
                let pos_in_chunk = LocalBlockPos::new(0, v, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::NEG_X)
                    .is_none();
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
                let pos_in_chunk = LocalBlockPos::new(v, 0, u);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::NEG_Y)
                    .is_none();
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
                let pos_in_chunk = LocalBlockPos::new(u, v, 0);
                let block_id = chunk.get_block(pos_in_chunk);
                let block = &BLOCKS[block_id.0 as usize];
                faces[index] = block
                    .model
                    .face(FaceIndex::NEG_Z)
                    .is_none();
                index += 1;
            }
        }

        Self {
            faces: faces.into(),
        }
    }
}
