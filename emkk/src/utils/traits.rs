use core::ffi::c_void;

pub struct BasicRegion<LenT: LengthLimit, BaseT: BaseLimit> {
    base: BaseT,
    length: LenT,
}

impl<LenT: LengthLimit, BaseT: BaseLimit> Region<LenT, BaseT> for BasicRegion<LenT, BaseT> {
    fn new(length: LenT, base: BaseT) -> Self {
        return Self { base, length };
    }

    fn get_base(&self) -> BaseT {
        return self.base.clone();
    }
    fn get_length(&self) -> LenT {
        return self.length.clone();
    }
    fn end(&self) -> u64 {
        return self.base.clone().as_u64() + self.length.clone().as_u64();
    }

    fn within(&self, other: &Self) -> bool {
        return other.base >= self.base && other.end() <= self.end();
    }

    //Disabled.
    fn offset(&self, _offset: LenT) -> Option<BasicRegion<LenT, BaseT>> {
        return Option::None;
    }
}

pub trait AsU64 {
    fn as_u64(&self) -> u64;
}

impl<T: ?Sized> AsU64 for *const T {
    fn as_u64(&self) -> u64 {
        return (*self as *const c_void) as u64;
    }
}

impl<T: ?Sized> AsU64 for *mut T {
    fn as_u64(&self) -> u64 {
        return (*self as *const c_void) as u64;
    }
}

impl AsU64 for u8 {
    fn as_u64(&self) -> u64 {
        return *self as u64;
    }
}

impl AsU64 for u16 {
    fn as_u64(&self) -> u64 {
        return *self as u64;
    }
}

impl AsU64 for u32 {
    fn as_u64(&self) -> u64 {
        return *self as u64;
    }
}

impl AsU64 for u64 {
    fn as_u64(&self) -> u64 {
        return *self;
    }
}

impl AsU64 for usize {
    fn as_u64(&self) -> u64 {
        return *self as u64;
    }
}

impl AsU64 for isize {
    fn as_u64(&self) -> u64 {
        return *self as u64;
    }
}

pub trait LengthLimit: Clone + PartialOrd + AsU64 {}
pub trait BaseLimit: Clone + PartialOrd + AsU64 {}

impl LengthLimit for u8 {}
impl LengthLimit for u16 {}
impl LengthLimit for u32 {}
impl LengthLimit for u64 {}
impl LengthLimit for usize {}
impl LengthLimit for isize {}

impl BaseLimit for u8 {}
impl BaseLimit for u16 {}
impl BaseLimit for u32 {}
impl BaseLimit for u64 {}
impl BaseLimit for usize {}
impl BaseLimit for isize {}

impl<T: ?Sized> LengthLimit for *const T {}
impl<T: ?Sized> BaseLimit for *const T {}

impl<T: ?Sized> LengthLimit for *mut T {}
impl<T: ?Sized> BaseLimit for *mut T {}

pub trait Region<LenT: LengthLimit, BaseT: BaseLimit> {
    fn from_region(other: &impl Region<LenT, BaseT>) -> Self
    where
        Self: Sized,
    {
        return Self::new(other.get_length(), other.get_base());
    }

    fn new(length: LenT, base: BaseT) -> Self;

    fn get_length(&self) -> LenT;
    fn get_base(&self) -> BaseT;

    fn end(&self) -> u64;
    fn within(&self, other: &Self) -> bool;

    fn offset(&self, offset: LenT) -> Option<BasicRegion<LenT, BaseT>>;
}
