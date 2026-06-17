use core::{
    arch::asm,
    ffi::c_void,
    fmt::{Arguments, Write},
    ptr::{null, null_mut},
    slice,
};

use crate::{
    fixed_vaddrs::FRAMEBUFFER_FIXED_VADDR,
    hal::memory::{
        allocator::Allocator,
        pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager},
    },
    utils::Errno,
};

#[repr(C)]
pub struct GopFramebuffer {
    base: u64,
    width: u32,
    height: u32,
    pixel_per_scaneline: u32,
    framebuffer_size: u32,
}

impl GopFramebuffer {
    pub fn get_base(&self) -> u64 {
        return self.base;
    }
    pub fn get_size(&self) -> u32 {
        return self.framebuffer_size;
    }
}

pub fn switch_framebuffer_location(pager: &mut Pager, physical_allocator: &mut Allocator) {
    let mut fb = unsafe { &mut (*SCREEN_INFO.framebuffer) };
    let mut pages = fb.framebuffer_size / 0x1000;
    if fb.framebuffer_size % 0x1000 != 0 {
        pages += 1;
    }

    for i in 0..pages as u64 {
        match pager.page_4_kb(
            FRAMEBUFFER_FIXED_VADDR + i * 0x1000,
            fb.base + i * 0x1000,
            PAGER_PCD | PAGER_RW | PAGER_PRESENT,
            physical_allocator,
        ) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic(
                "switch_framebuffer_location",
                "Could not switch framebuffer location\n",
            ),
        }
    }
    fb.base = FRAMEBUFFER_FIXED_VADDR;
    for i in 0..pages as u64 {
        match pager.unpage_4k(unsafe { FRAMEBUFFER_ORIGINAL_BASE } + i * 0x1000) {
            Some(e) => simple_kernel_panic(
                "switch_framebuffer_location",
                "Could not unpage framebuffer\n",
            ),
            None => {}
        }
    }
}

pub fn page_framebuffer_virtual(pager: &mut Pager, allocator: &mut Allocator) {
    let mut fb = unsafe { &mut (*SCREEN_INFO.framebuffer) };
    let mut pages = fb.framebuffer_size / 0x1000;
    if fb.framebuffer_size % 0x1000 != 0 {
        pages += 1;
    }
    for i in 0..pages as u64 {
        match pager.page_4_kb(
            FRAMEBUFFER_FIXED_VADDR + i * 0x1000,
            unsafe { FRAMEBUFFER_ORIGINAL_BASE } + i * 0x1000,
            PAGER_PCD | PAGER_RW | PAGER_PRESENT,
            allocator,
        ) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic("page_framebuffer_virtual", "Could not page framebuffer")
            }
        }
    }
}

static mut FRAMEBUFFER_ORIGINAL_BASE: u64 = 0;
struct ScreenInfo {
    chars_per_row: u32,
    chars_per_screen: u32,
    x_location: u32,
    y_location: u32,
    framebuffer: *mut GopFramebuffer,
}

fn get_pixel_location() -> *mut u32 {
    let mut base = unsafe { (*SCREEN_INFO.framebuffer).base };
    base += 32u64 * unsafe { SCREEN_INFO.x_location } as u64;
    base += 52u64
        * unsafe { (*SCREEN_INFO.framebuffer).width as u64 }
        * unsafe { SCREEN_INFO.y_location as u64 };
    return base as *mut u32;
}

#[unsafe(no_mangle)]
fn gopcharput(c: char, color_code: u32) {
    if c == '\n' {
        unsafe {
            SCREEN_INFO.x_location = 0;
            SCREEN_INFO.y_location += 1;
        }
    } else {
        let mut char_data: *const u8 = unsafe { (FONT as *const u8).add(16 * c as usize) };
        let mut pixel_data: *mut u32 = get_pixel_location();

        for _ in 0..16 {
            let data = unsafe { *char_data };
            for w in 0..8 {
                if (data & (1 << w)) != 0 {
                    unsafe { pixel_data.add(8 - w).write(color_code) };
                }
            }
            pixel_data = unsafe { pixel_data.add((*SCREEN_INFO.framebuffer).width as usize) };
            char_data = unsafe { char_data.add(1) };
        }
    }
    return;
}

static STDCHARPUT: fn(char, u32) = gopcharput;

#[unsafe(no_mangle)]
fn gopstrput(chars: &str, line: u16, length: u32, color_code: u32) {
    unsafe { SCREEN_INFO.y_location = line as u32 };
    for i in 0..length {
        let c = chars.as_bytes()[i as usize] as char;
        (STDCHARPUT)(c, color_code);
        if c != '\n' {
            unsafe { SCREEN_INFO.x_location += 1 };
        }
        if (unsafe { SCREEN_INFO.x_location } > unsafe { SCREEN_INFO.chars_per_row }) {
            unsafe { SCREEN_INFO.x_location = 0 };
            unsafe { SCREEN_INFO.y_location += 1 };
        }
    }
    return;
}

