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
