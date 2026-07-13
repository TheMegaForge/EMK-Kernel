use core::{ffi::c_void, ptr::null_mut, slice};

pub mod loader;
pub mod mark_table;
pub mod pe;
use crate::{
    aml::AmlType::Package,
    arch::lapic::TRIGGER_MODE_LEVEL,
    fixed_handle_basis::FIXED_FS_HANDLE_BASE,
    fixed_vaddrs::{FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS, GFS_FILES_FIXED_VADDR},
    hal::memory::allocator::{Allocator, MemoryBlock, VirtualAllocator},
    processes::{
        loader::{ExecutableImage, ExecutableLoadError, load_executable_file},
        mark_table::MarkTable,
    },
    utils::{buffer::Buffer, memory::memcpy_qword, slices::invalid_mut_slice},
    vfs::gfs::{GFS, GFS_PERMISSIONS_EXECUTE, GfsResult, file::GfsFile},
};

pub enum ExecutionMode {
    Kernel = 0,
    Process = 3,
}
#[repr(align(8))]
pub struct HandleBuffer<'a> {
    memory: MemoryBlock,
    handles: &'a mut [u64],
    current: u32,
}
#[repr(align(256))]
struct ApplicationRegisters {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rbp: u64,
    rsp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    cr3: u64,
    rflags: u64,
}

/*
 * Saved registers are: gpr, cr3, rflags and rip
 */
pub struct Application<'a> {
    fs_handle_buffer: HandleBuffer<'a>,
    registers: *mut ApplicationRegisters,
    fs_handle_base: u32,
    execution_fi: u32,
    image: ExecutableImage<'static>,
}

impl<'a> Application<'a> {
    pub fn default() -> Self {
        return Self {
            fs_handle_buffer: HandleBuffer {
                memory: MemoryBlock::empty(),
                handles: invalid_mut_slice(),
                current: 0,
            },
            registers: null_mut(),
            fs_handle_base: FIXED_FS_HANDLE_BASE,
            execution_fi: 0,
            image: ExecutableImage::empty(),
        };
    }

    pub fn spawn(&mut self, execution_fi: u32, virtual_allocator: &mut Allocator) -> bool {
        let mb = match virtual_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => return false,
        };

        self.execution_fi = execution_fi;
        self.fs_handle_buffer = HandleBuffer {
            memory: mb,
            handles: unsafe { slice::from_raw_parts_mut(mb.as_mut_ptr(), mb.length as usize / 8) },
            current: 0,
        };
        return true;
    }

    pub fn release(&mut self, allocator: &mut Allocator) -> bool {
        match allocator.free(&self.fs_handle_buffer.memory) {
            Ok(_) => {
                self.fs_handle_buffer.handles = invalid_mut_slice();
                return true;
            }
            Err(_e) => return false,
        }
    }

    pub fn reallocate_handles(&mut self, allocator: &mut Allocator) {
        let mb = allocator
            .alloc_zero((self.fs_handle_buffer.memory.length / 0x1000) as u16 * 2)
            .unwrap();
        unsafe {
            memcpy_qword(
                mb.as_mut_ptr(),
                self.fs_handle_buffer.memory.as_ptr(),
                self.fs_handle_buffer.handles.len() as u32,
            )
        };
        self.fs_handle_buffer.handles =
            unsafe { slice::from_raw_parts_mut(mb.as_mut_ptr(), mb.length as usize / 8) };
        allocator.free(&self.fs_handle_buffer.memory).unwrap();
        self.fs_handle_buffer.memory = mb;
    }

    /** Option::None => call reallocate_handles*/
    pub fn allocate_handle(&mut self, ftab_index: u32) -> Option<u64> {
        if self.fs_handle_buffer.current + 1 > self.fs_handle_buffer.handles.len() as u32 {
            return Option::None;
        }
        self.fs_handle_buffer.handles[self.fs_handle_buffer.current as usize] = ftab_index as u64;
        let ret = self.fs_handle_buffer.current;
        self.fs_handle_buffer.current += 1;
        return Option::Some(ret as u64 + self.fs_handle_base as u64);
    }

    pub fn load_image(
        &mut self,
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
        buffer: &Buffer,
        scheduler: u8,
    ) -> Option<ExecutableLoadError> {
        match load_executable_file(
            buffer,
            physical_allocator,
            kernel_allocator,
            virtual_allocator,
            scheduler,
        ) {
            Ok(img) => {
                let registers = unsafe { self.registers.as_mut().unwrap() };
                registers.rsp = img.rbp_default;
                registers.rbp = img.rbp_default;
                registers.r15 = 0;
                registers.r14 = 0;
                registers.r13 = 0;
                registers.r12 = 0;
                registers.r11 = 0;
                registers.r10 = 0;
                registers.r9 = 0;
                registers.r8 = 0;
                registers.rdx = 0;
                registers.rcx = 0;
                registers.rbx = 0;
                registers.rax = 0;
                registers.rip = img.entry_point;
                registers.cr3 = img.cr3;
                self.image = img.extract();
            }
            Err(e) => return Option::Some(e),
        }
        return Option::None;
    }
    /* returned function has to be called*/
    pub fn launch(&self) -> impl FnOnce() {
        let registers = unsafe { &*self.registers };
        return move || unsafe {
            launch_application(registers.cr3, registers.rbp, registers.rip);
        };
    }

    pub fn unload_image(&mut self) {
        todo!("Free Memory & reset");
    }
}

