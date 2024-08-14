#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ffi::{c_char, c_void},
    mem::transmute,
    ptr::{addr_of, addr_of_mut},
};
use mrow_common::mbr::{PartitionTable, TableEntry};

global_asm!(include_str!("./boot.s"));

unsafe extern "C" {
    pub static _mbr_start: c_void;

    pub static _partition_table: c_void;
    pub static mut _stage_2_start: c_void;
}

#[inline(always)]
pub unsafe fn partition_table<'a>() -> &'a PartitionTable {
    unsafe { &*addr_of!(_partition_table).cast::<PartitionTable>() }
}

#[no_mangle]
pub unsafe extern "C" fn _stage_1() -> ! {
    let table = unsafe { partition_table() };
    let stage_2 = &table.entries[0];

    if stage_2.is_bootable() & (stage_2.sector_len() != 0)
    // & (stage_2.sector_len() <= (u16::MAX as u32))
    {
        unsafe { load_stage_2(stage_2) }
    }

    unsafe { print(c"Could not find stage 2".as_ptr()) };
    loop {}
}

type Stage2Fn = unsafe extern "C" fn(print_fn: unsafe extern "C" fn(ptr: *const c_char));

unsafe fn load_stage_2(entry: &TableEntry) {
    unsafe { print(c"Loading stage 2...\r\n".as_ptr()) };

    let mut sectors = entry.sector_len() as u16;
    let mut target = addr_of_mut!(_stage_2_start) as *mut u8;
    let mut lba = entry.start_lba() as u64;

    while sectors > 0 {
        let mut dap =
            DiskAddressPacket::from_lba(lba, 1, target as u16, ((target as u32) >> 16) as u16);

        unsafe {
            dap.load(0x0080);
        }

        sectors -= 1;
        target = unsafe { target.add(512) };
        lba += 1;
    }
    let stage_2 = unsafe { transmute::<_, Stage2Fn>(addr_of!(_stage_2_start)) };

    unsafe { stage_2(print) };
}

#[repr(C, packed)]
struct DiskAddressPacket {
    pub size: u8,
    pub zero: u8,
    pub sectors: u16,
    pub target_offset: u16,
    pub target_segment: u16,
    pub start_lba: u64,
}

impl DiskAddressPacket {
    pub fn from_lba(start_lba: u64, sectors: u16, target_offset: u16, target_segment: u16) -> Self {
        Self {
            size: 0x10,
            zero: 0,
            sectors,
            target_offset,
            target_segment,
            start_lba,
        }
    }
    pub unsafe fn load(&mut self, disk: u16) {
        let dap_addr = (self as *mut _) as u16;
        unsafe {
            asm!(
                "mov {1:x}, si",
                "mov si, {0:x}",
                "int 0x13",
                "jc fail",
                "mov si, {1:x}",
                in(reg) dap_addr,
                out(reg) _,
                in("ax") 0x4200_u16,
                in("dx") disk,
            );
        }
    }
}

#[no_mangle]
unsafe extern "C" fn fail() -> ! {
    unsafe { print(c"Failed to load stage 2 loader\r\n".as_ptr()) };
    loop {}
}

#[no_mangle]
unsafe extern "C" fn print(ptr: *const c_char) {
    unsafe {
        asm!(
            "mov si, {0:x}",
            "2:",
            "lodsb",
            "or al, al",
            "jz 3f",

            "mov ah, 0x0e",
            "mov bh, 0",
            "int 0x10",
            "jmp 2b",

            "3:",
            in(reg) ptr,
        );
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
