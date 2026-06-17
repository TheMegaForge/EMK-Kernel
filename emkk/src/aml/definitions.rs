use core::ffi::{c_uchar, c_void};

use crate::aml::NameString;

pub enum DataRefObject {
    Zero,
    One,
    Ones,
    Byte(u8),
    Word(u16),
    DWord(u32),
    QWord(u64),
    RevisionOp,
    /**
     * [*const c_uchar] => ptr (chars)
     * [u16] => length
     */
    String(*const c_uchar, u16),
    /**
     * [*const u8] => ptr (data)
     * [u16] => length
     */
    Buffer(*const u8, u32),
    /**
     * [*const c_void] => ptr (data)
     * [u16] => length
     * [u8] => num_elements
     */
    Package(*const c_void, u16, u8),
}

impl Clone for DataRefObject {
    fn clone(&self) -> Self {
        return match self {
            Self::Zero => Self::Zero,
            Self::One => Self::One,
            Self::Ones => Self::Ones,
            Self::Byte(b) => Self::Byte(*b),
            Self::Word(w) => Self::Word(*w),
            Self::DWord(dw) => Self::DWord(*dw),
            Self::QWord(qw) => Self::QWord(*qw),
            Self::RevisionOp => Self::RevisionOp,
            Self::String(ptr, length) => Self::String(*ptr, *length),
            Self::Buffer(ptr, length) => Self::Buffer(*ptr, *length),
            Self::Package(ptr, length, num_elements) => Self::Package(*ptr, *length, *num_elements),
        };
    }
}
#[derive(Clone, Copy)]
pub enum TermArgInt {
    Byte(u8),
    Word(u16),
    DWord(u32),
    QWord(u64),
    Zero,
    One,
    Ones,
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    Inc,
    Dec,
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Not,
    ShiftLeft,
    ShiftRight,
    FindSetLeftBit,
    SizeOf,
    ObjectType,
    LogicalEquals(*const c_void),
    NameString(NameString),
}

pub enum SimpleName {
    NameString(NameString),
    Arg(u8),
    Local(u8),
}

pub enum Supername {
    SimpleName(SimpleName),
    DebugObj,
}

#[repr(u8)]
pub enum TermArg {
    Arg(u8),
    Local(u8),
    Byte(u8),
    Word(u16),
    DWord(u32),
    QWord(u64),
    String(*const c_uchar, u16),
    Zero,
    One,
    Ones,
    ReturnOp(*const c_void),
    NameString(NameString),
}

pub enum FieldElement {
    Default,
    /**
     * [c_uchar;4] => name
     * [u32] => package_length
     */
    NamedField([c_uchar; 4], u32),

    /**
     * [u32] => package_length
     */
    ReservedField(u32),

    /**
     * [u8] => type
     * [u8] => attrib
     */
    AccessField(u8, u8),
}

impl Default for FieldElement {
    fn default() -> Self {
        return FieldElement::Default;
    }
}
