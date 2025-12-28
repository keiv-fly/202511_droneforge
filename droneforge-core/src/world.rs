use crate::block::BlockId;
use crate::chunk::{Chunk, ChunkBlocks, ChunkError};
use crate::coordinates::{ChunkPosition, LocalBlockCoord};
use crate::drone::DronePose;
use crate::inventory::{InventorySlots, add_block_to_slots, empty_inventory};
use crate::storage::{LoadBlocksFn, SaveBlocksFn, StorageError};
use crate::worldgen::DeterministicMap;
use std::collections::HashMap;

pub struct World {
    pub tick: u64,
    chunks: HashMap<ChunkPosition, Chunk>,
    drones: Vec<DronePose>,
    inventories: Vec<InventorySlots>,
    save_blocks: SaveBlocksFn,
    load_blocks: LoadBlocksFn,
}

impl World {
    pub fn new() -> Self {
        Self::new_with_callbacks(Self::default_save_blocks(), Self::default_load_blocks())
    }

    pub fn new_with_callbacks(save_blocks: SaveBlocksFn, load_blocks: LoadBlocksFn) -> Self {
        Self {
            tick: 0,
            chunks: HashMap::new(),
            drones: Vec::new(),
            inventories: Vec::new(),
            save_blocks,
            load_blocks,
        }
    }

    pub fn step(&mut self) {
        self.tick += 1;
    }

    pub fn drones(&self) -> &[DronePose] {
        &self.drones
    }

    pub fn drones_mut(&mut self) -> &mut [DronePose] {
        &mut self.drones
    }

    pub fn set_drones(&mut self, drones: Vec<DronePose>) {
        self.drones = drones;
        self.reset_inventories_for(self.drones.len());
    }

    pub fn add_drone(&mut self, drone: DronePose) {
        self.drones.push(drone);
        self.inventories.push(empty_inventory());
    }

    pub fn inventory(&self, drone_index: usize) -> Option<&InventorySlots> {
        self.inventories.get(drone_index)
    }

    pub fn inventory_mut(&mut self, drone_index: usize) -> Option<&mut InventorySlots> {
        self.inventories.get_mut(drone_index)
    }

    pub fn add_block_to_inventory(&mut self, drone_index: usize, block: BlockId) -> bool {
        let Some(slots) = self.inventory_mut(drone_index) else {
            return false;
        };
        add_block_to_slots(slots, block)
    }

    pub fn register_chunk(&mut self, position: ChunkPosition, default_block: BlockId) {
        self.chunks
            .entry(position)
            .or_insert_with(|| Chunk::new(position, default_block));
    }

    pub fn register_generated_chunk(
        &mut self,
        position: ChunkPosition,
        generator: &DeterministicMap,
    ) {
        self.chunks
            .entry(position)
            .or_insert_with(|| generator.chunk_for_position(position));
    }

    pub fn chunk(&self, position: &ChunkPosition) -> Option<&Chunk> {
        self.chunks.get(position)
    }

    pub fn set_block(
        &mut self,
        chunk: ChunkPosition,
        coord: LocalBlockCoord,
        block: BlockId,
    ) -> Result<(), ChunkError> {
        let chunk = self
            .chunks
            .get_mut(&chunk)
            .ok_or(ChunkError::InvalidBlockCount(0))?;
        chunk.set_block(coord, block)
    }

    pub fn save_chunk_blocks(&self, position: &ChunkPosition) -> Result<(), StorageError> {
        if let Some(chunk) = self.chunks.get(position) {
            let data = chunk.to_block_save();
            (self.save_blocks)(*position, data.blocks)
        } else {
            Err(StorageError::new("Chunk not found"))
        }
    }

