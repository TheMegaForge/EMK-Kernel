use core::{
    ffi::{c_uchar, c_void},
    ptr::null_mut,
};

use crate::{
    drivers::usb::{
        independent::{BOOT_PROTOCOL, USB_MICRO_FRAME_TO_FRAME_CONVERSION_FACTOR},
        standard_requests::UsbHID,
        traits::{UsbController, UsbDevice},
    },
    hal::{
        memory::allocator::Allocator,
        print::{Module, simple_kernel_panic},
    },
    info,
    time::{self},
    utils::memory::{memcpy, memset},
};

pub const CONTROL_CHARACTER_LEFT_CTRL: u8 = 0;
pub const CONTROL_CHARACTER_LEFT_SHIFT: u8 = 1;
pub const CONTROL_CHARACTER_LEFT_ALT: u8 = 2;
pub const CONTROL_CHARACTER_LEFT_GUI: u8 = 3;
pub const CONTROL_CHARACTER_RIGHT_CTRL: u8 = 4;
pub const CONTROL_CHARACTER_RIGHT_SHIFT: u8 = 5;
pub const CONTROL_CHARACTER_RIGHT_ALT: u8 = 6;
pub const CONTROL_CHARACTER_RIGHT_GUI: u8 = 7;

pub const CONTROL_CHARACTER_CAPSLOCK: u8 = 8;
pub const CONTROL_CHARACTER_F1: u8 = 9;
pub const CONTROL_CHARACTER_F2: u8 = 10;
pub const CONTROL_CHARACTER_F3: u8 = 11;
pub const CONTROL_CHARACTER_F4: u8 = 12;
pub const CONTROL_CHARACTER_F5: u8 = 13;
pub const CONTROL_CHARACTER_F6: u8 = 14;
pub const CONTROL_CHARACTER_F7: u8 = 15;
pub const CONTROL_CHARACTER_F8: u8 = 16;
pub const CONTROL_CHARACTER_F9: u8 = 17;
pub const CONTROL_CHARACTER_F10: u8 = 18;
pub const CONTROL_CHARACTER_F11: u8 = 19;
pub const CONTROL_CHARACTER_F12: u8 = 20;
pub const CONTROL_CHARACTER_PRINT_SCREEN: u8 = 21;
pub const CONTROL_CHARACTER_SCROLL_LOCK: u8 = 22;
pub const CONTROL_CHARACTER_PAUSE: u8 = 23;
pub const CONTROL_CHARACTER_INSERT: u8 = 24;
pub const CONTROL_CHARACTER_HOME: u8 = 25;
pub const CONTROL_CHARACTER_PAGE_UP: u8 = 26;
pub const CONTROL_CHARACTER_DELETE_FORWARD: u8 = 27;
pub const CONTROL_CHARACTER_END: u8 = 28;
pub const CONTROL_CHARACTER_PAGE_DOWN: u8 = 29;
pub const CONTROL_CHARACTER_RIGHT_ARROW: u8 = 30;
pub const CONTROL_CHARACTER_LEFT_ARROW: u8 = 31;
pub const CONTROL_CHARACTER_DOWN_ARROW: u8 = 32;
pub const CONTROL_CHARACTER_UP_ARROW: u8 = 33;
pub const CONTROL_CHARACTER_NUM_LOCK: u8 = 34;
pub const CONTROL_CHARACTER_F13: u8 = 35;
pub const CONTROL_CHARACTER_F14: u8 = 36;
pub const CONTROL_CHARACTER_F15: u8 = 37;
pub const CONTROL_CHARACTER_F16: u8 = 38;
pub const CONTROL_CHARACTER_F17: u8 = 39;
pub const CONTROL_CHARACTER_F18: u8 = 40;
pub const CONTROL_CHARACTER_F19: u8 = 41;
pub const CONTROL_CHARACTER_F20: u8 = 42;
pub const CONTROL_CHARACTER_F21: u8 = 43;
pub const CONTROL_CHARACTER_F22: u8 = 44;
pub const CONTROL_CHARACTER_F23: u8 = 45;
pub const CONTROL_CHARACTER_F24: u8 = 46;

