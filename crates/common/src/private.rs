pub mod var {
    macro_rules! parse_int {
        ($($ty:ident),* $(,)?) => {

            $(
                #[inline(always)]
                #[track_caller]
                pub const fn $ty(digits: &[u8]) -> ::core::primitive::$ty {
                    #[inline(always)]
                    #[track_caller]
                    const fn run_loop<const IS_POSITIVE: bool>(mut digits: &[u8]) -> ::core::primitive::$ty {
                        let mut result = 0 as ::core::primitive::$ty;

                        while let [c, rest @ ..] = digits {
                            if !c.is_ascii_digit() {
                                panic!("invalid digit");
                            }

                            let x = (*c - b'0') as ::core::primitive::$ty;

                            let res = match result.checked_mul(10) {
                                Some(mul) if IS_POSITIVE => mul.checked_add(x),
                                Some(mul) => mul.checked_sub(x),
                                None => None,
                            };

                            result = match res {
                                Some(result) => result,
                                _ if IS_POSITIVE => panic!("positive overflow occurred"),
                                _ => panic!("negative overflow occurred"),
                            };

                            digits = rest;
                        }

                        result
                    }

                    let (is_positive, digits) = match digits {
                        [b'-' | b'+'] => panic!("invalid digit"),
                        [b'+', rest @ ..] => (true, rest),
                        #[allow(unused_comparisons)]
                        [b'-', rest @ ..] if ::core::primitive::$ty::MIN < 0 => (false, rest),
                        [b'-', ..] => panic!("unsigned integers cannot be negative"),
                        _ => (true, digits),
                    };

                    if is_positive {
                        run_loop::<true>(digits)
                    } else {
                        run_loop::<false>(digits)
                    }
                }
            )*
        };
    }

    parse_int!(u8, u16, u32, u64, u128, usize);
    parse_int!(i8, i16, i32, i64, i128, isize);
}
