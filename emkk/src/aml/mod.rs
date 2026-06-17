use core::{
    arch::asm,
    ffi::{CStr, c_uchar, c_void},
    ptr::null,
    slice,
};

pub mod definitions;
pub mod field_system;
pub mod name_system;
pub mod path_system;
pub mod utils;
use crate::{
    aml::{
        definitions::{DataRefObject, FieldElement, SimpleName, Supername, TermArg, TermArgInt},
        field_system::FieldSystem,
        name_system::NameSystem,
        path_system::PathSystem,
        utils::{
            is_name_string_character, parse_data_ref_object, parse_name_string, parse_pkglen,
            parse_supername, parse_termarg, parse_termarg_int, unwrap_data_ref_object_int,
        },
    },
    drivers::disk::nvme_registers::NVMeCsts,
    error,
    hal::{
        memory::allocator::Allocator,
        print::{Module, simple_kernel_panic},
    },
    utils::{
        allocators::PageAllocator,
        memory::{memcmp_dword_unaligned, memcpy},
        reader::{BufferedReader, Reader},
        stack::Stack,
    },
};

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum AmlType {
    Invalid,
    UnsignedNumber,
    Number,
    String,
    DataRefObject,
    Package,
}
#[derive(Clone, Copy)]
pub struct AmlMethodFrame {
    arguments: [u64; 7],
    locals: [u64; 7],
    local_types: [AmlType; 7],
    argument_types: [AmlType; 7],
}

impl Default for AmlMethodFrame {
    fn default() -> Self {
        return Self {
            arguments: [0; 7],
            locals: [0; 7],
            local_types: [AmlType::Invalid; 7],
            argument_types: [AmlType::Invalid; 7],
        };
    }
}

pub enum AmlValue {
    Empty,
    UnsignedNumber(u64),
    Number(i64),
    String(*const c_uchar, u16),
    DataRefObject(*mut DataRefObject),
    Package(*const c_void, u16),
}

impl AmlValue {
    pub fn from_u64(val: u64, aml_type: AmlType) -> AmlValue {
        return match aml_type {
            AmlType::Package => simple_kernel_panic(
                "Acpi/Machine Code",
                "AmlValue::from_u64 --- aml_type == Package\n",
            ),
            AmlType::Invalid => AmlValue::Empty,
            AmlType::Number => AmlValue::Number(val as i64),
            AmlType::UnsignedNumber => AmlValue::UnsignedNumber(val),
            AmlType::String => {
                let mut length = 0u16;
                while unsafe { *((val + length as u64) as *const u8) } != 0 {
                    length += 1;
                }
                AmlValue::String(val as *const u8, length)
            }
            AmlType::DataRefObject => AmlValue::DataRefObject(val as *mut DataRefObject),
        };
    }

    pub fn get_type(&self) -> AmlType {
        return match self {
            AmlValue::Empty => AmlType::Invalid,
            AmlValue::UnsignedNumber(_) => AmlType::UnsignedNumber,
            AmlValue::String(_, _) => AmlType::String,
            AmlValue::DataRefObject(_) => AmlType::DataRefObject,
            AmlValue::Number(_) => AmlType::Number,
            AmlValue::Package(_, _) => AmlType::Package,
        };
    }
    pub fn as_u64(&self) -> u64 {
        return match self {
            AmlValue::Empty => {
                simple_kernel_panic("Acpi/Machine Code", "AmlValue::as_u64: self == Empty\n")
            }
            AmlValue::UnsignedNumber(r) => *r,
            AmlValue::String(base, _) => *base as u64,
            AmlValue::DataRefObject(dro) => *dro as u64,
            AmlValue::Number(val) => *val as u64,
            AmlValue::Package(_, _) => simple_kernel_panic(
                "Acpi/Machine Code",
                "AmlValue::as_u64 --- self == Package\n",
            ),
        };
    }
}

pub enum AmlError {
    NotFound,
    InvalidInput,
    MismatchInLength,
    MalformedPath,
}
#[derive(Clone, Copy)]
pub enum NameString {
    /**
     * [c_char;4] => Segment
     * [bool] => Absolute
     */
    Single([c_uchar; 4], bool),

    /**
     * [*const c_char] => Ptr
     * [bool] => Absolute
     */
    Dual(*const c_uchar, bool),

    /**
     * [*const c_char] => Ptr
     * [u8] => NumSegments
     * [bool] => Absolute
     */
    Multiple(*const c_uchar, u8, bool),
}

impl NameString {
    pub fn is_absolute(&self) -> bool {
        return match self {
            Self::Single(_, absolute) => *absolute,
            Self::Dual(_, absolute) => *absolute,
            Self::Multiple(_, _, absolute) => *absolute,
        };
    }
    pub fn as_str<'a>(&'a self) -> &'a str {
        return unsafe {
            match self {
                Self::Single(val, _) => {
                    str::from_utf8_unchecked(slice::from_raw_parts((*val).as_ptr(), 4))
                }
                Self::Dual(val, _) => str::from_utf8_unchecked(slice::from_raw_parts(*val, 8)),
                Self::Multiple(val, segments, _) => {
                    str::from_utf8_unchecked(slice::from_raw_parts(*val, (*segments) as usize * 4))
                }
            }
        };
    }
}

pub struct Scope {
    path: u16,
    name_system: Option<u16>,
}

impl Scope {
    pub fn new(path: u16, name_system: Option<u16>) -> Self {
        return Self { path, name_system };
    }
}
// Device is root
pub struct Device {
    path: u16,
    name_system: Option<u16>,
}

impl Device {
    pub fn new(path: u16, name_system: Option<u16>) -> Self {
        return Self { path, name_system };
    }

    pub fn get_name_system(&self) -> &Option<u16> {
        return &self.name_system;
    }
}
#[allow(dead_code)]
pub struct Mutex {
    path: u16,
    sync_flags: u8,
    name: [c_uchar; 4],
}

impl Mutex {
    pub fn new(path: u16, sync_flags: u8, name: [c_uchar; 4]) -> Self {
        return Self {
            path,
            sync_flags,
            name,
        };
    }
}
#[allow(dead_code)]
pub struct Method {
    path: u16,
    name: [c_uchar; 4],
    flags: u8,
    sync_level: u8,
    num_bytes: u16,
    code_begin: *const c_void,
}

