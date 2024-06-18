use crate::terrain::lighting::PackedLightLevels;

/// Underlying storage mechanism for light levels in the chunk.
/// This is separated so that `Chunk` and its users don't have to worry about the underlying
/// mechanism
#[derive(Clone, Debug)]
pub struct ChunkLightStorage {
    data: Vec<PackedLightLevels>,
}

impl ChunkLightStorage {
    pub fn new(data: Vec<PackedLightLevels>) -> Self {
        Self { data }
    }
}
