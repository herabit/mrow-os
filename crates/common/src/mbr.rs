use core::ptr::{addr_of, addr_of_mut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C, packed)]
pub struct TableEntry {
    /// Flags associated with the partition.
    pub flags: u8,
    /// Start CHS address of the partition.
    pub start_chs: [u8; 3],
    /// What kind of partition this is.
    pub partition_kind: u8,
    /// End CHS address of the partition.
    pub end_chs: [u8; 3],
    /// Logical block address of the partition.
    ///
    /// # Unaligned Accesses
    ///
    /// See [`TableEntry::start_lba`] and [`TableEntry::set_start_lba`].
    pub start_lba: u32,
    /// Size of the partition in sectors.
    ///
    /// # Unaligned Accesses
    ///
    /// See [`TableEntry::sector_len`] and [`TableEntry::set_sectors_len`].
    pub sector_len: u32,
}

impl TableEntry {
    /// Returns whether this entry is bootable.
    #[inline]
    #[must_use]
    pub const fn is_bootable(&self) -> bool {
        self.flags & 0x80 != 0
    }

    /// Reads the logical block address of the entry.
    #[inline]
    #[must_use]
    pub const fn start_lba(&self) -> u32 {
        unsafe { addr_of!(self.start_lba).read_unaligned() }
    }

    /// Reads the length in sectors of the entry.
    #[inline]
    #[must_use]
    pub const fn sector_len(&self) -> u32 {
        unsafe { addr_of!(self.sector_len).read_unaligned() }
    }

    /// Sets a new value to the logical block address of the entry.
    #[inline]
    #[must_use]
    pub fn set_start_lba(&mut self, lba: u32) {
        unsafe { addr_of_mut!(self.start_lba).write_unaligned(lba) }
    }

    /// Sets a new value to the sector length of the entry.
    #[inline]
    #[must_use]
    pub fn set_sector_len(&mut self, sector_len: u32) {
        unsafe { addr_of_mut!(self.sector_len).write_unaligned(sector_len) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C, packed)]
pub struct PartitionTable {
    pub entries: [TableEntry; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C, packed)]
pub struct MasterBootRecord {
    pub bootstrap: [u8; 440],
    pub unique_id: u32,
    pub reserved: u16,
    pub partition_table: PartitionTable,
    pub signature: u16,
}

impl MasterBootRecord {
    #[inline]
    #[must_use]
    pub const fn unique_id(&self) -> u32 {
        unsafe { addr_of!(self.unique_id).read_unaligned() }
    }

    #[inline]
    #[must_use]
    pub fn set_unique_id(&mut self, unique_id: u32) {
        unsafe { addr_of_mut!(self.unique_id).write_unaligned(unique_id) }
    }

    #[inline]
    #[must_use]
    pub const fn reserved(&self) -> u16 {
        unsafe { addr_of!(self.reserved).read_unaligned() }
    }

    #[inline]
    #[must_use]
    pub fn set_reserved(&mut self, reserved: u16) {
        unsafe { addr_of_mut!(self.reserved).write_unaligned(reserved) }
    }

    #[inline]
    #[must_use]
    pub const fn signature(&self) -> u16 {
        unsafe { addr_of!(self.signature).read_unaligned() }
    }

    #[inline]
    #[must_use]
    pub fn set_signature(&mut self, signature: u16) {
        unsafe { addr_of_mut!(self.signature).write_unaligned(signature) }
    }
}

impl Default for MasterBootRecord {
    fn default() -> Self {
        Self {
            bootstrap: [0; 440],
            unique_id: 0,
            reserved: 0,
            partition_table: PartitionTable::default(),
            signature: 0,
        }
    }
}
