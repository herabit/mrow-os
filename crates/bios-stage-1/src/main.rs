#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    ffi::CStr,
};

use mrow_common::option_var;

global_asm!(include_str!("./boot.s"));

pub const STAGE_2_SIZE: u16 = match option_var!("MROW_STAGE_2_SIZE", u16) {
    Some(size) => size,
    None => 0,
};

#[no_mangle]
pub unsafe extern "C" fn _stage_1() -> ! {
    if STAGE_2_SIZE != 0 {
        unsafe { load_stage_2() };
    } else {
        unsafe { no_stage_2() };
    }

    loop {}
}

#[inline(always)]
unsafe fn load_stage_2() {
    unsafe { print(c"cannot load the stage 2 loader yet.") };
}

#[inline(always)]
unsafe fn no_stage_2() {
    unsafe { print(c"stage 2 loader does not exist.") };
}

#[inline(never)]
#[no_mangle]
unsafe fn print(msg: &CStr) {
    let mut ptr = msg.as_ptr();

    loop {
        let ch = unsafe { ptr.read() };

        if ch == 0 {
            break;
        }

        let ax = (ch as u16) | 0x0e00;

        unsafe {
            asm!("xor bx, bx", "int 0x10", in("ax") ax);
            // asm!("push bx", "mov bx, 0", "int 0x10", "pop bx", in("ax") ax);
        }

        ptr = unsafe { ptr.add(1) };
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