static STDSTRPUT: fn(&str, u16, u32, u32) = gopstrput;
static mut SCREEN_INFO: ScreenInfo = ScreenInfo {
    chars_per_row: 0,
    chars_per_screen: 0,
    x_location: 0,
    y_location: 0,
    framebuffer: null_mut(),
};
static mut FONT: *const c_void = null();
static mut PRINT_BUFFER: *const c_void = null();

pub fn get_framebuffer() -> *const GopFramebuffer {
    return unsafe { SCREEN_INFO.framebuffer };
}

#[allow(unsafe_op_in_unsafe_fn)]
#[unsafe(no_mangle)]
pub unsafe fn print_init(
    tmp_buffer_address: *mut c_void,
    framebuffer: *mut GopFramebuffer,
    font: *const c_void,
) -> Errno {
    SCREEN_INFO.chars_per_row = (*framebuffer).width / 8;
    SCREEN_INFO.chars_per_screen = SCREEN_INFO.chars_per_row * ((*framebuffer).height / 16);
    SCREEN_INFO.framebuffer = framebuffer;
    PRINT_BUFFER = tmp_buffer_address;
    FRAMEBUFFER_ORIGINAL_BASE = (*framebuffer).base;
    FONT = font;
    return Errno::EOK;
}

pub fn clear_line(line: u16) {
    let framebuffer = unsafe { &*SCREEN_INFO.framebuffer };

    let base = framebuffer.base + framebuffer.pixel_per_scaneline as u64 * line as u64 * 4 * 16;
    let buffer =
        unsafe { slice::from_raw_parts_mut(base as *mut u32, framebuffer.width as usize * 16) };
    for pixel in buffer {
        *pixel = 0;
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn clear_screen() {
    let framebuffer = &*SCREEN_INFO.framebuffer;
    let buffer: &mut [u32] = slice::from_raw_parts_mut(
        framebuffer.base as *mut u32,
        (framebuffer.height * framebuffer.width) as usize,
    );
    for pixel in buffer {
        *pixel = 0;
    }
    SCREEN_INFO.x_location = 0;
    SCREEN_INFO.y_location = 0;
}

pub enum StringAlignment {
    Center,
    Left,
    Right,
}

pub fn show_string_direct(
    str: &str,
    line: u16,
    color_code: u32,
    string_alignment: StringAlignment,
) {
    let prev_x = unsafe { SCREEN_INFO.x_location };
    let prev_y = unsafe { SCREEN_INFO.y_location };

    match string_alignment {
        StringAlignment::Right => unsafe {
            SCREEN_INFO.x_location = SCREEN_INFO.chars_per_row - str.len() as u32
        },
        StringAlignment::Left => unsafe {
            SCREEN_INFO.x_location = 0;
        },
        StringAlignment::Center => unsafe {
            let middle = SCREEN_INFO.chars_per_row / 2;
            SCREEN_INFO.x_location = middle - (str.len() as u32) / 2;
        },
    }

    (STDSTRPUT)(str, line, str.len() as u32, color_code);
    unsafe {
        SCREEN_INFO.y_location = prev_y;
        SCREEN_INFO.x_location = prev_x;
    }
}

pub fn show_string_direct_ex(str: &str, line: u16, x_char: u16, color_code: u32) {
    let prev_x = unsafe { SCREEN_INFO.x_location };
    let prev_y = unsafe { SCREEN_INFO.y_location };

    unsafe {
        SCREEN_INFO.x_location = x_char as u32;
    }

    (STDSTRPUT)(str, line, str.len() as u32, color_code);
    unsafe {
        SCREEN_INFO.y_location = prev_y;
        SCREEN_INFO.x_location = prev_x;
    }
}

pub fn middle_char_pos() -> u32 {
    unsafe { SCREEN_INFO.chars_per_row / 2 }
}

pub enum ModuleWriteMode {
    Neutral,
    Success,
    Info,
    Warn,
    Error,
}

pub struct Module<'a> {
    pub(super) name: &'a str,
    pub(super) deconstructed: bool,
    pub(super) write_mode: ModuleWriteMode,
}

#[macro_export]
macro_rules! success {
    ($dst:expr, $($arg:tt)*) => {
        $dst.success_impl(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($dst:expr, $($arg:tt)*) => {
        $dst.info_impl(format_args!($($arg)*))
    };
}
#[macro_export]
macro_rules! warn {
    ($dst:expr, $($arg:tt)*) => {
        $dst.warning_impl(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($dst:expr, $($arg:tt)*) => {
        $dst.error_impl(format_args!($($arg)*))
    };
}

impl<'a> Write for Module<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self.write_mode {
            ModuleWriteMode::Success => (STDSTRPUT)(
                s,
                unsafe { SCREEN_INFO.y_location as u16 },
                s.len() as u32,
                0x00FF00,
            ),
            ModuleWriteMode::Error => (STDSTRPUT)(
                s,
                unsafe { SCREEN_INFO.y_location as u16 },
                s.len() as u32,
                0xFF0000,
            ),
            ModuleWriteMode::Neutral => (STDSTRPUT)(
                s,
                unsafe { SCREEN_INFO.y_location as u16 },
                s.len() as u32,
                0x000000,
            ),
            ModuleWriteMode::Warn => (STDSTRPUT)(
                s,
                unsafe { SCREEN_INFO.y_location as u16 },
                s.len() as u32,
                0xFFFF00,
            ),
            ModuleWriteMode::Info => (STDSTRPUT)(
                s,
                unsafe { SCREEN_INFO.y_location as u16 },
                s.len() as u32,
                0x0000FF,
            ),
        };
        return Ok(());
    }
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        match self.write_mode {
            ModuleWriteMode::Success => (STDCHARPUT)(c, 0x00FF00),
            ModuleWriteMode::Error => (STDCHARPUT)(c, 0xFF0000),
            ModuleWriteMode::Neutral => (STDCHARPUT)(c, 0x000000),
            ModuleWriteMode::Warn => (STDCHARPUT)(c, 0xFFFF00),
            ModuleWriteMode::Info => (STDCHARPUT)(c, 0x0000FF),
        };
        return Ok(());
    }
}

const INVALID_NAME: &'static str = "INVALID MODULE";

impl<'a> Module<'a> {
    pub const fn empty() -> Self {
        return Self {
            name: INVALID_NAME,
            deconstructed: true,
            write_mode: ModuleWriteMode::Neutral,
        };
    }
}

impl<'a> Default for Module<'a> {
    fn default() -> Self {
        return Self {
            name: INVALID_NAME,
            deconstructed: true,
            write_mode: ModuleWriteMode::Neutral,
        };
    }
}

impl<'a> Module<'a> {
    pub fn new(name: &'a str) -> Module<'a> {
        return Module {
            name,
            deconstructed: false,
            write_mode: ModuleWriteMode::Neutral,
        };
    }

    pub fn name(&self) -> &'a str {
        return self.name;
    }

    pub fn destory(&mut self) {
        self.deconstructed = true;
    }

    pub fn info_impl(&mut self, args: Arguments) {
        self.write_mode = ModuleWriteMode::Info;
        (STDCHARPUT)('[', 0x0000FF);
        unsafe { SCREEN_INFO.x_location += 1 };
        (STDSTRPUT)(
            self.name,
            unsafe { SCREEN_INFO.y_location as u16 },
            self.name.len() as u32,
            0x0000FF,
        );
        (STDCHARPUT)(']', 0x0000FF);
        unsafe { SCREEN_INFO.x_location += 2 };
        let _ = self.write_fmt(args);
    }

    pub fn debug(&mut self, args: Arguments) {
        self.write_mode = ModuleWriteMode::Neutral;
        (STDCHARPUT)('[', 0x000000);
        unsafe { SCREEN_INFO.x_location += 1 };
        (STDSTRPUT)(
            self.name,
            unsafe { SCREEN_INFO.y_location as u16 },
            self.name.len() as u32,
            0x000000,
        );
        (STDCHARPUT)(']', 0x000000);
        unsafe { SCREEN_INFO.x_location += 2 };
        let _ = self.write_fmt(args);
    }

    pub fn warning_impl(&mut self, args: Arguments) {
        self.write_mode = ModuleWriteMode::Warn;
        (STDCHARPUT)('[', 0xFFFF00);
        unsafe { SCREEN_INFO.x_location += 1 };
        (STDSTRPUT)(
            self.name,
            unsafe { SCREEN_INFO.y_location as u16 },
            self.name.len() as u32,
            0xFFFF00,
        );
        (STDCHARPUT)(']', 0xFFFF00);
        (STDCHARPUT)(' ', 0xFFFF00);
        unsafe { SCREEN_INFO.x_location += 2 };
        let _ = self.write_fmt(args);
    }

    pub fn success_impl(&mut self, args: Arguments) {
        self.write_mode = ModuleWriteMode::Success;
        (STDCHARPUT)('[', 0x00FF00);
        unsafe { SCREEN_INFO.x_location += 1 };
        (STDSTRPUT)(
            self.name,
            unsafe { SCREEN_INFO.y_location as u16 },
            self.name.len() as u32,
            0x00FF00,
        );
        (STDCHARPUT)(']', 0x00FF00);
        unsafe { SCREEN_INFO.x_location += 2 };

        let _ = self.write_fmt(args);
    }

    pub fn error_impl(&mut self, args: Arguments) {
        self.write_mode = ModuleWriteMode::Error;
        (STDCHARPUT)('[', 0xFF0000);
        unsafe { SCREEN_INFO.x_location += 1 };
        (STDSTRPUT)(
            self.name,
            unsafe { SCREEN_INFO.y_location as u16 },
            self.name.len() as u32,
            0xFF0000,
        );
        (STDCHARPUT)(']', 0xFF0000);
        unsafe { SCREEN_INFO.x_location += 2 };
        let _ = self.write_fmt(args);
    }
}

pub fn simple_kernel_panic(module: &'static str, error: &'static str) -> ! {
    let mut kp_module = Module::new(module);
    error!(&mut kp_module, "{}", error);
    unsafe { asm!("cli;hlt") };
    loop {}
}
