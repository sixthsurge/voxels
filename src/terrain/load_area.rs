use derive_more::IsVariant;
use generational_arena::Index;
use glam::{Vec3, Vec3Swizzles};

use super::position_types::ChunkPosition;
use crate::util::{size::Size3, vector_map::VectorMapExt};

/// An `LoadArea` represents a region of terrain that is loaded in memory.
/// The `LoadArea` provides O(1) lookup for the chunks it contains
#[derive(Clone, Debug)]
pub struct LoadArea {
    /// State of each chunk in the area
    chunk_states: Vec<ChunkState>,
    /// Position of the lower corner of the area in chunks
    position: ChunkPosition,
    /// Size of the area in chunks
    size: Size3,
    /// Shape of the area
    shape: AreaShape,
    /// Status of the area with regards to chunk loading
    state: LoadAreaState,
    /// Position of the center of the area in chunks
    center_pos: Vec3,
    /// Reciprocal of the size of the area in chunks,
    size_recip: Vec3,
}

impl LoadArea {
    pub fn new(pos: ChunkPosition, size: Size3, shape: AreaShape) -> Self {
        Self {
            chunk_states: vec![ChunkState::Unloaded; size.product()],
            position: pos,
            size,
            shape,
            state: LoadAreaState::Dirty,
            center_pos: pos.as_vec3() + 0.5 * size.as_vec3(),
            size_recip: size.as_vec3().recip(),
        }
    }

    /// If the given chunk position is within the bounds of this area and the chunk is loaded,
    /// returns index of the given chunk in the chunk arena.
    /// Otherwise returns None
    pub fn get_chunk_index(&self, chunk_pos: &ChunkPosition) -> Option<Index> {
        self.get_array_index(chunk_pos)
            .and_then(|array_index| match self.chunk_states[array_index] {
                ChunkState::Unloaded | ChunkState::Loading(_) => None,
                ChunkState::Loaded(pos, index) => {
                    if pos == *chunk_pos {
                        Some(index)
                    } else {
                        None
                    }
                }
            })
    }

    /// True if the given chunk in the area is loaded
    pub fn is_loaded(&self, chunk_pos: &ChunkPosition) -> bool {
        self.get_array_index(chunk_pos)
            .map(|array_index| match self.chunk_states[array_index] {
                ChunkState::Unloaded | ChunkState::Loading(_) => false,
                ChunkState::Loaded(pos, _) => pos == *chunk_pos,
            })
            .unwrap_or(false)
    }

    /// True if the given chunk in the area is currently loading
    pub fn is_loading(&self, chunk_pos: &ChunkPosition) -> bool {
        self.get_array_index(chunk_pos)
            .map(|array_index| match self.chunk_states[array_index] {
                ChunkState::Unloaded | ChunkState::Loaded(_, _) => false,
                ChunkState::Loading(pos) => pos == *chunk_pos,
            })
            .unwrap_or(false)
    }

    /// True if the chunk at the given position is neither loaded or loading, or isn't contained in
    /// the area's bounds
    pub fn is_unloaded(&self, chunk_pos: &ChunkPosition) -> bool {
        self.get_array_index(chunk_pos)
            .map(|array_index| match self.chunk_states[array_index] {
                ChunkState::Unloaded => true,
                ChunkState::Loading(pos) => pos != *chunk_pos,
                ChunkState::Loaded(pos, _) => pos != *chunk_pos,
            })
            .unwrap_or(true)
    }

    /// True if the chunk position is contained by the area
    pub fn is_within_area(&self, chunk_pos: &ChunkPosition) -> bool {
        if !self.is_within_bounds(chunk_pos) {
            return false;
        }

        let v = (chunk_pos.as_vec3() - self.center_pos) * self.size_recip;
        match self.shape {
            AreaShape::Cubic => v.abs().max_element() <= 0.5,
            AreaShape::Spherical => v.length_squared() <= 0.25,
            AreaShape::Cylindrical => {
                let len_sq_xz = v.xz().length_squared();
                let abs_y = v.y.abs();
                len_sq_xz <= 0.25 && abs_y <= 0.5
            }
        }
    }

