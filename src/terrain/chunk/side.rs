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
use crate::{
    terrain::lighting::{emitted_light::EmittedLight, skylight::Skylight},
    util::face::FaceIndex,
};

/// Represents a side of a chunk, storing whether each tile is solid (false) or empty (true).
/// The tiles are indexed as follows:
///   CHUNK_SIZE * V + U
/// where U goes in the direction of the first texture coordinate and V goes in the direction of
/// the second texture coordinate
#[derive(Clone, Debug)]
pub struct ChunkSideFaces {
    pub faces: Arc<[bool; CHUNK_SIZE_SQUARED]>,
}

impl ChunkSideFaces {
    pub fn px(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut faces = [false; CHUNK_SIZE_SQUARED];

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
    ) -> Vec<Option<ChunkSideFaces>> {
        let side_px = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(1, 0, 0)))
            .map(ChunkSideFaces::nx);
        let side_py = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 1, 0)))
            .map(ChunkSideFaces::ny);
        let side_pz = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 0, 1)))
            .map(ChunkSideFaces::nz);
        let side_nx = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(-1, 0, 0)),
            )
            .map(ChunkSideFaces::px);
        let side_ny = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, -1, 0)),
            )
            .map(ChunkSideFaces::py);
        let side_nz = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, 0, -1)),
            )
            .map(ChunkSideFaces::pz);

        vec![side_px, side_py, side_pz, side_nx, side_ny, side_nz]
    }
}

/// Represents a side of a chunk, the light data for each block on the edge
/// The tiles are indexed as follows:
///   CHUNK_SIZE * V + U
/// where U goes in the direction of the first texture coordinate and V goes in the direction of
/// the second texture coordinate
#[derive(Clone, Debug)]
pub struct ChunkSideLight {
    pub emitted: Arc<[EmittedLight; CHUNK_SIZE_SQUARED]>,
    pub sky: Arc<[Skylight; CHUNK_SIZE_SQUARED]>,
}

impl ChunkSideLight {
    pub fn px(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(CHUNK_SIZE_U32 - 1, v, u);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    pub fn py(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(v, CHUNK_SIZE_U32 - 1, u);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    pub fn pz(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(u, v, CHUNK_SIZE_U32 - 1);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    pub fn nx(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(0, v, u);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    pub fn ny(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(v, 0, u);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    pub fn nz(chunk: &Chunk) -> Self {
        let mut index = 0;
        let mut emitted = [EmittedLight::ZERO; CHUNK_SIZE_SQUARED];
        let mut sky = [Skylight::ZERO; CHUNK_SIZE_SQUARED];

        for v in 0..CHUNK_SIZE_U32 {
            for u in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = LocalBlockPosition::new(u, v, 0);

                emitted[index] = chunk.light_store.get_emitted_light(pos_in_chunk);
                sky[index] = chunk.light_store.get_skylight(pos_in_chunk);

                index += 1;
            }
        }

        Self {
            emitted: emitted.into(),
            sky: sky.into(),
        }
    }

    /// Returns the sides of all chunks surrounding `chunk_pos`
    pub fn get_surrounding_sides(
        center_pos: ChunkPosition,
        terrain: &Terrain,
        load_area_index: Index,
    ) -> Vec<Option<ChunkSideLight>> {
        let side_px = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(1, 0, 0)))
            .map(ChunkSideLight::nx);
        let side_py = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 1, 0)))
            .map(ChunkSideLight::ny);
        let side_pz = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPosition::new(0, 0, 1)))
            .map(ChunkSideLight::nz);
        let side_nx = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(-1, 0, 0)),
            )
            .map(ChunkSideLight::px);
        let side_ny = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, -1, 0)),
            )
            .map(ChunkSideLight::py);
        let side_nz = terrain
            .get_chunk(
                load_area_index,
                &(center_pos + ChunkPosition::new(0, 0, -1)),
            )
            .map(ChunkSideLight::pz);

        vec![side_px, side_py, side_pz, side_nx, side_ny, side_nz]
    }
}