impl Method {
    pub fn new(
        path: u16,
        name: [c_uchar; 4],
        flags: u8,
        sync_level: u8,
        num_bytes: u16,
        code_begin: *const c_void,
    ) -> Method {
        return Self {
            path,
            name,
            flags,
            sync_level,
            num_bytes,
            code_begin,
        };
    }
}
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct OperationRegion {
    master_path: u16,
    describer_path: u16,
    describer_name: [c_uchar; 4],
    length: TermArgInt,
    offset: TermArgInt,
    region: u8,
}

impl OperationRegion {
    pub fn new(
        master_path: u16,
        describer_path: u16,
        describer_name: [c_uchar; 4],
        length: TermArgInt,
        offset: TermArgInt,
        region: u8,
    ) -> OperationRegion {
        return Self {
            master_path,
            describer_path,
            describer_name,
            length,
            offset,
            region,
        };
    }
}
#[allow(dead_code)]
pub struct Processor {
    path: u16,
    name_system_used: Option<u16>,
    proc_id: u8,
    pblk_addr: u32,
    pblk_len: u8,
}

impl Processor {
    pub fn new(
        path: u16,
        name_system_used: Option<u16>,
        proc_id: u8,
        pblk_addr: u32,
        pblk_len: u8,
    ) -> Self {
        return Self {
            path,
            name_system_used,
            proc_id,
            pblk_addr,
            pblk_len,
        };
    }
}

#[allow(dead_code)]
pub struct Field {
    fs: u16,
    path: u16,
    name: [c_uchar; 4],
    operation_region: OperationRegion,
}

impl Field {
    pub fn new(fs: u16, path: u16, name: [c_uchar; 4], operation_region: OperationRegion) -> Field {
        return Self {
            fs,
            path,
            name,
            operation_region,
        };
    }
}

pub struct AmlCode {
    bytes_rem: u64,
    current_code: *const u8,
    module: Module<'static>,
    path_system: PathSystem,
    name_system: NameSystem,
    field_system: FieldSystem,
    scopes: PageAllocator<Scope>,
    devices: PageAllocator<Device>,
    methods: PageAllocator<Method>,
    mutexe: PageAllocator<Mutex>,
    fields: PageAllocator<Field>,
    processors: PageAllocator<Processor>,
    method_frames: Stack<AmlMethodFrame>,
    name_system_stores: Stack<*mut Option<u16>>,
    pending_operation_regions: Stack<(OperationRegion, *const u8)>,
    standalone_name_system: u16,
    current_executing_path: u16,
    execution_should_return: bool,
    execution_return_data: *const c_void,
}

impl Default for AmlCode {
    fn default() -> Self {
        return Self {
            bytes_rem: 0,
            current_code: null(),
            module: Module::default(),
            path_system: PathSystem::default(),
            name_system: NameSystem::default(),
            field_system: FieldSystem::default(),
            scopes: PageAllocator::default(),
            devices: PageAllocator::default(),
            methods: PageAllocator::default(),
            mutexe: PageAllocator::default(),
            fields: PageAllocator::default(),
            processors: PageAllocator::default(),
            method_frames: Stack::default(),
            pending_operation_regions: Stack::default(),
            name_system_stores: Stack::default(),
            standalone_name_system: 0,
            current_executing_path: 0,
            execution_should_return: false,
            execution_return_data: null(),
        };
    }
}

impl AmlCode {
    pub fn find_path_system(&self, path: &str, base: u16) -> Option<u16> {
        return self
            .path_system
            .find(base, path.as_ptr() as *const u8, (path.len() / 4) as u8);
    }

    pub fn get_content_of_name_system(
        &self,
        ns: u16,
        name: [c_uchar; 4],
    ) -> Option<&DataRefObject> {
        self.name_system.get(ns, name)
    }

    pub fn execute_method(
        &mut self,
        path: &CStr,
        method_name: &CStr,
        parameters: &[AmlValue],
    ) -> Result<AmlValue, AmlError> {
        let path_is_root;
        if path.count_bytes() == 0
            || (path.count_bytes() == 1 && unsafe { *path.as_ptr() as u8 as char } == '\\')
        {
            path_is_root = true;
        } else {
            path_is_root = false;
        }
        if method_name.count_bytes() != 4 {
            return Result::Err(AmlError::InvalidInput);
        }

        let mut method_path;
        if path_is_root {
            method_path = 1;
        } else {
            method_path = 0;
        }

        for m in 0..self.methods.size() {
            let method_name0: [c_uchar; 4];
            {
                let method = self.methods.as_ref(m).unwrap();
                method_path = method.path;
                method_name0 = method.name.clone();
            }
            let same_name = unsafe {
                memcmp_dword_unaligned(
                    method_name0.as_ptr() as *const u32,
                    method_name.as_ptr() as *const u32,
                    1,
                )
            };
            if path_is_root {
                if !same_name || method_path != 1 {
                    continue;
                }
            } else {
                if same_name {
                    unsafe { asm!("nop") }
                }

                if !self.path_system.compare_extern_single(
                    method_path,
                    path.as_ptr() as *const c_uchar,
                    path.count_bytes() as u16,
                ) || !same_name
                {
                    continue;
                }
            }
            return self.execute_method0(m, parameters);
        }
        return Result::Err(AmlError::NotFound);
    }

    pub fn execute_method_direct_path(
        &mut self,
        path: u16,
        method_name: &CStr,
        parameters: &[AmlValue],
    ) -> Result<AmlValue, AmlError> {
        if method_name.count_bytes() != 4 {
            return Result::Err(AmlError::InvalidInput);
        }
        for m in 0..self.methods.size() {
            let method_name0: [c_uchar; 4];
            let method_path;
            {
                let method = self.methods.as_ref(m).unwrap();
                method_path = method.path;
                method_name0 = method.name.clone();
            }
            let same_name = unsafe {
                memcmp_dword_unaligned(
                    method_name0.as_ptr() as *const u32,
                    method_name.as_ptr() as *const u32,
                    1,
                )
            };
            if !same_name {
                continue;
            }

            if method_path == path {
                return self.execute_method0(m, parameters);
            }
        }
        return Result::Err(AmlError::NotFound);
    }