/*
 * Keys when UPPERCASE=true
 */
pub static US_USB_KEY_UPPERCASE_TRANSLATION: [u8; 116] = [
    0xFF,       // Reserved
    0xFF,       // ErrorRollOver
    0xFF,       // POSTFail
    0xFF,       // ErrorUndefined
    0x41,       // A
    0x42,       // B
    0x43,       // C
    0x44,       // D
    0x45,       // E
    0x46,       // F
    0x47,       // G
    0x48,       // H
    0x49,       // I
    0x4A,       // J
    0x4B,       // K
    0x4C,       // L
    0x4D,       // M
    0x4E,       // N
    0x4F,       // O
    0x50,       // P
    0x51,       // Q
    0x52,       // R
    0x53,       // S
    0x54,       // T
    0x55,       // U
    0x56,       // V
    0x57,       // W
    0x58,       // X
    0x59,       // Y
    0x5A,       // Z
    0x21,       // !
    0x40,       // @
    0x23,       // #
    0x24,       // $
    0x25,       // %
    0x5E,       // ^
    0x26,       // &
    0x2A,       // *
    0x28,       // (
    0x29,       // )
    '\r' as u8, // Return (Enter)
    0xFF,       // Escape
    0xFF,       // Backspace
    '\t' as u8, // Tab
    0x20,       // Spacebar
    0x5F,       // _
    0x2B,       // +
    0x7B,       // {
    0x7D,       // }
    0x7C,       // |
    0xFF,       // Non-US
    0x3A,       // :
    0x22,       // "
    0x7E,       // ~ (Tilde)
    0x3C,       // <
    0x3D,       // >
    0x3F,       // ?
    0xFF,       // Capslock
    0xFF,       // F1
    0xFF,       // F2
    0xFF,       // F3
    0xFF,       // F4
    0xFF,       // F5
    0xFF,       // F6
    0xFF,       // F7
    0xFF,       // F8
    0xFF,       // F9
    0xFF,       // F10
    0xFF,       // F11
    0xFF,       // F12
    0xFF,       // PrintScreen
    0xFF,       // ScrollLock
    0xFF,       // Pause
    0xFF,       // Insert
    0xFF,       // Home
    0xFF,       // PageUp
    0xFF,       // Delete Forward
    0xFF,       // End
    0xFF,       // PageDown
    0xFF,       // RightArrow
    0xFF,       // LeftArrow
    0xFF,       // DownArrow
    0xFF,       // UpArrow
    0xFF,       // Num Lock
    0x2F,       // /
    0x2A,       // *
    0x2D,       // -
    0x2B,       // +
    0xFF,       // Enter
    0xFF,       // Keypad End
    0xFF,       // Keypad DownArrow
    0xFF,       // Keypad PageDown
    0xFF,       // Keypad LeftArrow
    0x35,       // Keypad 5
    0xFF,       // Keypad Right Arrow
    0xFF,       // Keypad Home
    0xFF,       // Keypad Up Arrow
    0xFF,       // Keypad PageUp
    0xFF,       // Keypad Insert
    0xFF,       // Keypad Delete
    0xFF,       // Non-US
    0xFF,       // Application
    0xFF,       // Power
    0x3D,       // Keypad =
    0xFF,       // F13
    0xFF,       // F14
    0xFF,       // F15
    0xFF,       // F16
    0xFF,       // F17
    0xFF,       // F18
    0xFF,       // F19
    0xFF,       // F20
    0xFF,       // F21
    0xFF,       // F22
    0xFF,       // F23
    0xFF,       // F24
];

pub const ASCII_TRANSLATION_TABLE_UPPERCASE_NEEDED: u8 = 0b1000_0000;

