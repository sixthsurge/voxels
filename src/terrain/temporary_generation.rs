use bracket_noise::prelude::*;
use glam::UVec3;

use crate::block::{BlockId, BLOCK_HAPPY, BLOCK_SAD};

use super::{
    chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_CUBED, CHUNK_SIZE_U32},
    position_types::ChunkPos,
};

pub fn generate_chunk(pos: ChunkPos) -> Chunk {
    let mut blocks: Vec<_> = (0..CHUNK_SIZE_CUBED)
        .map(|_| BlockId(0))
        .collect();

    let chunk_offset = pos.as_vec3() * (CHUNK_SIZE as f32);

    let mut noise = FastNoise::seeded(1);
    noise.set_noise_type(NoiseType::Simplex);
    noise.set_frequency(0.025);

    let mut noise2 = FastNoise::seeded(2);
    noise2.set_frequency(0.05);

    let mut index = 0;
    for z in 0..CHUNK_SIZE_U32 {
        for y in 0..CHUNK_SIZE_U32 {
            for x in 0..CHUNK_SIZE_U32 {
                let pos = UVec3::new(x, y, z).as_vec3() + chunk_offset;
                let noise_value = noise.get_noise3d(pos.x, pos.y, pos.z);
                if noise_value > 0.0 {
                    if noise2.get_noise3d(pos.x, pos.y, pos.z) > 0.0 {
                        blocks[index] = BLOCK_HAPPY;
                    } else {
                        blocks[index] = BLOCK_SAD;
                    }
                }
                index += 1;
            }
        }
    }

    Chunk::new(pos, blocks)
}