    // TODO: test!
    fn execute_method0(
        &mut self,
        method_index: u32,
        parameters: &[AmlValue],
    ) -> Result<AmlValue, AmlError> {
        let method = self.methods.as_ref(method_index).unwrap();
        let method_path = method.path;
        if (method.flags & 0b111) != parameters.len() as u8 {
            return Result::Err(AmlError::MismatchInLength);
        }

        let mut method_frame = AmlMethodFrame::default();
        for p in 0..parameters.len() {
            let parameter: &AmlValue = &parameters[p];
            method_frame.arguments[p] = parameter.as_u64();
            method_frame.argument_types[p] = parameter.get_type();
        }

        self.method_frames.push(method_frame);
        self.current_executing_path = method_path;
        let mut buf_reader = BufferedReader::new(method.code_begin, method.num_bytes as u32);
        while buf_reader.remaining_bytes() != 0 {
            let opcode = buf_reader.read_u8();
            self.execute_opcode(opcode, &mut buf_reader);
            if self.execution_should_return {
                self.execution_should_return = false;
                let mut reader = BufferedReader::new(self.execution_return_data, 0xFFFF);
                let ret_data = parse_termarg(&mut reader);
                self.method_frames.pop_silent();

                let (ret_val, ret_type) = self.unwrap_termarg(&ret_data);
                if let AmlValue::DataRefObject(data_ref_object) = ret_val {
                    return Result::Ok(self.unwrap_data_ref_object(data_ref_object).0);
                }
            }
            //Just overwrite it, after it has been overwritten
            self.current_executing_path = method_path;
        }
        self.method_frames.pop_silent();
        return Result::Ok(AmlValue::Empty);
    }
    //TODO: Sort some things out!
    fn execute_opcode(&mut self, opcode: u8, reader: &mut BufferedReader) {
        match opcode {
            0xa0 /* IfOp*/ => {
                let mut pkglen = parse_pkglen(reader);
                let mut current = reader.current();
                let predicate = parse_termarg_int(reader);
                let sub = reader.current() as u64 - current as u64;
                pkglen -= sub as u32;
                current = reader.current();

                let result = self.eval_termarg_int(&predicate);
                if result == 1 {
                    // If Block
                    while (unsafe {reader.current().sub(current as usize)} as u32) <= pkglen && !self.execution_should_return{
                        self.execute_termarg(reader);
                    }
                    return;
                }else {
                    // Else Block. PkgLen includes else op. This is why -1
                    reader.skip(pkglen - 1);
                    let oc = reader.read_u8();
                    if oc == 0xa1 /* ElseOp*/ {
                        return self.execute_opcode(0xa1, reader);
                    }else {
                        // go backwards once, since this is a instruction.
                        reader.go_back(1);
                        return;
                    }
                }
            }
            0xa1 /* ElseOp*/ => {
                let pkglen = parse_pkglen(reader);
                let current = reader.current();
                while ((reader.current() as u64 - current as u64)  as u32) <= pkglen && !self.execution_should_return{
                    self.execute_termarg(reader);
                }
                return;
            }
            0x70 /* Store*/ => {
                // Stores TermArg In Supername
                let termarg   = parse_termarg(reader);
                let supername = parse_supername(reader);

                let (source, source_type) = self.unwrap_termarg(&termarg);

                let dest: AmlValue;
                let dest_type: AmlType;

                match supername {
                    Supername::DebugObj => todo!("execute_opcode: StoreOp -- Supername::DebugObj not implemented\n"),
                    Supername::SimpleName(simple_name) => {
                        match simple_name {
                            SimpleName::Arg(a) => {
                                self.store_argument(a, &source, &source_type);
                                return;
                            }
                            SimpleName::Local(l) => {
                                self.store_local(l, &source, &source_type);
                                return;
                            }
                            SimpleName::NameString(name_string) => {
                                (dest, dest_type) = self.unwrap_namestring(&name_string);
                            }
                        }
                    }
                }

                if let AmlValue::DataRefObject(dro) = dest {
                    self.store_data_ref_object(dro, source, source_type);
                }

            }
            _ => simple_kernel_panic("Acpi/Machine Code", "execute_opcode: unknown opcode\n"),
        }
    }

    fn execute_termarg(&mut self, reader: &mut BufferedReader) {
        let termarg = parse_termarg(reader);
        match termarg {
            TermArg::ReturnOp(val_addr) => {
                self.execution_should_return = true;
                self.execution_return_data = val_addr;
                return;
            }
            _ => simple_kernel_panic("Acpi/Machine Code", "execute_termarg: unhandled termarg\n"),
        }
    }

    fn eval_termarg_int(&mut self, termarg: &TermArgInt) -> u64 {
        return match termarg {
            TermArgInt::Zero => 0,
            TermArgInt::One => 1,
            TermArgInt::Ones => 0xFFFFFFFFFFFFFFFF,
            TermArgInt::Byte(b) => *b as u64,
            TermArgInt::Word(w) => *w as u64,
            TermArgInt::DWord(dw) => *dw as u64,
            TermArgInt::QWord(qw) => *qw as u64,
            TermArgInt::LogicalEquals(base) => {
                let mut reader = BufferedReader::new(*base, 0xFFFFF);
                let operand0 = parse_termarg_int(&mut reader);
                let operand1 = parse_termarg_int(&mut reader);
                if self.eval_termarg_int(&operand0) == self.eval_termarg_int(&operand1) {
                    1
                } else {
                    0
                }
            }
            TermArgInt::NameString(name_string) => {
                let (value, value_type) = self.unwrap_namestring(name_string);
                match value {
                    AmlValue::DataRefObject(data_ref_object) => {
                        return unwrap_data_ref_object_int(data_ref_object);
                    }

                    _ => simple_kernel_panic(
                        "Acpi/Machine Code",
                        "eval_termarg_int: invalid type from unwrap_namestring\n",
                    ),
                }
            }
            _ => simple_kernel_panic("Acpi/Machine Code", "eval_termarg_int: Invalid termarg\n"),
        };
    }

    fn store_argument(&mut self, argument_index: u8, val: &AmlValue, val_type: &AmlType) {
        match self.method_frames.ref_top().argument_types[argument_index as usize] {
            AmlType::Invalid => simple_kernel_panic(
                "Acpi/Machine Code",
                "store_argument --- Argument is uninitialized\n",
            ),
            AmlType::UnsignedNumber | AmlType::Number => {
                if let AmlType::UnsignedNumber = val_type {
                    self.method_frames.mut_top().arguments[argument_index as usize] = val.as_u64();
                } else if let AmlType::Number = val_type {
                    self.method_frames.mut_top().arguments[argument_index as usize] = val.as_u64();
                } else {
                    simple_kernel_panic(
                        "Acpi/Machine Code",
                        "store_argument --- Dest/Source Type Mismatch\n",
                    )
                }
            }
            _ => todo!("store_argument --- Package/String"),
        }
    }

