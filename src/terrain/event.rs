use super::position_types::{ChunkPos, LocalBlockPos};

#[derive(Clone, Debug)]
pub enum TerrainEvent {
    ChunkLoaded(ChunkPos),
    ChunkUnloaded(ChunkPos),
    BlockModified(ChunkPos, LocalBlockPos),
}
