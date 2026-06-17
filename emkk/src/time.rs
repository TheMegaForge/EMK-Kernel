unsafe extern "C" {
    fn sleep0(ms: u32);
    fn current_tick0() -> u64;
}

#[inline(always)]
pub fn sleep(ms: u32) {
    unsafe { sleep0(ms) }
}

#[inline(always)]
pub fn current_tick() -> u64 {
    unsafe { current_tick0() }
}

pub const MS_PER_TICK: u64 = 10;
#[inline(always)]
pub fn tick_in_ms() -> u64 {
    return unsafe { current_tick0() } * MS_PER_TICK;
}