    fn store_local(&mut self, local_index: u8, val: &AmlValue, val_type: &AmlType) {
        match self.method_frames.ref_top().local_types[local_index as usize] {
            AmlType::Invalid => simple_kernel_panic(
                "Acpi/Machine Code",
                "store_local --- Local is uninitialized\n",
            ),
            AmlType::UnsignedNumber | AmlType::Number => {
                if let AmlType::UnsignedNumber = val_type {
                    self.method_frames.mut_top().locals[local_index as usize] = val.as_u64();
                } else if let AmlType::Number = val_type {
                    self.method_frames.mut_top().locals[local_index as usize] = val.as_u64();
                } else {
                    simple_kernel_panic(
                        "Acpi/Machine Code",
                        "store_local --- Dest/Source Type Mismatch\n",
                    )
                }
            }
            _ => todo!("store_local --- Package/String"),
        }
    }

    fn unwrap_namestring(&mut self, name_string: &NameString) -> (AmlValue, AmlType) {
        let path;
        let length;
        let absolute;
        if let NameString::Single(val, abs) = name_string {
            path = val as *const u8;
            length = 4;
            absolute = *abs;
            if absolute {
                todo!("unwrap_namestring --- Unimplemented absolute NameString\n")
            }
        } else {
            todo!("unwrap_namestring --- NameString is not a Single value")
        }

        let mut search_path = self.current_executing_path;

        let buf = unsafe { [*path, *(path.add(1)), *(path.add(2)), *(path.add(3))] };

        /* scope/device.path is justified, since the name_string can only contain a single name path*/
        while search_path != 0 {
            let mut scope: Option<&Scope> = Option::None;
            for s in 0..self.scopes.size() {
                if self.scopes.as_ref(s).unwrap().path == search_path {
                    scope = self.scopes.as_ref(s);
                    break;
                }
            }
            match scope {
                None => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "unwrap_namestring: Could not find scope\n",
                ),
                Some(s) => {
                    match s.name_system {
                        Some(ns) => match self.name_system.get_mut(ns, buf) {
                            Some(dro) => {
                                return (
                                    AmlValue::DataRefObject(dro as *mut DataRefObject),
                                    AmlType::DataRefObject,
                                );
                            }
                            None => {}
                        },
                        None => {}
                    }
                    search_path = self.path_system.get_owner_of(s.path);
                }
            }
        }
        search_path = self.current_executing_path;
        while search_path != 0 {
            let mut device: Option<&Device> = Option::None;
            for d in 0..self.devices.size() {
                if self.devices.as_ref(d).unwrap().path == search_path {
                    device = self.devices.as_ref(d);
                    break;
                }
            }
            match device {
                None => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "unwrap_namestring: Could not find scope\n",
                ),
                Some(d) => {
                    match d.name_system {
                        Some(ns) => match self.name_system.get_mut(ns, buf) {
                            Some(dro) => {
                                return (
                                    AmlValue::DataRefObject(dro as *mut DataRefObject),
                                    AmlType::DataRefObject,
                                );
                            }
                            None => {}
                        },
                        None => {}
                    }
                    search_path = self.path_system.get_owner_of(d.path);
                }
            }
        }

        todo!("unwrap_namestring --- CreatedFields have to be implemented")
    }

    fn unwrap_termarg(&mut self, termarg: &TermArg) -> (AmlValue, AmlType) {
        return match termarg {
            TermArg::Arg(a) => {
                let _type = self.method_frames.ref_top().argument_types[*a as usize];
                let val =
                    AmlValue::from_u64(self.method_frames.ref_top().arguments[*a as usize], _type);
                (val, _type)
            }
            TermArg::Local(l) => {
                let _type = self.method_frames.ref_top().local_types[*l as usize];
                let val =
                    AmlValue::from_u64(self.method_frames.ref_top().locals[*l as usize], _type);
                (val, _type)
            }
            TermArg::Byte(b) => (AmlValue::UnsignedNumber(*b as u64), AmlType::UnsignedNumber),
            TermArg::Word(w) => (AmlValue::UnsignedNumber(*w as u64), AmlType::UnsignedNumber),
            TermArg::DWord(dw) => (
                AmlValue::UnsignedNumber(*dw as u64),
                AmlType::UnsignedNumber,
            ),
            TermArg::QWord(qw) => (
                AmlValue::UnsignedNumber(*qw as u64),
                AmlType::UnsignedNumber,
            ),
            TermArg::Zero => (AmlValue::UnsignedNumber(0), AmlType::UnsignedNumber),
            TermArg::One => (AmlValue::UnsignedNumber(1), AmlType::UnsignedNumber),
            TermArg::Ones => (
                AmlValue::UnsignedNumber(0xFFFFFFFFFFFFFFFF),
                AmlType::UnsignedNumber,
            ),
            TermArg::String(base, len) => (AmlValue::String(*base, *len), AmlType::String),
            TermArg::ReturnOp(_) => simple_kernel_panic(
                "Acpi/Machine Code",
                "unwrap_termarg: Invalid Termarg to unwrap\n",
            ),
            TermArg::NameString(name_string) => self.unwrap_namestring(name_string),
        };
    }

    fn unwrap_data_ref_object(&self, data_ref_object: *mut DataRefObject) -> (AmlValue, AmlType) {
        return match unsafe { data_ref_object.as_ref().unwrap() } {
            DataRefObject::One => (AmlValue::UnsignedNumber(1), AmlType::UnsignedNumber),
            DataRefObject::Zero => (AmlValue::UnsignedNumber(0), AmlType::UnsignedNumber),
            DataRefObject::Ones => (
                AmlValue::UnsignedNumber(0xFFFFFFFFFFFFFFFF),
                AmlType::UnsignedNumber,
            ),
            DataRefObject::Package(data, _length, num_elements) => (
                AmlValue::Package(*data, *num_elements as u16),
                AmlType::Package,
            ),
            _ => simple_kernel_panic(
                "Acpi/Machine Code",
                "unwrap_data_ref_object --- unhandled data_ref_object\n",
            ),
        };
    }

    fn store_data_ref_object(
        &self,
        data_ref_object: *mut DataRefObject,
        val: AmlValue,
        val_type: AmlType,
    ) {
        match unsafe { data_ref_object.as_mut().unwrap() } {
            DataRefObject::Byte(b) => match val {
                AmlValue::Number(num) => {
                    if num > 127 || num < -128 {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *b = num as u8;
                }
                AmlValue::UnsignedNumber(num) => {
                    if num > 255 {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *b = num as u8;
                }
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "store_data_ref_object --- Unhandled Source\n",
                ),
            },
            DataRefObject::Word(w) => match val {
                AmlValue::Number(num) => {
                    if num > 32767 || num < -32768 {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *w = num as u16;
                }
                AmlValue::UnsignedNumber(num) => {
                    if num > 0xFFFF {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *w = num as u16;
                }
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "store_data_ref_object --- Unhandled Source\n",
                ),
            },
            DataRefObject::DWord(dw) => match val {
                AmlValue::Number(num) => {
                    if num > 0x7FFFFFFF || num < 0x80000000 {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *dw = num as u32;
                }
                AmlValue::UnsignedNumber(num) => {
                    if num > 0xFFFFFFFF {
                        simple_kernel_panic(
                            "Acpi/Machine Code",
                            "store_data_ref_object --- Bandwidth mismatch\n",
                        )
                    }
                    *dw = num as u32;
                }
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "store_data_ref_object --- Unhandled Source\n",
                ),
            },
            DataRefObject::QWord(qw) => match val {
                AmlValue::Number(num) => {
                    *qw = num as u64;
                }
                AmlValue::UnsignedNumber(num) => {
                    *qw = num;
                }
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "store_data_ref_object --- Unhandled Source\n",
                ),
            },
            DataRefObject::Zero => match val {
                AmlValue::Number(num) => {
                    if num > 0xFFFFFFFF {
                        unsafe { *data_ref_object = DataRefObject::QWord(num as u64) };
                    } else if num > 0xFFFFF {
                        unsafe { *data_ref_object = DataRefObject::DWord(num as u32) };
                    } else if num > 0xFF {
                        unsafe { *data_ref_object = DataRefObject::Word(num as u16) };
                    } else if num > 0 {
                        unsafe { *data_ref_object = DataRefObject::Byte(num as u8) };
                    } else {
                        todo!("store_data_ref_object --- writing Number < 0");
                    }
                }
                AmlValue::UnsignedNumber(num) => {
                    if num > 0xFFFFFFFF {
                        unsafe { *data_ref_object = DataRefObject::QWord(num as u64) };
                    } else if num > 0xFFFFF {
                        unsafe { *data_ref_object = DataRefObject::DWord(num as u32) };
                    } else if num > 0xFF {
                        unsafe { *data_ref_object = DataRefObject::Word(num as u16) };
                    } else if num > 0 {
                        unsafe { *data_ref_object = DataRefObject::Byte(num as u8) };
                    }
                }
                _ => simple_kernel_panic(
                    "Acpi/Machine Code",
                    "store_data_ref_object --- Unhandled Source\n",
                ),
            },
            _ => todo!("store_data_ref_object --- Writing to DataRefObject has to be extended"),
        }
    }
}

