use derive_more::IsVariant;
use generational_arena::Index;
use glam::{Vec3, Vec3Swizzles};

use super::{chunk::Chunk, position_types::ChunkPos, Terrain};
use crate::util::size::{AsSize3, Size3};

/// An `LoadArea` represents a region of terrain that is loaded in memory.
/// The `LoadArea` provides O(1) lookup for the chunks it contains
#[derive(Clone, Debug)]
pub struct LoadArea {
    /// Lookup table for indices into the global chunk arena
    chunk_indices: Vec<Option<Index>>,
    /// Position of the lower corner of the area in chunks
    pos: ChunkPos,
    /// Size of the area in chunks
    size: Size3,
    /// Shape of the area
    shape: AreaShape,
    /// Status of the area with regards to chunk loading
    status: AreaStatus,
    /// Position of the center of the area in chunks
    center_pos: Vec3,
    /// Reciprocal of the size of the area in chunks,
    size_recip: Vec3,
}

impl LoadArea {
    pub fn new(pos: ChunkPos, size: Size3, shape: AreaShape) -> Self {
        Self {
            pos,
            size,
            shape,
            chunk_indices: vec![None; size.product()],
            status: AreaStatus::Dirty,
            center_pos: pos.as_vec3() + 0.5 * size.as_vec3(),
            size_recip: size.as_vec3().recip(),
        }
    }

    /// Returns the index of the chunk at the given position in the chunk arena
    /// In `dev` builds, panics if the chunk positions is outside of the bounds of the area
    /// The chunk is not guaranteed to still exist
    pub fn get_chunk_index(&self, chunk_pos: &ChunkPos) -> Option<Index> {
        self.chunk_indices[self.index_in_chunk_indices(chunk_pos)]
    }

    /// Returns the chunk at the given chunk position, if it is loaded
    pub fn get_chunk<'terrain>(
        &self,
        terrain: &'terrain Terrain,
        chunk_pos: &ChunkPos,
    ) -> Option<&'terrain Chunk> {
        self.get_chunk_index(chunk_pos)
            .and_then(|index| terrain.chunks().get(index))
    }

    /// Returns a mutable reference to the chunk at the given chunk position, if it is loaded
    pub fn get_chunk_mut<'terrain>(
        &self,
        terrain: &'terrain mut Terrain,
        chunk_pos: &ChunkPos,
    ) -> Option<&'terrain mut Chunk> {
        self.get_chunk_index(chunk_pos)
            .and_then(|index| terrain.chunks_mut().get_mut(index))
    }

    /// True if the area has an index for this chunk
    pub fn has_chunk_index(&self, chunk_pos: &ChunkPos) -> bool {
        self.get_chunk_index(chunk_pos)
            .is_some()
    }

    /// True if the chunk position is contained by the area
    pub fn contains_pos(&self, chunk_pos: &ChunkPos) -> bool {
        let v = (chunk_pos.as_vec3() - self.center_pos) * self.size_recip;
        match self.shape {
            AreaShape::Cubic => v.max_element().abs() <= 1.0,
            AreaShape::Spherical => v.length_squared() <= 1.0,
            AreaShape::Cylindrical => {
                let len_xz = v.xz().length();
                let abs_y = v.y.abs();
                len_xz <= 1.0 && abs_y <= 1.0
            }
        }
    }

    /// Iterator over all chunk positions contained by the area
    pub fn iter_positions<'a>(&'a self) -> impl Iterator<Item = ChunkPos> + 'a {
        let lower = self.pos.as_ivec3();
        let upper = self.pos.as_ivec3() + self.size.as_ivec3();

        itertools::iproduct!(lower.x..upper.x, lower.y..upper.y, lower.z..upper.z)
            .map(|(x, y, z)| ChunkPos::new(x, y, z))
            .filter(|chunk_pos| self.contains_pos(&chunk_pos))
    }

    /// Called when a chunk within the area is loaded
    pub(super) fn chunk_loaded(&mut self, chunk_pos: &ChunkPos, chunk_index: Index) {
        let index_in_chunk_indices = self.index_in_chunk_indices(chunk_pos);
        self.chunk_indices[index_in_chunk_indices] = Some(chunk_index);
    }

    /// Status of the area with regards to chunk loading
    pub fn status(&self) -> AreaStatus {
        self.status
    }

    /// Position of the lower corner of this area in chunks
    pub fn pos(&self) -> ChunkPos {
        self.pos
    }

    /// Size of this area in chunks
    pub fn size(&self) -> Size3 {
        self.size
    }

    /// Position of the center of this area in chunks
    pub fn center(&self) -> Vec3 {
        self.center_pos
    }

    /// Update the status of this area
    pub(super) fn set_status(&mut self, status: AreaStatus) {
        self.status = status;
    }

    /// Update the position of the lower corner of this area in chunks
    /// This will only mark the area as dirty if the new position is different
    pub fn set_pos(&mut self, new_pos: ChunkPos) {
        if new_pos != self.pos {
            self.pos = new_pos;
            self.center_pos = self.pos.as_vec3() + 0.5 * self.size.as_vec3();
            self.status = AreaStatus::Dirty;
        }
    }

    /// Update the size of this area in chunks
    /// This will only mark the area as dirty if the new position is different
    pub fn set_size(&mut self, new_size: Size3) {
        if new_size != self.size {
            self.size = new_size;
            self.status = AreaStatus::Dirty;
        }
    }

    /// Update the position of the center of this area in chunks
    /// This will only mark the area as dirty if the center has moved between chunks
    pub fn set_center(&mut self, center_pos: Vec3) {
        let pos = ChunkPos::from(center_pos.floor().as_ivec3() - self.size.as_ivec3() / 2);
        self.set_pos(pos);
    }

    /// For the given chunk position, returns the index in the global chunk arena for that chunk,
    /// if it exists.
    /// Assumes that the chunk position is within the area
    fn index_in_chunk_indices(&self, chunk_pos: &ChunkPos) -> usize {
        let grid_pos = chunk_pos
            .as_ivec3()
            .rem_euclid(self.size.as_ivec3())
            .as_uvec3();
        self.size.flatten(grid_pos)
    }
}

#[derive(Clone, Copy, Debug, IsVariant)]
pub enum AreaStatus {
    /// All chunks in the `LoadArea` are loaded or loading
    Clean,
    /// The `LoadArea` is new or just moved; chunks require loading and unloading
    Dirty,
}

#[derive(Clone, Copy, Debug)]
pub enum AreaShape {
    /// Chunks are loaded in a cuboid
    Cubic,
    /// Chunks are loaded in a spheriod
    Spherical,
    /// Chunks are loaded in a cylinder around the y axis
    Cylindrical,
}
