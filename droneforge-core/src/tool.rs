use crate::block::{AIR, BlockId};
use crate::chunk::ChunkError;
use crate::chunk_cache::ChunkCache;
use crate::coordinates::WorldCoord;
use crate::inventory::{InventorySlots, remove_block_from_slot, slot_block};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSelection {
    pub block: BlockId,
    pub slot_index: usize,
    pub remaining: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementOutcome {
    pub placed_block: BlockId,
    pub target: WorldCoord,
    pub remaining_in_slot: u32,
    pub selection_cleared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementErrorReason {
    NoSelection,
    SlotEmpty,
    SlotMismatch,
    DifferentLevel,
    TooFar,
    SameTile,
    TargetBlocked,
    TargetUnloaded,
    Chunk(ChunkError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementError {
    pub reason: PlacementErrorReason,
}

impl PlacementError {
    pub fn new(reason: PlacementErrorReason) -> Self {
        Self { reason }
    }

    pub fn message(&self) -> &'static str {
        match self.reason {
            PlacementErrorReason::NoSelection => "select a block to place",
            PlacementErrorReason::SlotEmpty => "selected slot is empty",
            PlacementErrorReason::SlotMismatch => "selected slot changed",
            PlacementErrorReason::DifferentLevel => "target must be on the same level",
            PlacementErrorReason::TooFar => "target must be adjacent to the drone",
            PlacementErrorReason::SameTile => "cannot place on the drone's tile",
            PlacementErrorReason::TargetBlocked => "target tile is not empty",
            PlacementErrorReason::TargetUnloaded => "target tile is not loaded",
            PlacementErrorReason::Chunk(_) => "failed to save placed block",
        }
    }
}

impl From<ChunkError> for PlacementError {
    fn from(reason: ChunkError) -> Self {
        Self {
            reason: PlacementErrorReason::Chunk(reason),
        }
    }
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            PlacementErrorReason::Chunk(err) => {
                write!(f, "{} ({:?})", self.message(), err)
            }
            _ => f.write_str(self.message()),
        }
    }
}

impl Error for PlacementError {}

#[derive(Debug, Clone)]
pub struct ToolController {
    selection: Option<ToolSelection>,
    max_distance: i32,
}

impl ToolController {
    pub fn new() -> Self {
        Self::with_max_distance(1)
    }

    pub fn with_max_distance(max_distance: i32) -> Self {
        Self {
            selection: None,
            max_distance: max_distance.max(0),
        }
    }

    pub fn selection(&self) -> Option<ToolSelection> {
        self.selection
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn select_from_inventory(
        &mut self,
        slots: &InventorySlots,
        slot_index: usize,
    ) -> Option<ToolSelection> {
        let selection = slot_block(slots, slot_index).map(|(block, remaining)| ToolSelection {
            block,
            slot_index,
            remaining,
        });
        self.selection = selection;
        selection
    }

    pub fn refresh_from_inventory(&mut self, slots: &InventorySlots) {
        if let Some(current) = self.selection {
            let refreshed = slot_block(slots, current.slot_index).and_then(|(block, count)| {
                (block == current.block).then_some(ToolSelection {
                    block,
                    slot_index: current.slot_index,
                    remaining: count,
                })
            });
            self.selection = refreshed;
        }
    }

    pub fn place_selected_block(
        &mut self,
        slots: &mut InventorySlots,
        chunk_cache: &mut ChunkCache,
        drone_tile: WorldCoord,
        target_tile: WorldCoord,
    ) -> Result<PlacementOutcome, PlacementError> {
        let selection = self
            .selection
            .ok_or_else(|| PlacementError::new(PlacementErrorReason::NoSelection))?;

        self.validate_target(drone_tile, target_tile)?;

        let current_block = chunk_cache
            .block_at_world(target_tile)
            .ok_or_else(|| PlacementError::new(PlacementErrorReason::TargetUnloaded))?;
        if current_block != AIR {
            return Err(PlacementError::new(PlacementErrorReason::TargetBlocked));
        }

        let Some(removed_block) = remove_block_from_slot(slots, selection.slot_index) else {
            self.refresh_from_inventory(slots);
            return Err(PlacementError::new(PlacementErrorReason::SlotEmpty));
        };

        if removed_block != selection.block {
            self.refresh_from_inventory(slots);
            return Err(PlacementError::new(PlacementErrorReason::SlotMismatch));
        }

        chunk_cache.set_block(target_tile, removed_block)?;
        self.refresh_from_inventory(slots);

        let remaining_in_slot = slots
            .get(selection.slot_index)
            .filter(|slot| slot.block == Some(selection.block))
            .map(|slot| slot.count)
            .unwrap_or(0);

        let selection_cleared = self.selection.is_none();

        Ok(PlacementOutcome {
            placed_block: removed_block,
            target: target_tile,
            remaining_in_slot,
            selection_cleared,
        })
    }

    fn validate_target(
        &self,
        drone_tile: WorldCoord,
        target_tile: WorldCoord,
    ) -> Result<(), PlacementError> {
        if drone_tile.z != target_tile.z {
            return Err(PlacementError::new(PlacementErrorReason::DifferentLevel));
        }

        let dx = (target_tile.x - drone_tile.x).abs();
        let dy = (target_tile.y - drone_tile.y).abs();

        if dx == 0 && dy == 0 {
            return Err(PlacementError::new(PlacementErrorReason::SameTile));
        }

        if dx > self.max_distance || dy > self.max_distance {
            return Err(PlacementError::new(PlacementErrorReason::TooFar));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, STONE};
    use crate::coordinates::ChunkPosition;
    use crate::inventory::{InventorySlot, empty_inventory};
    use crate::worldgen::DeterministicMap;

    fn cache_with_air_at(target: WorldCoord) -> ChunkCache {
        let mut cache = ChunkCache::with_capacity(1);
        let generator = DeterministicMap::new(7);
        let chunk_pos = ChunkPosition::new(0, 0, 0);
        cache.populate_chunk_at(&generator, chunk_pos);
        // Ensure the target is empty regardless of generator output.
        let _ = cache.set_block(target, AIR);
        cache
    }

    #[test]
    fn places_block_when_selection_valid() {
        let target = WorldCoord::new(0, 0, 0);
        let drone_tile = WorldCoord::new(1, 0, 0);
        let mut cache = cache_with_air_at(target);

        let mut slots = empty_inventory();
        slots[0] = InventorySlot {
            block: Some(STONE),
            count: 2,
        };

        let mut controller = ToolController::new();
        controller.select_from_inventory(&slots, 0);

        let outcome = controller
            .place_selected_block(&mut slots, &mut cache, drone_tile, target)
            .expect("placement should succeed");

        assert_eq!(cache.block_at_world(target), Some(STONE));
        assert_eq!(outcome.remaining_in_slot, 1);
        assert!(!outcome.selection_cleared);
        assert_eq!(controller.selection().unwrap().remaining, 1);
    }

    #[test]
    fn rejects_when_tile_blocked() {
        let target = WorldCoord::new(0, 0, 0);
        let drone_tile = WorldCoord::new(1, 0, 0);
        let mut cache = cache_with_air_at(target);
        // Block the tile.
        cache.set_block(target, STONE).unwrap();

        let mut slots = empty_inventory();
        slots[0] = InventorySlot {
            block: Some(STONE),
            count: 1,
        };

        let mut controller = ToolController::new();
        controller.select_from_inventory(&slots, 0);

        let err = controller
            .place_selected_block(&mut slots, &mut cache, drone_tile, target)
            .unwrap_err();
        assert_eq!(err.reason, PlacementErrorReason::TargetBlocked);
    }

    #[test]
    fn clears_selection_when_slot_empties() {
        let target = WorldCoord::new(2, 0, 0);
        let drone_tile = WorldCoord::new(1, 0, 0);
        let mut cache = cache_with_air_at(target);

        let mut slots = empty_inventory();
        slots[0] = InventorySlot {
            block: Some(STONE),
            count: 1,
        };

        let mut controller = ToolController::new();
        controller.select_from_inventory(&slots, 0);

        let outcome = controller
            .place_selected_block(&mut slots, &mut cache, drone_tile, target)
            .expect("placement should succeed");

        assert!(outcome.selection_cleared);
        assert!(controller.selection().is_none());
        assert_eq!(slots[0].count, 0);
        assert!(slots[0].block.is_none());
    }
}