impl Reader for AmlCode {
    fn current(&self) -> *const c_void {
        return self.current_code as *const c_void;
    }

    fn peek_u8(&self, offset: u32) -> u8 {
        return unsafe { self.current_code.add(offset as usize).read_unaligned() };
    }

    fn read_u8(&mut self) -> u8 {
        let m = self.current_code;
        self.current_code = unsafe { self.current_code.add(1) };
        self.bytes_rem -= 1;
        return unsafe { m.read_unaligned() };
    }
    fn read_u16(&mut self) -> u16 {
        let m = self.current_code;
        self.current_code = unsafe { self.current_code.add(2) };
        self.bytes_rem -= 2;
        return unsafe { (m as *const u16).read_unaligned() };
    }
    fn read_u32(&mut self) -> u32 {
        let m = self.current_code;
        self.current_code = unsafe { self.current_code.add(4) };
        self.bytes_rem -= 4;
        return unsafe { (m as *const u32).read_unaligned() };
    }
    fn read_u64(&mut self) -> u64 {
        let m = self.current_code;
        self.current_code = unsafe { self.current_code.add(8) };
        self.bytes_rem -= 8;
        return unsafe { (m as *const u64).read_unaligned() };
    }
    fn read_bytes(&mut self, buffer: &mut [u8]) -> bool {
        if self.bytes_rem < buffer.len() as u64 {
            return false;
        }
        let m = self.current_code;
        self.current_code = unsafe { self.current_code.add(buffer.len()) };
        self.bytes_rem -= buffer.len() as u64;
        unsafe {
            memcpy(
                buffer.as_mut_ptr() as *mut c_void,
                m as *const c_void,
                buffer.len() as u32,
            )
        };
        return true;
    }
    fn skip(&mut self, len: u32) -> bool {
        if (len as u64) > self.bytes_rem {
            return false;
        }
        self.bytes_rem -= len as u64;
        self.current_code = unsafe { self.current_code.add(len as usize) };
        return true;
    }
    fn go_back(&mut self, len: u32) -> bool {
        self.bytes_rem += len as u64;
        self.current_code = unsafe { self.current_code.sub(len as usize) };
        return true;
    }
}

impl AmlCode {
    pub fn new(bytes_of_aml: u64, code_begin: u64, allocator: &mut Allocator) -> AmlCode {
        let mut code = AmlCode {
            bytes_rem: bytes_of_aml,
            current_code: code_begin as *const u8,
            module: Module::new("Acpi/Machine Code"),
            path_system: PathSystem::new(allocator),
            name_system: NameSystem::new(allocator),
            field_system: FieldSystem::new(allocator),
            scopes: PageAllocator::new(allocator, 256),
            devices: PageAllocator::new(allocator, 256),
            methods: PageAllocator::new(allocator, 256),
            processors: PageAllocator::new(allocator, 256),
            pending_operation_regions: Stack::new(allocator, 256),
            mutexe: PageAllocator::new(allocator, 256),
            fields: PageAllocator::new(allocator, 256),
            method_frames: Stack::new(allocator, 256),
            name_system_stores: Stack::new(allocator, 256),
            standalone_name_system: 0,
            current_executing_path: 0,
            execution_should_return: false,
            execution_return_data: null(),
        };

        let ns = code.name_system.new_system();
        code.standalone_name_system = ns;

        while code.bytes_rem != 0 {
            code.next_high_instruction();
        }

        for s in 0..code.scopes.size() {
            let scope = code.scopes.as_mut(s).unwrap();
            if scope.path == 1 {
                scope.name_system = Option::Some(code.standalone_name_system);
                break;
            }
        }

        return code;
    }