    /// True if the chunk position is contained by the bounding box of the area (faster test)
    pub fn is_within_bounds(&self, chunk_pos: &ChunkPosition) -> bool {
        self.size
            .contains_ivec3((*chunk_pos - self.position).as_ivec3())
    }

    /// Iterator over all chunk positions contained by the area
    pub fn iter_positions<'a>(&'a self) -> impl Iterator<Item = ChunkPosition> + 'a {
        let lower = self.position.as_ivec3();
        let upper = self.position.as_ivec3() + self.size.as_ivec3();

        itertools::iproduct!(lower.x..upper.x, lower.y..upper.y, lower.z..upper.z)
            .map(|(x, y, z)| ChunkPosition::new(x, y, z))
            .filter(|chunk_pos| self.is_within_area(&chunk_pos))
    }

    /// State of the area with regards to chunk loading
    pub fn state(&self) -> LoadAreaState {
        self.state
    }

    /// Position of the lower corner of this area in chunks
    pub fn position(&self) -> ChunkPosition {
        self.position
    }

    /// Size of this area in chunks
    pub fn size(&self) -> Size3 {
        self.size
    }

    /// Position of the center of this area in chunks
    pub fn center(&self) -> Vec3 {
        self.center_pos
    }

    /// Update the state of this area
    pub(super) fn set_state(&mut self, state: LoadAreaState) {
        self.state = state;
    }

    /// Update the position of the lower corner of this area in chunks
    /// This will only mark the area as dirty if the new position is different
    pub fn set_pos(&mut self, new_pos: ChunkPosition) {
        if new_pos != self.position {
            self.position = new_pos;
            self.center_pos = self.position.as_vec3() + 0.5 * self.size.as_vec3();
            self.state = LoadAreaState::Dirty;
        }
    }

    /// Update the size of this area in chunks
    /// This will only mark the area as dirty if the new position is different
    pub fn set_size(&mut self, new_size: Size3) {
        if new_size != self.size {
            self.size = new_size;
            self.state = LoadAreaState::Dirty;
        }
    }

    /// Update the position of the center of this area in chunks
    /// This will only mark the area as dirty if the center has moved between chunks
    pub fn set_center(&mut self, center_pos: Vec3) {
        let pos = ChunkPosition::from(center_pos.floor().as_ivec3() - self.size.as_ivec3() / 2);
        self.set_pos(pos);
    }

    /// Called when a chunk within the area is loaded
    pub(super) fn mark_loaded(&mut self, chunk_pos: &ChunkPosition, chunk_index: Index) {
        let array_index = self
            .get_array_index(chunk_pos)
            .expect("chunk_pos should be within the load area's bounds");

        self.chunk_states[array_index] = ChunkState::Loaded(*chunk_pos, chunk_index);
    }

    /// Called when a chunk within the area is queued for loading
    pub(super) fn mark_loading(&mut self, chunk_pos: &ChunkPosition) {
        let array_index = self
            .get_array_index(chunk_pos)
            .expect("chunk_pos should be within the load area's bounds");

        self.chunk_states[array_index] = ChunkState::Loading(*chunk_pos);
    }

    /// Called when a chunk within the area is unloaded
    pub(super) fn mark_unloaded(&mut self, chunk_pos: &ChunkPosition) {
        let array_index = self
            .get_array_index(chunk_pos)
            .expect("chunk_pos should be within the load area's bounds");

        self.chunk_states[array_index] = ChunkState::Unloaded;
    }

    /// If the chunk position is within the area's bounds, returns the index in `self.chunks` for
    /// that chunk
    fn get_array_index(&self, chunk_pos: &ChunkPosition) -> Option<usize> {
        if self.is_within_bounds(chunk_pos) {
            let grid_pos = chunk_pos
                .as_ivec3()
                .rem_euclid(self.size.as_ivec3())
                .as_uvec3();

            Some(self.size.flatten(grid_pos))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, IsVariant)]
pub enum LoadAreaState {
    Clean,
    Dirty,
}

#[derive(Clone, Copy, Debug, IsVariant)]
pub enum ChunkState {
    /// The chunk is not loaded, loading or queued for loading
    Unloaded,
    /// The chunk is loading or queued for loading
    Loading(ChunkPosition),
    /// Flatten a 2D grid position into an index in a 1D array ordered by y then x
    Loaded(ChunkPosition, Index),
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
