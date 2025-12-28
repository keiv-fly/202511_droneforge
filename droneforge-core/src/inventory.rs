use crate::block::BlockId;

pub const INVENTORY_SLOTS: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventorySlot {
    pub block: Option<BlockId>,
    pub count: u32,
}

pub type InventorySlots = [InventorySlot; INVENTORY_SLOTS];

pub fn empty_inventory() -> InventorySlots {
    [InventorySlot::default(); INVENTORY_SLOTS]
}

pub fn add_block_to_slots(slots: &mut InventorySlots, block: BlockId) -> bool {
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