    pub fn parse_field_element(&mut self) -> FieldElement {
        let ident = self.read_u8();
        return match ident {
            0x0 /* Reserved */=> {
                let package_length = parse_pkglen(self);
                FieldElement::ReservedField(package_length)
            }
            0x1 /* Access */=> {
                let _type = self.read_u8();
                let attrib = self.read_u8();
                FieldElement::AccessField(_type, attrib)
            }
            0x2 /* Connect */=> {
                simple_kernel_panic("Acpi/Machine Code", "parse_field_element: Connect Field encountered\n")
            }
            0x3 /* ExtendedAccess */=> {
                simple_kernel_panic("Acpi/Machine Code", "parse_field_element: ExtendedAccess encountered\n")
            }
            _ /* Named */=> {
                let name = unsafe {[
                    ident,
                    *self.current_code,
                    *self.current_code.add(1),
                    *self.current_code.add(2)
                ]};
                self.advance(3);
                let package_length = parse_pkglen(self);
                FieldElement::NamedField(name, package_length)
            }
        };
    }

    pub fn divide_path(&mut self, name: &NameString) -> (u16, [c_uchar; 4]) {
        let name_buffer: [c_uchar; 4];
        let path;
        match name {
            NameString::Single(name_, absolute) => {
                if *absolute {
                    path = 1; // 1 = Root Path (0x5C)
                } else {
                    path = self.path_system.current_path();
                }
                name_buffer = *name_;
            }
            NameString::Dual(name_, absolute) => {
                // First Segment [0..3] is the path while [4..7] is the method name
                let constructed = unsafe {
                    NameString::Single(
                        [name_.read(), *name_.add(1), *name_.add(2), *name_.add(3)],
                        *absolute,
                    )
                };
                if *absolute {
                    path = self.path_system.insert(&constructed);
                } else {
                    path = self.path_system.append(&constructed);
                }
                unsafe {
                    name_buffer = [*name_.add(4), *name_.add(5), *name_.add(6), *name_.add(7)];
                }
            }
            NameString::Multiple(name_, segments, absolute) => {
                let constructed = NameString::Multiple(*name_, segments - 1, *absolute);
                if *absolute {
                    path = self.path_system.insert(&constructed);
                } else {
                    path = self.path_system.append(&constructed);
                }
                unsafe {
                    name_buffer = [
                        *name_.add((*segments as usize - 1) * 4),
                        *name_.add((*segments as usize - 1) * 4 + 1),
                        *name_.add((*segments as usize - 1) * 4 + 2),
                        *name_.add((*segments as usize - 1) * 4 + 3),
                    ];
                }
            }
        }
        return (path, name_buffer);
    }

    /*
     * Option::None => Scope was not inserted yet
     * Option::Some => Scope with the same path was inserted
     */
    fn get_original_scope(&self, base: u16) -> Option<&Scope> {
        let mut ret = Option::None;
        self.scopes.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };

            if cmp.path == base {
                ret = Option::Some(cmp);
                return false;
            }

