use core::{ffi::c_uchar, ptr::read};

use crate::{
    aml::{
        AmlCode, NameString,
        definitions::{DataRefObject, SimpleName, Supername, TermArg, TermArgInt},
    },
    hal::print::simple_kernel_panic,
    utils::reader::{BufferedReader, Reader},
};

pub fn is_name_string_character(c: u8) -> bool {
    let digit_char: bool = c >= '0' as u8 && '9' as u8 >= c;
    let name_char: bool = c >= 'A' as u8 && 'Z' as u8 >= c;
    return digit_char || name_char || c == '_' as u8;
}

pub fn parse_termarg(reader: &mut BufferedReader) -> TermArg {
    let oc = reader.read_u8();
    return match oc {
        0 => TermArg::Zero,
        1 => TermArg::One,
        0xFF => TermArg::Ones,
        0x0A => {
            let b = reader.read_u8();
            TermArg::Byte(b)
        }
        0x0B => {
            let w = reader.read_u16();
            TermArg::Word(w)
        }
        0x0C => {
            let dw = reader.read_u32();
            TermArg::DWord(dw)
        }
        0x0D => {
            let begin = reader.current();
            let mut length = 0u16;
            while reader.peek_u8(0) != 0 {
                reader.skip(1);
                length += 1;
            }
            reader.skip(1);
            TermArg::String(begin as *const c_uchar, length)
        }
        0x0E => {
            let qw = reader.read_u64();
            TermArg::QWord(qw)
        }
        0x60 => TermArg::Local(0),
        0x61 => TermArg::Local(1),
        0x62 => TermArg::Local(2),
        0x63 => TermArg::Local(3),
        0x64 => TermArg::Local(4),
        0x65 => TermArg::Local(5),
        0x66 => TermArg::Local(6),
        0x68 => TermArg::Arg(0),
        0x69 => TermArg::Arg(1),
        0x6A => TermArg::Arg(2),
        0x6B => TermArg::Arg(3),
        0x6C => TermArg::Arg(4),
        0x6D => TermArg::Arg(5),
        0x6E => TermArg::Arg(6),
        0xA4 => TermArg::ReturnOp(reader.current()),
        0x2F | 0x2E | 0x5C => {
            reader.go_back(1);
            TermArg::NameString(parse_name_string(reader))
        }
        _ => {
            if is_name_string_character(oc) {
                reader.go_back(1);
                TermArg::NameString(parse_name_string(reader))
            } else {
                simple_kernel_panic("Acpi/Machine Code", "parse_termarg: invalid oc\n")
            }
        }
    };
}
pub fn parse_supername(reader: &mut BufferedReader) -> Supername {
    let oc0 = reader.read_u8();
    return match oc0 {
        0x5B => {
            let oc1 = reader.read_u8();
            match oc1 {
                0x31 => Supername::DebugObj,
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "parse_supername: invalid extended oc\n",
                ),
            }
        }
        0x60 => Supername::SimpleName(SimpleName::Local(0)),
        0x61 => Supername::SimpleName(SimpleName::Local(1)),
        0x62 => Supername::SimpleName(SimpleName::Local(2)),
        0x63 => Supername::SimpleName(SimpleName::Local(3)),
        0x64 => Supername::SimpleName(SimpleName::Local(4)),
        0x65 => Supername::SimpleName(SimpleName::Local(5)),
        0x66 => Supername::SimpleName(SimpleName::Local(6)),
        0x68 => Supername::SimpleName(SimpleName::Arg(0)),
        0x69 => Supername::SimpleName(SimpleName::Arg(1)),
        0x6A => Supername::SimpleName(SimpleName::Arg(2)),
        0x6B => Supername::SimpleName(SimpleName::Arg(3)),
        0x6C => Supername::SimpleName(SimpleName::Arg(4)),
        0x6D => Supername::SimpleName(SimpleName::Arg(5)),
        0x6E => Supername::SimpleName(SimpleName::Arg(6)),
        0x2F | 0x2E | 0x5C => {
            reader.go_back(1);
            Supername::SimpleName(SimpleName::NameString(parse_name_string(reader)))
        }
        _ => {
            if is_name_string_character(oc0) {
                reader.go_back(1);
                Supername::SimpleName(SimpleName::NameString(parse_name_string(reader)))
            } else {
                simple_kernel_panic("Acpi/Machine Code", "parse_supername: invalid oc\n")
            }
        }
    };
}

pub fn parse_name_string(reader: &mut impl Reader) -> NameString {
    let mut indicator = reader.read_u8();
    let mut absolute = false;
    if indicator == '\\' as u8 || indicator == 0x5E {
        indicator = reader.read_u8();
        absolute = true;
        if indicator == '\0' as u8 {
            return NameString::Single([0x5C, 0, 0, 0], true);
        } else {
            if !is_name_string_character(reader.peek_u8(3)) {
                let mut buf: [u8; 3] = [0; 3];
                reader.read_bytes(&mut buf);
                return NameString::Single([indicator, buf[0], buf[1], buf[2]], true);
            }
        }
    }
    if indicator == 0x2F
    /* Multi */
    {
        let num_segments = reader.read_u8();
        let ptr = reader.current();
        reader.skip(num_segments as u32 * 4);
        return NameString::Multiple(ptr as *const u8, num_segments, absolute);
    } else if indicator == 0x2E
    /* Dual */
    {
        let ptr = reader.current();
        reader.skip(8);
        return NameString::Dual(ptr as *const u8, absolute);
    } else {
        if reader.peek_u8(0) == 0 {
            reader.skip(1);
            return NameString::Single([indicator, 0, 0, 0], absolute);
        }
        let mut buf: [u8; 3] = [0; 3];
        reader.read_bytes(&mut buf);
        return NameString::Single([indicator, buf[0], buf[1], buf[2]], absolute);
    }
}

