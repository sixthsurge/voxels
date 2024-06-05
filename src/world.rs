use crate::chunk::Chunk;

use self::event::WorldEvent;

pub mod event;

pub struct World {
    chunk: Chunk,
    events: Vec<WorldEvent>,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            events: Vec::new(),
        }
    }

    /// Returns an iterator over all `WorldEvent`s that have occurred since the last call to
    /// `clear_events()`
    pub fn events(&self) -> impl Iterator<Item = &WorldEvent> {
        self.events.iter()
    }

    /// Clear the `WorldEvent`s
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}