    pub fn load_chunk_blocks(&mut self, position: &ChunkPosition) -> Result<bool, StorageError> {
        if let Some(loaded) = (self.load_blocks)(*position)? {
            if loaded.is_empty() {
                return Ok(false);
            }
            let chunk = self
                .chunks
                .get_mut(position)
                .ok_or_else(|| StorageError::new("Chunk not registered"))?;
            let data = ChunkBlocks::new(*position, loaded).map_err(|err| {
                StorageError::new(format!("Chunk error while loading: {:?}", err))
            })?;
            chunk
                .apply_block_save(&data)
                .map_err(|err| StorageError::new(format!("Failed to apply chunk: {:?}", err)))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_all_chunks(&mut self) -> Result<(), StorageError> {
        let positions: Vec<ChunkPosition> = self.chunks.keys().copied().collect();
        for position in positions {
            self.load_chunk_blocks(&position)?;
        }
        Ok(())
    }

    fn default_save_blocks() -> SaveBlocksFn {
        Box::new(|_, _| Ok(()))
    }

    fn default_load_blocks() -> LoadBlocksFn {
        Box::new(|_| Ok(None))
    }

    fn reset_inventories_for(&mut self, drone_count: usize) {
        self.inventories = vec![empty_inventory(); drone_count];
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldCoord;
    use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};
    use crate::worldgen::DeterministicMap;
    use crate::{INVENTORY_SLOTS, IRON, MAX_INVENTORY_UNITS, STONE};

    fn test_positions() -> ChunkPosition {
        ChunkPosition::new(0, 0, 0)
    }

    #[test]
    fn saves_chunk_blocks() {
        use std::sync::{Arc, Mutex};

        let position = test_positions();
        let saved = Arc::new(Mutex::new(Vec::new()));
        let saved_clone = Arc::clone(&saved);
        let save = Box::new(move |pos: ChunkPosition, blocks: Vec<BlockId>| {
            assert_eq!(pos, position);
            let mut guard = saved_clone.lock().unwrap();
            *guard = blocks;
            Ok(())
        });
        let load = Box::new(|_| Ok(None));

        let mut world = World::new_with_callbacks(save, load);
        world.register_chunk(position, 0);
        let coord = LocalBlockCoord::new(0, 0, 0);
        world.set_block(position, coord, 7).unwrap();
        world.save_chunk_blocks(&position).unwrap();

        let saved = saved.lock().unwrap();
        assert_eq!(saved.len(), CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT);
        assert_eq!(saved[0], 7);
    }

    #[test]
    fn loads_chunk_blocks() {
        let position = test_positions();
        let loaded_blocks = vec![1; CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT];
        let load = Box::new(move |pos: ChunkPosition| {
            assert_eq!(pos, position);
            Ok(Some(loaded_blocks.clone()))
        });
        let save = Box::new(|_, _| Ok(()));

        let mut world = World::new_with_callbacks(save, load);
        world.register_chunk(position, 0);
        world.load_chunk_blocks(&position).unwrap();
        let coord = LocalBlockCoord::new(1, 0, 0);
        assert_eq!(
            world
                .chunks
                .get(&position)
                .unwrap()
                .get_block(coord)
                .unwrap(),
            1
        );
    }

    #[test]
    fn load_all_chunks_uses_registered_positions() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let position = test_positions();
        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = Arc::clone(&calls);
        let load = Box::new(move |_| {
            call_counter.fetch_add(1, Ordering::SeqCst);
            Ok(Some(vec![2; CHUNK_WIDTH * CHUNK_DEPTH * CHUNK_HEIGHT]))
        });
        let save = Box::new(|_, _| Ok(()));

        let mut world = World::new_with_callbacks(save, load);
        world.register_chunk(position, 0);
        world.load_all_chunks().unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let coord = LocalBlockCoord::new(0, 0, 0);
        assert_eq!(
            world
                .chunks
                .get(&position)
                .unwrap()
                .get_block(coord)
                .unwrap(),
            2
        );
    }

    #[test]
    fn registers_generated_chunk() {
        let position = test_positions();
        let mut world = World::new();
        let generator = DeterministicMap::new(42);

        world.register_generated_chunk(position, &generator);

        let coord = LocalBlockCoord::new(10, 0, 0);
        assert_eq!(
            world
                .chunks
                .get(&position)
                .unwrap()
                .get_block(coord)
                .unwrap(),
            generator.block_at(WorldCoord::new(10, 0, 0))
        );
    }

    #[test]
    fn initializes_inventories_with_drone_count() {
        let mut world = World::new();
        world.set_drones(vec![
            DronePose::new([0.0, 0.0, 0.0], [1.0, 0.0], "d1", 10, 10),
            DronePose::new([1.0, 0.0, 0.0], [1.0, 0.0], "d2", 10, 10),
        ]);

        assert_eq!(world.inventories.len(), 2);
        assert_eq!(world.inventory(0).unwrap()[0].count, 0);
        assert_eq!(world.inventory(1).unwrap()[0].count, 0);
    }

    #[test]
    fn add_block_to_inventory_stacks_and_fills_new_slot() {
        let mut world = World::new();
        world.add_drone(DronePose::new([0.0, 0.0, 0.0], [1.0, 0.0], "d1", 10, 10));

        assert!(world.add_block_to_inventory(0, STONE));
        assert!(world.add_block_to_inventory(0, STONE));
        assert!(world.add_block_to_inventory(0, IRON));

        let slots = world.inventory(0).unwrap();
        let stone_slot = slots.iter().find(|slot| slot.block == Some(STONE)).unwrap();
        assert_eq!(stone_slot.count, 2);
        let iron_slot = slots.iter().find(|slot| slot.block == Some(IRON)).unwrap();
        assert_eq!(iron_slot.count, 1);
    }

    #[test]
    fn add_block_to_full_inventory_returns_false() {
        let mut world = World::new();
        world.add_drone(DronePose::new([0.0, 0.0, 0.0], [1.0, 0.0], "d1", 10, 10));

        for i in 0..INVENTORY_SLOTS {
            let block = (i as BlockId) + 10;
            assert!(world.add_block_to_inventory(0, block));
        }

        assert!(!world.add_block_to_inventory(0, 999));
    }

    #[test]
    fn add_block_to_inventory_respects_unit_capacity() {
        let mut world = World::new();
        world.add_drone(DronePose::new([0.0, 0.0, 0.0], [1.0, 0.0], "d1", 10, 10));

        for _ in 0..MAX_INVENTORY_UNITS {
            assert!(world.add_block_to_inventory(0, STONE));
        }

        assert!(!world.add_block_to_inventory(0, STONE));

        let total_units: u32 = world
            .inventory(0)
            .unwrap()
            .iter()
            .map(|slot| slot.count)
            .sum();

        assert_eq!(total_units, MAX_INVENTORY_UNITS);
    }
}