            return true;
        });
        return ret;
    }
    pub fn get_original_device(&self, base: u16) -> Option<&Device> {
        let mut ret = Option::None;
        self.devices.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };
            if cmp.path == base {
                ret = Option::Some(cmp);
                return false;
            }
            return true;
        });
        return ret;
    }

    fn get_original_scope_mut(&mut self, base: u16) -> Option<&mut Scope> {
        let size = self.scopes.size();
        for s in 0..size {
            let cmp = self.scopes.as_ref(s).unwrap();
            if cmp.path == base {
                return self.scopes.as_mut(s);
            }
        }
        return Option::None;
    }
    fn get_original_device_mut(&mut self, base: u16) -> Option<&mut Device> {
        let size = self.devices.size();
        for d in 0..size {
            let cmp = self.devices.as_ref(d).unwrap();
            if cmp.path == base {
                return self.devices.as_mut(d);
            }
        }
        return Option::None;
    }

    fn get_original_processor(&self, base: u16) -> Option<&Processor> {
        let mut ret = Option::None;
        self.processors.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };
            if cmp.path == base {
                ret = Option::Some(cmp);
                return false;
            }
            return true;
        });
        return ret;
    }

    //TODO: Test this!
    pub(in crate::aml) fn get_name_entry(&self, name: &NameString) -> Option<&DataRefObject> {
        let root_ptr: *const u8;
        let root_length;
        let name_ptr: *const u8;
        let absolute;
        match name {
            NameString::Single(name_, absolute_) => {
                if *absolute_ {
                    return self.name_system.get(self.standalone_name_system, *name_);
                } else {
                    // Scope, Device, Processor push the current name system onto the stack. And since this is relative, the current name system can be used.
                    let option = *self.name_system_stores.ref_top();
                    let name_system = match unsafe { *option } {
                        Some(ns) => ns,
                        None => simple_kernel_panic(
                            "get_name_entry",
                            "NameString::Single(relative): current name_system is not initialized\n",
                        ),
                    };
                    return self.name_system.get(name_system, *name_);
                }
            }
            NameString::Dual(ptr_, absolute_) => {
                root_ptr = *ptr_;
                root_length = 4;
                name_ptr = unsafe { root_ptr.add(4) };
                absolute = *absolute_;
            }
            NameString::Multiple(ptr_, segments, absolute_) => {
                root_ptr = *ptr_;
                root_length = segments - 1 * 4;
                name_ptr = unsafe { root_ptr.add(root_length as usize) };
                absolute = *absolute_;
            }
        }
        let mut ret = Option::None;
        let path_system;
        if !absolute {
            path_system = self.path_system.current_path();
        } else {
            path_system = 0; // 0 = Root Path
        }
        self.scopes.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };
            if self
                .path_system
                .compare_extern(cmp.path, path_system, root_ptr, root_length as u16)
            {
                if let Option::Some(name_system) = cmp.name_system {
                    let name: [u8; 4] = unsafe {
                        [
                            *name_ptr,
                            *name_ptr.add(1),
                            *name_ptr.add(2),
                            *name_ptr.add(3),
                        ]
                    };
                    if let Option::Some(dro) = self.name_system.get(name_system, name) {
                        ret = Option::Some(dro);
                        return false;
                    }
                }
                return false;
            } else {
                return true;
            }
        });
        if let Option::Some(_) = ret {
            return ret;
        }
        self.devices.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };
            if self
                .path_system
                .compare_extern(cmp.path, path_system, root_ptr, root_length as u16)
            {
                if let Option::Some(name_system) = cmp.name_system {
                    let name: [u8; 4] = unsafe {
                        [
                            *name_ptr,
                            *name_ptr.add(1),
                            *name_ptr.add(2),
                            *name_ptr.add(3),
                        ]
                    };
                    if let Option::Some(dro) = self.name_system.get(name_system, name) {
                        ret = Option::Some(dro);
                        return false;
                    }
                }
                return false;
            } else {
                return true;
            }
        });
        if let Option::Some(_) = ret {
            return ret;
        }
        self.processors.for_each(|_, entry| -> bool {
            let cmp = unsafe { entry.as_ref().unwrap() };
            if self
                .path_system
                .compare_extern(cmp.path, path_system, root_ptr, root_length as u16)
            {
                if let Option::Some(name_system) = cmp.name_system_used {
                    let name: [u8; 4] = unsafe {
                        [
                            *name_ptr,
                            *name_ptr.add(1),
                            *name_ptr.add(2),
                            *name_ptr.add(3),
                        ]
                    };
                    if let Option::Some(dro) = self.name_system.get(name_system, name) {
                        ret = Option::Some(dro);
                        return false;
                    }
                }
                return false;
            } else {
                return true;
            }
        });
        return ret;
    }

    /**
     * Will read as many bytes as buffer can hold
     */
    fn consume_multiple(&mut self, buffer: &mut [u8]) {
        let dst = buffer.as_mut_ptr();
        unsafe {
            memcpy(
                dst as *mut c_void,
                self.current_code as *const c_void,
                buffer.len() as u32,
            );
        }
        self.current_code = unsafe { self.current_code.add(buffer.len()) };
        self.bytes_rem -= buffer.len() as u64;
    }
    fn advance(&mut self, bytes: u16) {
        self.bytes_rem -= bytes as u64;
        self.current_code = unsafe { self.current_code.add(bytes as usize) };
    }
    fn current(&self) -> u8 {
        return unsafe { *self.current_code };
    }
    fn last_addr(&self) -> u64 {
        return unsafe { self.current_code.sub(1) } as u64;
    }

    pub fn next_extended_high_instruction(&mut self) {
        let inst = self.read_u8();
        match inst {
            0x01 /* Mutex*/ => {
                let name = parse_name_string(self);
                let sync_flags = self.read_u8();

                let (path, name_buffer) = self.divide_path(&name);
                self.mutexe.push_back(Mutex::new(path, sync_flags, name_buffer));
            }

            0x80 /* DefOpRegion*/ => {
                let cc = self.current_code;
                let name = parse_name_string(self);

                let (path, name_buffer) = self.divide_path(&name);
                let region = self.read_u8();
                let offset = parse_termarg_int(self);
                let length = parse_termarg_int(self);
                self.pending_operation_regions.push((OperationRegion::new(self.path_system.current_path(), path, name_buffer, length, offset, region), cc))
            }

            0x81 /* Field*/ => {
                let cur: *const u8 = self.current_code;
                let package_length = parse_pkglen(self);
                let name = parse_name_string(self);
                let (path, name) = self.divide_path(&name);
                let flags = self.read_u8();

                let mut original_field: Option<u16> = Option::None;

                self.fields.for_each(|_, entry_| -> bool {
                    let entry = unsafe {entry_.as_ref().unwrap()};
                    if self.path_system.compare(path, entry.path) &&
                        unsafe {memcmp_dword_unaligned(name.as_ptr() as *const u32, entry.name.as_ptr() as *const u32, 1)}
                    {
                        original_field = Option::Some(entry.fs);
                        return false;
                    }
                    return true;
                });

                if let Option::Some(fs) = original_field {
                    self.field_system.set_flags(flags);
                    while self.current_code != unsafe {cur.add(package_length as usize)}{
                        let field_element = self.parse_field_element();
                        self.field_system.add(fs, field_element);
                    }
                }else {
                    let fs = self.field_system.new_system(flags);
                    while self.current_code != unsafe {cur.add(package_length as usize)}{
                        let field_element = self.parse_field_element();
                        self.field_system.add(fs, field_element);
                    }
                    let mut operation_region: Option<OperationRegion> = Option::None;
                    self.pending_operation_regions.for_each(|_, entry| -> bool {
                        let operation_region_ = unsafe {entry.as_ref().unwrap().0};
                        if self.path_system.compare(path, operation_region_.describer_path) &&
                            unsafe {memcmp_dword_unaligned(name.as_ptr() as *const u32, operation_region_.describer_name.as_ptr() as *const u32, 1)}
                        {
                            operation_region = Option::Some(operation_region_.clone());
                            return false;
                        }
                        return true;
                    });

                    self.fields.push_back(Field::new(fs, path, name, match operation_region {
                        Some(op_reg) => {op_reg},
                        None => simple_kernel_panic("Acpi/Machine Code", "Field does not have an operation region\n"),
                    }));
                }
            }

            0x82 /* Device */ => {
                let begin = self.current_code;
                let mut package_length = parse_pkglen(self);
                package_length -= unsafe {self.current_code.offset_from(begin)} as u32;
                let current = self.current_code;
                let name = parse_name_string(self);
                package_length-= unsafe {self.current_code.offset_from(current)} as u32;
                let body_start = self.current_code;
                let body_end = unsafe {body_start.add(package_length as usize)};

                self.path_system.push_frame();
                let path;
                if let NameString::Single(data, absolute) = name {
                    /* If Scope name is not '\'*/
                    if data[3] != 0x5C && data[0] != 0{
                        if !absolute {
                            path = self.path_system.append(&name);
                            self.path_system.push_as_appended(path);
                        }else {
                            path = self.path_system.insert(&name);
                            self.path_system.push_as_inserted(path);
                        }
                    }else {
                        if !name.is_absolute() {
                            path = self.path_system.append(&name);
                            self.path_system.push_as_appended(path);
                        }else {
                            path = self.path_system.insert(&name);
                            self.path_system.push_as_inserted(path);
                        }
                    }
                }else {
                    if !name.is_absolute() {
                        path = self.path_system.append(&name);
                        self.path_system.push_as_appended(path);
                    }else {
                        path = self.path_system.insert(&name);
                        self.path_system.push_as_inserted(path);
                    }
                }

                let mut name_system_used: Option<u16> = Option::None;
                let duplicated;
                if let Option::Some(org_scope) = self.get_original_device(path) {
                    name_system_used = org_scope.name_system;
                    self.name_system_stores.push(&mut name_system_used as *mut Option<u16>);
                    duplicated = true;
                }else {
                    self.name_system_stores.push(&mut name_system_used as *mut Option<u16>);
                    duplicated = false;
                }
                while body_end > self.current_code {
                    self.next_high_instruction();
                }
                while self.current_code >= self.pending_operation_regions.ref_top().1 && self.pending_operation_regions.ref_top().1 >= begin{
                    self.pending_operation_regions.mut_top().1 = null();
                    self.pending_operation_regions.pop_silent();
                }
                if !duplicated {
                    self.devices.push_back(Device::new(path, name_system_used));
                }else {
                    self.get_original_device_mut(path).unwrap().name_system = name_system_used;
                }
                self.name_system_stores.pop_silent();
                self.path_system.pop_frame();
            }
            0x83 /* Processor*/ => {
                let begin = self.current_code;
                let mut package_length = parse_pkglen(self);
                let current = self.current_code;
                package_length-= unsafe {current.offset_from(begin)} as u32;
                let name = parse_name_string(self);
                self.path_system.push_frame();
                let path;
                if !name.is_absolute() {
                    path = self.path_system.append(&name);
                    self.path_system.push_as_appended(path);
                }else {
                    path = self.path_system.insert(&name);
                    self.path_system.push_as_inserted(path);
                }

                let proc_id = self.read_u8();
                let pblk_addr = self.read_u32();
                let pblk_len = self.read_u8();
                package_length-=unsafe{self.current_code.offset_from(current)} as u32;

                let begin_body = self.current_code;
                let end_body = unsafe {begin_body.add(package_length as usize)};

                let mut name_system_used: Option<u16> = Option::None;
                self.name_system_stores.push(&mut name_system_used as *mut Option<u16>);
                while end_body > self.current_code {
                    self.next_high_instruction();
                }

                while self.current_code >= self.pending_operation_regions.ref_top().1 && self.pending_operation_regions.ref_top().1 >= current{
                    self.pending_operation_regions.mut_top().1 = null();
                    self.pending_operation_regions.pop_silent();
                }
                self.processors.push_back(Processor::new(path, name_system_used, proc_id, pblk_addr, pblk_len));
                self.name_system_stores.pop_silent();
                self.path_system.pop_frame();
            }

            _ => {
                let last_addr = self.last_addr();
                error!(
                    &mut self.module,
                    "Invalid Extended High Instruction at 0x{:x}\n", last_addr
                );
                loop {}
            }
        }
    }

    pub fn next_high_instruction(&mut self) {
        let inst = self.read_u8();
        match inst {
            0x10 /* Scope */ => {
                let mut package_length = parse_pkglen(self);
                let current = self.current_code;
                let name = parse_name_string(self);
                package_length-=unsafe {self.current_code.offset_from(current)} as u32;
                self.path_system.push_frame();
                let path;

                if let NameString::Single(data, absolute) = name {
                    /* If Scope name is not '\'*/
                    if data[0] != 0x5C && data[3] != 0{
                        if !absolute {
                            path = self.path_system.append(&name);
                            self.path_system.push_as_appended(path);
                        }else {
                            path = self.path_system.insert(&name);
                            self.path_system.push_as_inserted(path);
                        }
                    }else {
                        if !name.is_absolute() {
                            path = self.path_system.append(&name);
                            self.path_system.push_as_appended(path);
                        }else {
                            path = self.path_system.insert(&name);
                            self.path_system.push_as_inserted(path);
                        }
                    }
                }else {
                    if !name.is_absolute() {
                        path = self.path_system.append(&name);
                        self.path_system.push_as_appended(path);
                    }else {
                        path = self.path_system.insert(&name);
                        self.path_system.push_as_inserted(path);
                    }
                }

                let mut name_system_used: Option<u16> = Option::None;
                let duplicated;
                if let Option::Some(org_scope) = self.get_original_scope(path){
                    name_system_used = org_scope.name_system;
                    self.name_system_stores.push(&mut name_system_used as *mut Option<u16>);
                    duplicated = true;
                }else {
                    self.name_system_stores.push(&mut name_system_used as *mut Option<u16>);
                    duplicated = false;
                }
                while(unsafe {current.add(package_length as usize)} > self.current_code) {
                    self.next_high_instruction();
                }
                while self.current_code >= self.pending_operation_regions.ref_top().1 && self.pending_operation_regions.ref_top().1 >= current{
                    self.pending_operation_regions.mut_top().1 = null();
                    self.pending_operation_regions.pop_silent();
                }
                if !duplicated {
                    self.scopes.push_back(Scope::new(path, name_system_used));
                }else {
                    self.get_original_scope_mut(path).unwrap().name_system = name_system_used;
                }
                self.name_system_stores.pop_silent();
                self.path_system.pop_frame();
            },

            0x14 /* Method */ => {
                let base = self.current_code;
                let mut package_length = parse_pkglen(self);
                let end = unsafe {base.add(package_length as usize)};
                let cur = self.current_code;
                let name_string = parse_name_string(self);

                let (path, name) = self.divide_path(&name_string);


                let raw_flags = self.read_u8();
                package_length -= unsafe {self.current_code.offset_from(cur)} as u32;
                let flags = raw_flags&0b1111;
                let sync_level = raw_flags>>5;

                self.methods.push_back(Method::new(path, name, flags, sync_level, package_length as u16 - 1, self.current_code as *const c_void));
                self.advance(unsafe {end.offset_from(self.current_code)} as u16);
            }

            0x8 /* Name */ => {
                let name_system;
                if self.name_system_stores.num_occupied() != 0 {
                    let option = *self.name_system_stores.ref_top();
                    if let Option::Some(name_system_) = unsafe {*option} {
                        name_system = name_system_;
                    }else {
                        name_system = self.name_system.new_system();
                        unsafe {*option = Option::Some(name_system)};
                    }
                }else {
                    name_system = self.standalone_name_system;
                }
                let name = parse_name_string(self);

                if let NameString::Single(_, _) = name {
                    let mut tmp_reader = BufferedReader::new(self.current_code as *const c_void, self.bytes_rem as u32);
                    let data_ref_object = parse_data_ref_object(self, &mut tmp_reader);
                    self.current_code = tmp_reader.current() as *const u8;
                    self.bytes_rem = tmp_reader.remaining_bytes() as u64;
                    self.name_system.add(name_system, &name, &data_ref_object);
                }else {
                    simple_kernel_panic("Acpi/Machine Code", "Parsing Name with multiple segments\n");
                }

            }

            0x5B => self.next_extended_high_instruction(),

            _ => {
                let last_addr = self.last_addr();
                error!(
                    &mut self.module,
                    "Invalid High Instruction at 0x{:x}\n",
                    last_addr
                );
                loop {}
            }
        }
    }
}
