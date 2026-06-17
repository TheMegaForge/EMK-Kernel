pub mod gfs;
/*
/* TODO: implement file table*/
pub struct FileTableEntry {
    pub file_pointer: &'static mut GfsFile,
    pub occupation_map: u32,
    pub read_count: u16,
    pub write_count: u16,
}

static mut FILE_TABLE: &mut [FileTableEntry] = invalid_mut_slice();
pub fn initialize_file_table(allocator: &mut Allocator, threading: &Multithreading) {
    let mb = allocator.alloc_zero(8).unwrap();
    let pager = unsafe {
        (*(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor))
            .pager
            .as_mut()
            .unwrap()
    };
    for i in 0..8 {
        pager
            .page_4_kb(
                FIXED_FILE_TABLE_VIRTUAL_ADDRESS + i * 0x1000,
                mb.base + i * 0x1000,
                PAGER_RW | PAGER_PRESENT,
                allocator,
            )
            .unwrap();
    }
    threading.foreach_lapic(|lapic| {
        if lapic.get_id() as u32 != LocalApic::from_local_core().get_id() {
            threading.send_user_ipi(lapic.get_id(), |packet| {
                packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                packet.status = crate::processes::IpiStatus::Pending;
                packet.request_data.vmem_sync =
                    crate::processes::IpiVMemSync::new(mb, FIXED_FILE_TABLE_VIRTUAL_ADDRESS);
            });
            if !threading.await_ipi(lapic.get_id()) {
                simple_kernel_panic("initialize_file_table", "IPI got stuck\n")
            }
        }
    });
    unsafe {
        FILE_TABLE = unsafe {
            slice::from_raw_parts_mut(
                FIXED_FILE_TABLE_VIRTUAL_ADDRESS as *mut FileTableEntry,
                0x8000 / size_of::<FileTableEntry>(),
            )
        }
    };
}

pub fn file_table_allocate() -> Option<&mut FileTableEntry> {}
*/
