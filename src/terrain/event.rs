use super::position_types::{ChunkPosition, LocalBlockPosition};

#[derive(Clone, Debug)]
pub enum TerrainEvent {
    ChunkLoaded(ChunkPosition),
    ChunkUnloaded(ChunkPosition),
    BlockModified(ChunkPosition, LocalBlockPosition),
    ChunkLightUpdate(ChunkPosition),
}