/*
 * Translates Ascii Keys into keycodes, which can index into PRESSED_KEYS
 * 2 means that the key is always unpressed
 * 0x80| <x> means that x has to be checked and UPPERCASE must be true
 */
static US_USB_ASCII_TRANSLATION_TABLE: [u8; 255] = [
    2,           // Null
    2,           // SOH
    2,           // SOT
    2,           // EOT
    2,           // EOTM
    2,           // Enquiry
    2,           // Acknowledge
    2,           // Audible bell
    2,           // Backspace
    0x2B,        // Horizontal Tab
    2,           // Line feed
    2,           // Vertical Tab
    2,           // Form feed
    2,           // Carriage return
    2,           // Shift out
    2,           // Shift in
    2,           // Data link escape
    2,           // Dev-Ctrl 1
    2,           // Dev-Ctrl 2
    2,           // Dev-Ctrl 3
    2,           // Dev-Ctrl 4
    2,           // Neg. ack
    2,           // Sync Idle
    2,           // End trans. block
    2,           // Cancle
    2,           // End of medium
    2,           // Substitution
    2,           // Escape
    2,           // File seperator
    2,           // Group seperator
    2,           // Record seperator
    2,           // Unit seperator
    0x2C,        // Space
    0x80 | 0x1E, // !
    0x80 | 0x33, // "
    0x80 | 0x20, // #
    0x80 | 0x21, // $
    0x80 | 0x22, // %
    0x80 | 0x24, // &
    0x34,        // '
    0x80 | 0x26, // (
    0x80 | 0x27, // )
    0x80 | 0x25, // *
    0x80 | 0x2E, // +
    0x36,        // ,
    0x2D,        // -
    0x37,        // .
    0x38,        // /
    0x27,        // 0
    0x1E,        // 1
    0x1F,        // 2
    0x20,        // 3
    0x21,        // 4
    0x22,        // 5
    0x23,        // 6
    0x24,        // 7
    0x25,        // 8
    0x26,        // 9
    0x80 | 0x33, // :
    0x33,        // ;
    0x80 | 0x36, // <
    0x2E,        // =
    0x80 | 0x37, // >
    0x80 | 0x38, // ?
    0x80 | 0x1F, // @
    0x80 | 0x04, // A
    0x80 | 0x05, // B
    0x80 | 0x06, // C
    0x80 | 0x07, // D
    0x80 | 0x08, // E
    0x80 | 0x09, // F
    0x80 | 0x0A, // G
    0x80 | 0x0B, // H
    0x80 | 0x0C, // I
    0x80 | 0x0D, // J
    0x80 | 0x0E, // K
    0x80 | 0x0F, // L
    0x80 | 0x10, // M
    0x80 | 0x11, // N
    0x80 | 0x12, // O
    0x80 | 0x13, // P
    0x80 | 0x14, // Q
    0x80 | 0x15, // R
    0x80 | 0x16, // S
    0x80 | 0x17, // T
    0x80 | 0x18, // U
    0x80 | 0x19, // V
    0x80 | 0x1A, // W
    0x80 | 0x1B, // X
    0x80 | 0x1C, // Y
    0x80 | 0x1D, // Z
    0x2F,        // [
    0x31,        // \
    0x30,        // ]
    0x80 | 0x23, // ^
    0x80 | 0x2D, // _
    0x35,        // ´ (Grave Accent)
    0x4,         // a
    0x5,         // b
    0x6,         // c
    0x7,         // d
    0x8,         // e
    0x9,         // f
    0xA,         // g
    0xB,         // h
    0xC,         // i
    0xD,         // j
    0xE,         // k
    0xF,         // l
    0x10,        // m
    0x11,        // n
    0x12,        // o
    0x13,        // p
    0x14,        // q
    0x15,        // r
    0x16,        // s
    0x17,        // t
    0x18,        // u
    0x19,        // v
    0x1A,        // w
    0x1B,        // x
    0x1C,        // y
    0x1D,        // z
    0x80 | 0x2F, // {
    0x80 | 0x31, // |
    0x80 | 0x30, // }
    0x80 | 0x35, // ~
    2,           // Box
    2,           // C with this bottom thingy
    2,           // ü
    2,           // é
    2,           // â
    2,           // ä
    2,           // a with bar ontop
    2,           // a but swedish
    2,           // c with this bottom thingy
    2,           // ê
    2,           // e with dots untop
    2,           // e with bar ontop
    2,           // i with 2 dots
    2,           // î
    2,           // Ä
    2,           // A but swedish
    2,           // Ê
    2,           // ae
    2,           // AE
    2,           // ô
    2,           // ö
    2,           // ò
    2,           // û
    2,           // ù
    2,           // y with 2 dots
    2,           // Ö
    2,           // Ü
    2,           // ascii 0x9B
    2,           // ascii 0x9C
    2,           // yen symbol
    2,           // ascii 0x9E
    2,           // fancy f
    2,           // á
    2,           // í
    2,           // ó
    2,           // ú
    2,           // n with tilde
    2,           // N with tilde
    2,           // high case 2
    2,           // circle
    2,           // bottom question mark
    2,           // logical neg
    2,           // ascii 0xAA
    2,           // 1/2 fraction
    2,           // 1/4 fraction
    2,           // ascii 0xAD
    2,           // <<
    2,           // >>
    2,           // ascii 0xB0
    2,           // ascii 0xB1
    2,           // ascii 0xB2
    2,           // |
    2,           // ascii 0xB4
    2,           // ascii 0xB5
    2,           // ascii 0xB6
    2,           // ascii 0xB7
    2,           // ascii 0xB8
    2,           // ascii 0xB9
    2,           // ascii 0xBA
    2,           // ascii 0xBB
    2,           // ascii 0xBC
    2,           // ascii 0xBD
    2,           // ascii 0xBD
    2,           // ascii 0xBE
    2,           // ascii 0xBF
    2,           // ascii 0xC0
    2,           // ascii 0xC1
    2,           // ascii 0xC2
    2,           // ascii 0xC3
    2,           // ascii 0xC4
    2,           // ascii 0xC5
    2,           // ascii 0xC6
    2,           // ascii 0xC7
    2,           // ascii 0xC8
    2,           // ascii 0xC9
    2,           // ascii 0xCA
    2,           // ascii 0xCB
    2,           // ascii 0xCC
    2,           // ascii 0xCD
    2,           // ascii 0xCE
    2,           // ascii 0xCF
    2,           // ascii 0xD0
    2,           // ascii 0xD1
    2,           // ascii 0xD2
    2,           // ascii 0xD3
    2,           // ascii 0xD4
    2,           // ascii 0xD5
    2,           // ascii 0xD6
    2,           // ascii 0xD7
    2,           // ascii 0xD8
    2,           // ascii 0xD9
    2,           // ascii 0xDA
    2,           // ascii 0xDB
    2,           // ascii 0xDC
    2,           // ascii 0xDD
    2,           // ascii 0xDE
    2,           // ascii 0xDF
    2,           // ascii 0xE0
    2,           // ascii 0xE1
    2,           // ascii 0xE2
    2,           // ascii 0xE3
    2,           // ascii 0xE4
    2,           // ascii 0xE5
    2,           // ascii 0xE6
    2,           // ascii 0xE7
    2,           // ascii 0xE8
    2,           // ascii 0xE9
    2,           // ascii 0xEA
    2,           // ascii 0xEB
    2,           // ascii 0xEC
    2,           // ascii 0xED
    2,           // ascii 0xEF
    2,           // ascii 0xF0
    2,           // ascii 0xF1
    2,           // ascii 0xF2
    2,           // ascii 0xF3
    2,           // ascii 0xF4
    2,           // ascii 0xF5
    2,           // ascii 0xF6
    2,           // ascii 0xF7
    2,           // ascii 0xF8
    2,           // ascii 0xF9
    2,           // ascii 0xFA
    2,           // ascii 0xFB
    2,           // ascii 0xFC
    2,           // ascii 0xFD
    2,           // ascii 0xFE
    2,           // ascii 0xFF
];

