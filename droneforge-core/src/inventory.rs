use crate::block::BlockId;

pub const INVENTORY_SLOTS: usize = 10;
pub const MAX_INVENTORY_UNITS: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventorySlot {
    pub block: Option<BlockId>,
    pub count: u32,
}

pub type InventorySlots = [InventorySlot; INVENTORY_SLOTS];

pub fn empty_inventory() -> InventorySlots {
    [InventorySlot::default(); INVENTORY_SLOTS]
}

pub fn slot_block(slots: &InventorySlots, slot_index: usize) -> Option<(BlockId, u32)> {
    slots
        .get(slot_index)
        .and_then(|slot| slot.block.map(|block| (block, slot.count)))
        .filter(|(_, count)| *count > 0)
}

pub fn remove_block_from_slot(slots: &mut InventorySlots, slot_index: usize) -> Option<BlockId> {
    let slot = slots.get_mut(slot_index)?;
    if slot.count == 0 {
        slot.block = None;
        return None;
    }

    slot.count = slot.count.saturating_sub(1);
    let block = slot.block;
    if slot.count == 0 {
        slot.block = None;
    }
    block
}

pub fn inventory_unit_count(slots: &InventorySlots) -> u32 {
    slots.iter().map(|slot| slot.count).sum()
}

pub fn add_block_to_slots(slots: &mut InventorySlots, block: BlockId) -> bool {
    if inventory_unit_count(slots) >= MAX_INVENTORY_UNITS {
        return false;
    }

    if let Some(slot) = slots.iter_mut().find(|slot| slot.block == Some(block)) {
        slot.count = slot.count.saturating_add(1);
        return true;
    }

    if let Some(slot) = slots.iter_mut().find(|slot| slot.block.is_none()) {
        slot.block = Some(block);
        slot.count = 1;
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_block_reports_present_item() {
        let mut slots = empty_inventory();
        slots[2] = InventorySlot {
            block: Some(7),
            count: 3,
        };

        assert_eq!(slot_block(&slots, 2), Some((7, 3)));
        assert_eq!(slot_block(&slots, 1), None);
    }

    #[test]
    fn remove_block_from_slot_decrements_and_clears() {
        let mut slots = empty_inventory();
        slots[1] = InventorySlot {
            block: Some(5),
            count: 2,
        };

        assert_eq!(remove_block_from_slot(&mut slots, 1), Some(5));
        assert_eq!(slot_block(&slots, 1), Some((5, 1)));

        assert_eq!(remove_block_from_slot(&mut slots, 1), Some(5));
        assert_eq!(slot_block(&slots, 1), None);
        assert!(slots[1].block.is_none());
        assert_eq!(slots[1].count, 0);
    }
}
