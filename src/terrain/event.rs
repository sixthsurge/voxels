use super::position_types::ChunkPos;

#[derive(Clone, Debug)]
pub enum TerrainEvent {
    ChunkLoaded(ChunkPos),
    ChunkUnloaded(ChunkPos),
}