pub static KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED: u8 = 116;
/*
 * Maps input from us usb keyboard to ascii table
 * Only when Shift or Capslock isn´t pressed.
 */
static US_USB_KEY_TRANSLATION_TABLE: [u8; 116] = [
    0xFF,       // Reserved
    0xFF,       // ErrorRollOver
    0xFF,       // POSTFail
    0xFF,       // ErrorUndefined
    0x61,       // a
    0x62,       // b
    0x63,       // c
    0x64,       // d
    0x65,       // e
    0x66,       // f
    0x67,       // g
    0x68,       // h
    0x69,       // i
    0x6A,       // j
    0x6B,       // k
    0x6C,       // l
    0x6D,       // m
    0x6E,       // n
    0x6F,       // o
    0x70,       // p
    0x71,       // q
    0x72,       // r
    0x73,       // s
    0x74,       // t
    0x75,       // u
    0x76,       // v
    0x77,       // w
    0x78,       // x
    0x79,       // y
    0x7A,       // z
    0x31,       // 1
    0x32,       // 2
    0x33,       // 3
    0x34,       // 4
    0x35,       // 5
    0x36,       // 6
    0x37,       // 7
    0x38,       // 8
    0x39,       // 9
    0x30,       // 0
    '\r' as u8, // Return (Enter)
    0xFF,       // Escape
    0xFF,       // Delete (Backspace)
    '\t' as u8, // Tab
    0x20,       // Space
    0x2D,       // -
    0x3D,       // =
    0x5B,       // [
    0x5D,       // ]
    0x5C,       // \
    0xFF,       // Non-US
    0x3B,       // ;
    0x27,       // '
    0x60,       // ` (Grave Accent)
    0x2C,       // ,
    0x2E,       // .
    0x2F,       // /
    0xFF,       // Caps
    0xFF,       // F1
    0xFF,       // F2
    0xFF,       // F3
    0xFF,       // F4
    0xFF,       // F5
    0xFF,       // F6
    0xFF,       // F7
    0xFF,       // F8
    0xFF,       // F9
    0xFF,       // F10
    0xFF,       // F11
    0xFF,       // F12
    0xFF,       // Print Screen
    0xFF,       // Scroll Lock
    0xFF,       // Pause
    0xFF,       // Insert
    0xFF,       // Home
    0xFF,       // PageUp
    0xFF,       // Delete Forward
    0xFF,       // End
    0xFF,       // PageDown
    0xFF,       // Right Arrow
    0xFF,       // LeftArrow
    0xFF,       // DownArrow
    0xFF,       // UpArrow
    0xFF,       // Num Lock
    0x2F,       // Keypad /
    0x2A,       // Keypad *
    0x2D,       // Keypad -
    0x2B,       // Keypad +
    0xFF,       // Keypad Enter
    0x31,       // Keypad 1
    0x32,       // Keypad 2
    0x33,       // Keypad 3
    0x34,       // Keypad 4
    0x35,       // Keypad 5
    0x36,       // Keypad 6
    0x37,       // Keypad 7
    0x38,       // Keypad 8
    0x39,       // Keypad 9
    0x30,       // Keypad 0
    0x2E,       // .
    0xFF,       // Non-US
    0xFF,       // Application
    0xFF,       // Power
    0x3D,       // =
    0xFF,       // F13
    0xFF,       // F14
    0xFF,       // F15
    0xFF,       // F16
    0xFF,       // F17
    0xFF,       // F18
    0xFF,       // F19
    0xFF,       // F20
    0xFF,       // F21
    0xFF,       // F22
    0xFF,       // F23
    0xFF,       // F24
];
#[unsafe(no_mangle)]
static mut USB_KEYBOARD_TRANSLATION_TABLE: [u8; (KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED * 2)
    as usize] = [0; (KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED * 2) as usize];
