#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ffi::{c_char, c_void},
    ptr::addr_of,
};
use mrow_common::mbr::PartitionTable;

global_asm!(include_str!("./boot.s"));

unsafe extern "C" {
    pub static _mbr_start: c_void;

    pub static _partition_table: c_void;
    pub static _stage_2_start: c_void;
}

#[inline(always)]
pub unsafe fn partition_table<'a>() -> &'a PartitionTable {
    unsafe { &*addr_of!(_partition_table).cast::<PartitionTable>() }
}

#[no_mangle]
pub unsafe extern "C" fn _stage_1() -> ! {
    let table = unsafe { partition_table() };
    let stage_2 = &table.entries[0];

    if stage_2.is_bootable() & (stage_2.sector_len() != 0) {
        unsafe { load_stage_2() }
    } else {
        unsafe { no_stage_2() }
    }

    loop {}
}

#[inline(always)]
unsafe fn load_stage_2() {
    unsafe { print(c"cannot load the stage 2 loader yet.".as_ptr()) };
}

#[inline(always)]
unsafe fn no_stage_2() {
    unsafe { print(c"stage 2 loader does not exist.".as_ptr()) };
}

#[inline(never)]
#[no_mangle]
unsafe fn print(ptr: *const c_char) {
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
