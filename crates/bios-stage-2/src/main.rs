#![no_std]
#![no_main]

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    pub static _mbr_start: c_void;
    pub static _stage_2_end: c_void;
}

#[no_mangle]
#[link_section = ".start"]
pub unsafe extern "C" fn _start(print_fn: unsafe extern "C" fn(ptr: *const c_char)) -> ! {
    unsafe { print_fn(c"Hello from stage 2!\r\n".as_ptr()) };
    loop {}
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