pub fn parse_pkglen(reader: &mut impl Reader) -> u32 {
    let fd = reader.read_u8();
    let following_bytes = (fd & 0b11000000) >> 6;
    if following_bytes == 0 {
        return (fd & 0b00111111) as u32;
    } else {
        let mut ret = (fd & 0b00001111) as u32;
        for i in 0..following_bytes {
            let val = reader.read_u8();
            ret |= (val as u32) << (4 + (8 * i));
        }
        return ret;
    }
}

pub fn parse_termarg_int(reader: &mut impl Reader) -> TermArgInt {
    let inst = reader.read_u8();
    return match inst {
        0x0 => TermArgInt::Zero,
        0x1 => TermArgInt::One,
        0xA => {
            let b = reader.read_u8();
            TermArgInt::Byte(b)
        }
        0xB => {
            let w = reader.read_u16();
            TermArgInt::Word(w)
        }
        0xC => {
            let dw = reader.read_u32();
            TermArgInt::DWord(dw)
        }
        0xE => {
            let qw = reader.read_u64();
            TermArgInt::QWord(qw)
        }
        0x93 => {
            let c = reader.current();
            // Consumes both Operands. those operands cannot be stored in LogicalEquals, since it would turn TermArgInt sizeless.
            parse_termarg_int(reader);
            parse_termarg_int(reader);
            TermArgInt::LogicalEquals(c)
        }
        0x2F | 0x2E | 0x5C => {
            reader.go_back(1);
            TermArgInt::NameString(parse_name_string(reader))
        }
        _ => {
            if is_name_string_character(inst) {
                reader.go_back(1);
                TermArgInt::NameString(parse_name_string(reader))
            } else {
                simple_kernel_panic("Acpi/Machine Code", "parse_termarg_int: Invalid Inst\n")
            }
        }
    };
}

pub fn parse_data_ref_object(aml_code: &AmlCode, reader: &mut impl Reader) -> DataRefObject {
    let oc = reader.read_u8();
    return match oc {
        0 => DataRefObject::Zero,
        1 => DataRefObject::One,
        0xA => {
            let byte = reader.read_u8();
            DataRefObject::Byte(byte)
        }

        0xB => {
            let word = reader.read_u16();
            DataRefObject::Word(word)
        }
        0xC => {
            let dword = reader.read_u32();
            DataRefObject::DWord(dword)
        }
        0xE => {
            let qword = reader.read_u64();
            DataRefObject::QWord(qword)
        }
        0xD => {
            let begin = reader.current();
            let mut length = 0u16;
            while unsafe{*(reader.current() as *const u8)} != 0 {
                reader.skip(1);
                length += 1;
            }
            reader.skip(1);
            DataRefObject::String(begin as *const u8, length)
        }
        0x11 /* Buffer */ => {
            _ = parse_pkglen(reader); // package length is unused
            let buffer_size = parse_termarg_int(reader);
            let buffer_base = reader.current();
            let buffer_size = match buffer_size {
                TermArgInt::Byte(b) => b as u32,
                TermArgInt::Word(w) => w as u32,
                TermArgInt::DWord(dw) => dw,
                _ => simple_kernel_panic("Acpi/Machine Code", "parse_data_ref_object: Invalid Buffer Size\n")
            };
            reader.skip(buffer_size);
            DataRefObject::Buffer(buffer_base as *const u8, buffer_size)
        }
        0x12 /* Package */=> {
            let current = reader.current();
            let mut package_length = parse_pkglen(reader);
            package_length-=1;
            package_length-= unsafe{reader.current().offset_from(current) } as u32;
            let num_elements = reader.read_u8();
            let ptr = reader.current();
            reader.skip(package_length);
            DataRefObject::Package(ptr, package_length as u16,num_elements)
        }
        _ => {
            if is_name_string_character(oc) {
                reader.go_back(1);
                let name = parse_name_string(reader);
                return match aml_code.get_name_entry(&name) {
                    Some(ret_) => ret_.clone(),
                    None => simple_kernel_panic("Acpi/Machine Code", "parse_data_ref_object: Invalid name\n"),
                };
            }else {
                simple_kernel_panic("Acpi/Machine Code", "parse_data_ref_object: Invalid Identifier\n");
            }
        }
    };
}

pub fn unwrap_data_ref_object_int(data_ref_object: *const DataRefObject) -> u64 {
    return match unsafe { data_ref_object.as_ref().unwrap() } {
        DataRefObject::Byte(b) => *b as u64,
        DataRefObject::Word(w) => *w as u64,
        DataRefObject::DWord(dw) => *dw as u64,
        DataRefObject::QWord(qw) => *qw,
        DataRefObject::Zero => 0,
        DataRefObject::One => 1,
        DataRefObject::Ones => 0xFFFFFFFFFFFFFFFF,
        _ => simple_kernel_panic(
            "Acpi/Machine Code",
            "unwrap_data_ref_object_int: invalid data_ref_object\n",
        ),
    };
}
