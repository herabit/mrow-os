/// Promise the compiler that a condition is always true.
///
/// You must specify an error message that is not empty,
/// otherwise it is a compile time error.
///
/// # Safety
///
/// This macro is ***very unsafe***, as failing to actually ensure that the
/// condition you promise is true results in undefined behavior.
///
/// # Panics
///
/// This macro panics when compiled with debug assertions enabled.
#[macro_export]
macro_rules! promise {
    ($cond:expr, $message:expr $(,)?) => {{
        const {
            ::core::assert!(
                !$message.is_empty(),
                "a promise must have a non empty error message"
            );
        }

        match $cond {
            false if ::core::cfg!(debug_assertions) => {
                #[inline(always)]
                #[cold]
                const unsafe fn __needs_unsafe() {}

                __needs_unsafe();

                ::core::panic!(::core::concat!("unsafe promise(s) violated: ", $message))
            }
            false => ::core::hint::unreachable_unchecked(),
            true => {}
        }
    }};
}

/// Parses a compile time environment variable.
#[macro_export]
macro_rules! var {
    ($name:expr, $ty:ident $(, $error_msg:expr)? $(,)?) => {{
        let digits = ::core::primitive::str::as_bytes(
            ::core::env!($name, $($error_msg)?)
        );

        $crate::__private::var::$ty(digits)
    }};
}

/// Parses a compile time environment variable if it exists.
#[macro_export]
macro_rules! option_var {
    ($name:expr, $ty:ident $(,)?) => {{
        match ::core::option_env!($name) {
            ::core::option::Option::Some(digits) => {
                let digits = ::core::primitive::str::as_bytes(digits);
                let value = $crate::__private::var::$ty(digits);

                ::core::option::Option::Some(value)
            }
            ::core::option::Option::None => ::core::option::Option::None,
        }
    }};
}
