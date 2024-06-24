use bracket_noise::prelude::*;
use glam::UVec3;

use super::{
    chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_CUBED, CHUNK_SIZE_U32},
    position_types::ChunkPosition,
};
use crate::block::{BlockId, BLOCK_DIRT, BLOCK_GRASS};

pub fn generate_chunk(pos: ChunkPosition) -> Chunk {
    let mut blocks = vec![BlockId(0); CHUNK_SIZE_CUBED];

    let chunk_offset = pos.as_vec3() * (CHUNK_SIZE as f32);

    let mut noise = FastNoise::seeded(1);
    noise.set_noise_type(NoiseType::SimplexFractal);
    noise.set_fractal_octaves(7);
    noise.set_frequency(0.003);

    let mut cave_noise = FastNoise::seeded(2);
    cave_noise.set_noise_type(NoiseType::SimplexFractal);
    cave_noise.set_fractal_octaves(3);
    cave_noise.set_frequency(0.03);

    let mut index = 0;
    for z in 0..CHUNK_SIZE_U32 {
        for y in 0..CHUNK_SIZE_U32 {
            for x in 0..CHUNK_SIZE_U32 {
                let pos = UVec3::new(x, y, z).as_vec3() + chunk_offset;
                let noise_value = noise.get_noise3d(pos.x, pos.y, pos.z);
                let noise_value_above = noise.get_noise3d(pos.x, pos.y + 1.0, pos.z);
                if noise_value > (y as f32 + chunk_offset.y) * 0.01 {
                    let cave_noise = cave_noise.get_noise3d(pos.x, pos.y, pos.z);
                    if cave_noise < 0.4 {
                        if noise_value_above > ((y + 1) as f32 + chunk_offset.y) * 0.01 {
                            blocks[index] = BLOCK_DIRT;
                        } else {
                            blocks[index] = BLOCK_GRASS;
                        }
                    }
                }
                index += 1;
            }
        }
    }

    Chunk::new(pos, blocks)
}
