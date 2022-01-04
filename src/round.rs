pub trait Round: Copy {}

macro_rules! impl_round {
    ($t:ty) => {
        impl Round for $t {}
        impl Round for ($t, $t) {}
        impl Round for ($t, $t, $t) {}
        impl Round for ($t, $t, $t, $t) {}
    };
}

impl_round!(usize);
impl_round!(u8);
impl_round!(u16);
impl_round!(u32);
impl_round!(u64);
impl_round!(u128);
impl_round!(i8);
impl_round!(i16);
impl_round!(i32);
impl_round!(i64);
impl_round!(i128);
