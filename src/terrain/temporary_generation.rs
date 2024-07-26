use bracket_noise::prelude::*;
use glam::UVec3;

use super::{
    block::{BlockId, BLOCK_DIRT, BLOCK_GRASS, BLOCK_LAMP_ORANGE},
    chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_CUBED, CHUNK_SIZE_U32},
    position_types::ChunkPosition,
};
use crate::util::size::Size3;

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

    for z in 0..CHUNK_SIZE_U32 {
        for x in 0..CHUNK_SIZE_U32 {
            let pos_above = UVec3::new(x, CHUNK_SIZE_U32, z).as_vec3() + chunk_offset;
            let noise_value_above = noise.get_noise3d(pos_above.x, pos_above.y, pos_above.z);
            let mut solid_above = noise_value_above > pos_above.y * 0.01;

            for y in 0..CHUNK_SIZE_U32 {
                let y = CHUNK_SIZE_U32 - 1 - y;
                let index = Size3::splat(CHUNK_SIZE).flatten(UVec3::new(x, z, y));

                let pos = UVec3::new(x, y, z).as_vec3() + chunk_offset;
                let noise_value = noise.get_noise3d(pos.x, pos.y, pos.z);

                if noise_value > pos.y * 0.01 {
                    let cave_noise = cave_noise.get_noise3d(pos.x, pos.y, pos.z);
                    if cave_noise < 0.4 {
                        if solid_above {
                            blocks[index] = BLOCK_DIRT;
                        } else {
                            blocks[index] = BLOCK_GRASS;
                        }
                    }
                    solid_above = true;

                    if rand::random::<f32>() > 0.998 {
                        blocks[index] = BLOCK_LAMP_ORANGE;
                    }
                }
            }
        }
    }

    Chunk::new(pos, &blocks)
}