static mut ASCII_TRANSLATION_TABLE: [u8; 256] = [0; 256];

pub enum KeyboardType {
    /* Report Size, Scancode Buffer*/
    Usb(u16, *const c_void),
    Ps2,
    NotPresent,
}

pub struct Keyboard {
    _type: KeyboardType,
}

impl Default for Keyboard {
    fn default() -> Self {
        return Self {
            _type: KeyboardType::NotPresent,
        };
    }
}
#[unsafe(no_mangle)]
static mut SCANCODE_BUFFER: *const u8 = null_mut();
pub static mut GLOBAL_KEYBOARD: *mut Keyboard = null_mut();

/* Usb */
static mut LAST_SCAN: [u8; 8] = [0; 8];
static mut PRESSED_KEYS: [u32; 8] = [0; 8];

/* ASCII Keys, goto ASCII_TRANSLATION_TABLE*/
static mut TRANSLATED_KEYS: [u8; 6] = [0; 6];
static mut NUM_TRANSLATED_KEYS: u8 = 0;
static mut NUM_KEYS_PRESSED: u8 = 0;
static mut UPPERCASE: bool = false;
#[inline(always)]
fn usb_update_modifier_keys() {
    // According to USB Document HID1_7 this is valid.
    unsafe { PRESSED_KEYS[0xE0 / 32] = LAST_SCAN[0] as u32 };
}
#[allow(unsafe_op_in_unsafe_fn)]
#[inline(always)]
unsafe fn usb_press_key(key: u8) {
    PRESSED_KEYS[key as usize / 32] |= 1 << (key % 32);
}
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn usb_unpress_key(key: u8) {
    PRESSED_KEYS[key as usize / 32] &= 0xFFFFFFFF ^ (1 << (key % 32));
}