unsafe extern "C" {
    /* loads cr3 and transitions to userspace*/
    pub(in crate::processes) fn launch_application(cr3: u64, rbp: u64, rip: u64);
}

/* PER CORE
 * Each Application has 256 bytes of registers space
 */
pub struct ApplicationPool {
    memory: [MemoryBlock; 2], // maximum number of applications is limited to 24
    applications: &'static mut [Application<'static>],
    mark_table: MarkTable,
    applications_present: u32,
}

impl ApplicationPool {
    pub fn allocate(physical_memory: &mut Allocator, virtual_allocator: &mut Allocator) -> Self {
        let register_space = virtual_allocator.alloc_zero(2).unwrap();
        let applications_mb = virtual_allocator.alloc_zero(1).unwrap();
        let applications = unsafe { slice::from_raw_parts_mut(applications_mb.as_mut_ptr(), 24) };
        for i in 0..applications.len() {
            let application: &mut Application = &mut applications[i];
            *application = Application::default();
            application.registers =
                (register_space.base + i as u64 * 256) as *mut ApplicationRegisters;
        }
        return Self {
            applications,
            memory: [register_space, applications_mb],
            mark_table: MarkTable::new(physical_memory),
            applications_present: 0,
        };
    }
}

pub enum ApplicationComplexResult {
    InvalidState,
    WrongPermissions,
    FTabAllocationFailed,
    MaxNumAppsExeceeded,
    SpawnFailed,
    FindingError(GfsResult),
}

pub struct ApplicationComplex {
    application_pool: ApplicationPool,
    scheduler: u8,
}

impl ApplicationComplex {
    pub fn allocate(
        scheduler: u8,
        physical_allocator: &mut Allocator,
        virtual_allocator: &mut Allocator,
    ) -> Self {
        return Self {
            application_pool: ApplicationPool::allocate(physical_allocator, virtual_allocator),
            scheduler,
        };
    }
    #[allow(static_mut_refs)]
    pub fn summon_application(
        &mut self,
        file_path: &str,
        virtual_allocator: &mut VirtualAllocator,
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
    ) -> Result<impl FnOnce(), ApplicationComplexResult> {
        let file;

        file = match unsafe { GFS.get_file_mut(file_path) } {
            Ok(file) => file,
            Err(e) => return Result::Err(ApplicationComplexResult::FindingError(e)),
        };

        if file.get_write_count() != 0 {
            /* A executing application is only allowed to be read from, when starting the execution*/
            return Result::Err(ApplicationComplexResult::InvalidState);
        }

        if file.get_permissions() & GFS_PERMISSIONS_EXECUTE == 0 {
            return Result::Err(ApplicationComplexResult::WrongPermissions);
        }
        let fi = unsafe {
            (file as *const GfsFile).offset_from(GFS_FILES_FIXED_VADDR as *const GfsFile)
        } as u32;

        if self.application_pool.applications_present + 1
            > self.application_pool.applications.len() as u32
        {
            return Result::Err(ApplicationComplexResult::MaxNumAppsExeceeded);
        }

        let app = &mut self.application_pool.applications
            [self.application_pool.applications_present as usize];

        if !app.spawn(fi as u32, &mut virtual_allocator.allocator) {
            return Result::Err(ApplicationComplexResult::SpawnFailed);
        }
        if !self.application_pool.mark_table.contains(fi) {
            self.application_pool.mark_table.insert(fi);
        } else {
            self.application_pool.mark_table.increase(fi);
        }
        let (buffer, release_buffer) =
            file.read(physical_allocator, unsafe { &GFS }, self.scheduler);
        app.load_image(
            physical_allocator,
            kernel_allocator,
            virtual_allocator,
            &buffer,
            self.scheduler,
        );
        let launch_fn = app.launch();
        self.application_pool.applications_present += 1;
        return Result::Ok(launch_fn);
    }
}

#[repr(u8)]
pub enum IpiRequestType {
    Invalid,
    SyncVMem,
    SummonApplication,
}
#[repr(u8)]
pub enum IpiStatus {
    Invalid = 0,
    Pending = 1,
    Recieved = 2,
    Completed = 3,
    Failed = 4,
}
#[derive(Clone, Copy)]
pub struct IpiVMemSync {
    pub paddr: MemoryBlock,
    pub vaddr: u64,
}

impl IpiVMemSync {
    pub fn new(paddr: MemoryBlock, vaddr: u64) -> Self {
        return Self { paddr, vaddr };
    }
}

pub union IpiRequestData {
    pub application_to_summon: &'static str,
    pub vmem_sync: IpiVMemSync,
}

pub struct IpiDescriptorPacket {
    pub request_type: IpiRequestType,
    pub status: IpiStatus,
    pub request_data: IpiRequestData,
}
