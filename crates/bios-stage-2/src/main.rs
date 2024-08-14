#![no_std]
#![no_main]

use core::ffi::c_char;

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