fn usb_translate_key(key: u8) -> Option<u8> {
    let translation_buffer;
    if unsafe { !UPPERCASE } {
        translation_buffer = unsafe { &raw const USB_KEYBOARD_TRANSLATION_TABLE[0] };
    } else {
        translation_buffer = unsafe {
            &raw const USB_KEYBOARD_TRANSLATION_TABLE[KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED as usize]
        };
    }
    if key > KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED {
        return Option::None;
    }
    let key_code = unsafe { *translation_buffer.add(key as usize) };
    if key_code == 0xFF {
        return Option::None;
    } else {
        return Option::Some(key_code);
    }
}

static mut KEYBOARD_LAST_MS_PRESSED: u64 = 0;
impl Keyboard {
    pub const MS_DELAY: u64 = 10;
    /*
     * NOTICE: When debugging this and all corrosponding functions of the keyboard, the release key will not be recorded
     * Every function has to be written in 1 go.
     * IMPLEMENT THIS!
     */
    #[allow(static_mut_refs)]
    fn usb_report(_usb_controller: &dyn UsbController) {
        /*
            let mut module = Module::new("usb_report");
        unsafe {
            info!(
                &mut module,
                "{}: {} {} {} {}",
                *(SCANCODE_BUFFER),
                *(SCANCODE_BUFFER.add(2)),
                *(SCANCODE_BUFFER.add(3)),
                *(SCANCODE_BUFFER.add(4)),
                *(SCANCODE_BUFFER.add(5))
            );
        }*/
        unsafe {
            let scancode_buffer = SCANCODE_BUFFER;

            // No keys were changed
            if (&raw mut LAST_SCAN as *mut u64).read_unaligned()
                == (scancode_buffer as *const u64).read_unaligned()
            {
                return;
            } else {
                if *SCANCODE_BUFFER != LAST_SCAN[0] {
                    usb_update_modifier_keys();

                    if *SCANCODE_BUFFER & (1 << CONTROL_CHARACTER_LEFT_SHIFT)
                        != LAST_SCAN[0] & (1 << CONTROL_CHARACTER_LEFT_SHIFT)
                    {
                        UPPERCASE = !UPPERCASE;
                    }
                    LAST_SCAN[0] = *SCANCODE_BUFFER;
                }

                let mut num_keys_pressed = 1 * (*SCANCODE_BUFFER.add(2) != 0) as u8
                    + 1 * (*SCANCODE_BUFFER.add(3) != 0) as u8
                    + 1 * (*SCANCODE_BUFFER.add(4) != 0) as u8
                    + 1 * (*SCANCODE_BUFFER.add(5) != 0) as u8
                    + 1 * (*SCANCODE_BUFFER.add(6) != 0) as u8
                    + 1 * (*SCANCODE_BUFFER.add(7) != 0) as u8;
                let backup_num_keys_pressed = num_keys_pressed;
                // safety
                if num_keys_pressed == 0 {
                    for i in 0..NUM_KEYS_PRESSED {
                        usb_unpress_key(LAST_SCAN[2 + i as usize]);
                    }
                    NUM_KEYS_PRESSED = 0;
                    memset(&raw mut LAST_SCAN[2] as *mut c_void, 0, 6);
                    return;
                }
                let keys = SCANCODE_BUFFER.add(2);

                NUM_TRANSLATED_KEYS = 0;
                // TODO: this can be improved by mathmatical if´ing this.
                if *keys.add(num_keys_pressed as usize - 1) == 0x1 {
                    return;
                } else if *keys.add(num_keys_pressed as usize - 1) == 0x39 {
                    // Capslock pressed
                    UPPERCASE = !UPPERCASE;
                    // Toggles Capslock
                    PRESSED_KEYS[0x39 / 32] ^= 1 << (0x39 % 32);

                    for i in 0..NUM_KEYS_PRESSED {
                        usb_unpress_key(LAST_SCAN[2 + i as usize]);
                    }
                    /*
                     * - 2, since CAPSLOCK cannot be translated and when multiple keys are pressed, those have to be translated
                     * Those Keys start at - 2
                     */
                    if num_keys_pressed > 2 {
                        while num_keys_pressed - 2 != 0 {
                            let index = num_keys_pressed - 2;
                            usb_press_key(*keys.add(index as usize));

                            let translated = usb_translate_key(*keys.add(index as usize));

                            // Key was translatable
                            if let Option::Some(key) = translated {
                                TRANSLATED_KEYS[NUM_TRANSLATED_KEYS as usize] = key;
                                NUM_TRANSLATED_KEYS += 1;
                            }

                            num_keys_pressed -= 1;
                        }
                    }
                } else {
                    for i in 0..NUM_KEYS_PRESSED {
                        usb_unpress_key(LAST_SCAN[2 + i as usize]);
                    }

                    while num_keys_pressed != 0 {
                        usb_press_key(*keys.add(num_keys_pressed as usize - 1));
                        let translated =
                            usb_translate_key(*keys.add(num_keys_pressed as usize - 1));

                        // Key was translatable
                        if let Option::Some(key) = translated {
                            TRANSLATED_KEYS[NUM_TRANSLATED_KEYS as usize] = key;
                            NUM_TRANSLATED_KEYS += 1;
                        }
                        num_keys_pressed -= 1;
                    }
                }

                memcpy(
                    &raw mut LAST_SCAN[2] as *mut c_void,
                    SCANCODE_BUFFER.add(2) as *const c_void,
                    6,
                );

                NUM_KEYS_PRESSED = backup_num_keys_pressed;
            }
        }
        return;
    }

    pub fn is_key_pressed(key: c_uchar) -> bool {
        let designated_key = unsafe { ASCII_TRANSLATION_TABLE[key as usize] };
        return unsafe { PRESSED_KEYS[designated_key as usize / 32] & 1 << (designated_key % 32) }
            != 0;
    }

    pub fn is_key_pressed_delay(key: c_uchar) -> bool {
        let current = time::tick_in_ms();
        if unsafe { KEYBOARD_LAST_MS_PRESSED } + Keyboard::MS_DELAY > current {
            return false;
        }
        unsafe { KEYBOARD_LAST_MS_PRESSED = current };
        let designated_key = unsafe { ASCII_TRANSLATION_TABLE[key as usize] };
        return unsafe { PRESSED_KEYS[designated_key as usize / 32] & 1 << (designated_key % 32) }
            != 0;
    }

    pub fn get_pressed_keys() -> Option<(u8, &'static [u8; 6])> {
        let current = time::tick_in_ms();
        if unsafe { KEYBOARD_LAST_MS_PRESSED } + Keyboard::MS_DELAY > current {
            return Option::None;
        }
        unsafe {
            KEYBOARD_LAST_MS_PRESSED = current;
        }
        #[allow(static_mut_refs)]
        let ret_tuple = (unsafe { NUM_TRANSLATED_KEYS }, unsafe { &TRANSLATED_KEYS });
        return Option::Some(ret_tuple);
    }

    //TOOD: properly implement this!
    fn choose_keyboard(module: &mut Module) {
        info!(module, "Keyset: EN-US \n");

        let keycode_to_ascii_dst =
            unsafe { &raw mut USB_KEYBOARD_TRANSLATION_TABLE[0] } as *mut c_void;
        let ascii_to_keycode_dst = unsafe { &raw mut ASCII_TRANSLATION_TABLE[0] } as *mut c_void;
        unsafe {
            memcpy(
                keycode_to_ascii_dst,
                (&raw const US_USB_KEY_TRANSLATION_TABLE[0]) as *const c_void,
                KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED as u32,
            );
            memcpy(
                keycode_to_ascii_dst.add(KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED as usize),
                (&raw const US_USB_KEY_UPPERCASE_TRANSLATION[0]) as *const c_void,
                KEYBOARD_MAXIMUM_USB_KEY_IMPLEMENTED as u32,
            );
            memcpy(
                ascii_to_keycode_dst,
                (&raw const US_USB_ASCII_TRANSLATION_TABLE[0]) as *const c_void,
                256,
            );
        }
    }

    pub fn new_usb(
        physical_allocator: &mut Allocator,
        controller: &mut dyn UsbController,
        usb_device: &mut dyn UsbDevice,
        usb_hid: &UsbHID,
    ) -> Self {
        let scancode_buffer: *const u8 = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => {
                simple_kernel_panic("Keyboard/new_usb", "Could not allocate scancode buffer\n")
            }
        };

        let mut module = Module::new("Keyboard");
        Keyboard::choose_keyboard(&mut module);

        unsafe { SCANCODE_BUFFER = scancode_buffer };
        // Stupid Borrowchecker
        if usb_device
            .get_configuration(0)
            .unwrap()
            .get_interface(0)
            .unwrap()
            .get_sub_class()
            != 1
        {
            simple_kernel_panic("Keyboard/new_usb", "Boot Protocol is not supported\n");
        }
        usb_device.set_protocol(0xB, BOOT_PROTOCOL, 0);

        let interface = usb_device
            .get_configuration(0)
            .unwrap()
            .get_interface(0)
            .unwrap();
        let endpoint = interface.get_endpoint(0).unwrap();
        let interval_in_ms = endpoint.get_interval_in_ms();
        controller.install_interrupt_poller(
            usb_device,
            0,
            0,
            interval_in_ms as u8,
            scancode_buffer as u32,
            8,
            Option::Some(Keyboard::usb_report),
        );
        return Self {
            _type: KeyboardType::Usb(usb_hid.descriptor_length, scancode_buffer as *const c_void),
        };
    }
}
