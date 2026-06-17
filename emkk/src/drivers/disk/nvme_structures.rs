use crate::{
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
    utils::traits::Region,
};

#[repr(C)]
pub struct NVMeCompletionEntry {
    cdw0_: u32,
    reserved: u32,
    sqhd: u16,
    sqid: u16,
    cid: u16,
    p_sf: u16,
}

impl NVMeCompletionEntry {
    pub fn cdw0(&self) -> u32 {
        return self.cdw0_;
    }
    pub fn phase(&self) -> bool {
        return 1 == (self.p_sf & 1);
    }
    pub fn status_field(&self) -> u16 {
        return self.p_sf >> 1;
    }

    pub fn clear_phase(&mut self) {
        self.p_sf = self.p_sf & !1;
    }
}

#[repr(C)]
pub union NVMeDataPointer {
    prps: [u64; 2],
    sgl_entry: [u64; 2],
}

impl NVMeDataPointer {
    pub fn prps(prp1: u64, prp2: u64) -> Self {
        return Self { prps: [prp1, prp2] };
    }
}

#[repr(C)]
pub struct NVMeSubmissionEntry {
    pub cdw0: u32,
    pub nsid: u32,
    reserved: u64,
    pub mptr: u64,
    pub dptr: NVMeDataPointer,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}
pub const NVME_OPC_IDENTIFY: u32 = 0x06;
pub const NVME_OPC_CREATE_IO_COMPLETION_QUEUE: u32 = 0x05;
pub const NVME_OPC_CREATE_IO_SUBMISSION_QUEUE: u32 = 0x01;
pub const NVME_OPC_READ: u32 = 0x2;
pub const NVME_OPC_WRITE: u32 = 0x1;
pub trait NVMeCommand {
    fn write(&self, submission_entry: &mut NVMeSubmissionEntry, cid: u16);
}

pub struct NVMeIdentifyCommand {
    nsid: u32,
    data_buffer: u64,

    cns: u8,
    cntid: u16,
    nvmesetid: u16,
}

impl NVMeIdentifyCommand {
    pub fn identify_controller(allocator: &mut Allocator) -> Self {
        return Self {
            nsid: 0xFFFFFFFF,
            data_buffer: match allocator.alloc_zero(1) {
                Ok(mb) => mb.base,
                Err(_e) => simple_kernel_panic(
                    "NVMeIdentifyCommand",
                    "Could not allocate data buffer for controller identification\n",
                ),
            },
            cns: 1,
            cntid: 0,
            nvmesetid: 0,
        };
    }
    pub fn active_nsid_list(allocator: &mut Allocator) -> Self {
        return Self {
            nsid: 0,
            data_buffer: match allocator.alloc_zero(1) {
                Ok(mb) => mb.base,
                Err(_e) => simple_kernel_panic(
                    "NVMeIdentifyCommand",
                    "Could not allocate data buffer for controller identification\n",
                ),
            },
            cns: 2,
            cntid: 0,
            nvmesetid: 0,
        };
    }
    pub fn identify_namespace_data_structures(allocator: &mut Allocator, nsid: u32) -> Self {
        return Self {
            nsid,
            data_buffer: match allocator.alloc_zero(1) {
                Ok(mb) => mb.base,
                Err(_e) => simple_kernel_panic(
                    "NVMeIdentifyCommand",
                    "Could not allocate data buffer for controller identification\n",
                ),
            },
            cns: 0,
            cntid: 0,
            nvmesetid: 0,
        };
    }

    pub fn data(&self) -> u64 {
        return self.data_buffer;
    }

    pub fn release(&mut self, allocator: &mut Allocator) {
        self.cns = 0;
        self.cntid = 0;
        self.nsid = 0xFFFFFFFF;
        self.nvmesetid = 0;

        if self.data_buffer != 0 {
            allocator
                .free(&MemoryBlock::new(0x1000, self.data_buffer))
                .unwrap();
            self.data_buffer = 0;
        }
    }
}

pub struct NVMeCreationCommand {
    opc: u8,
    qid: u16,
    cqid: u16,
    queue_size: u16,
    iv: u16,
    queue: u64,
}

impl NVMeCreationCommand {
    pub fn create_io_completion_queue(qid: u16, queue_size: u16, iv: u16, queue: u64) -> Self {
        return Self {
            opc: NVME_OPC_CREATE_IO_COMPLETION_QUEUE as u8,
            qid,
            queue_size,
            iv,
            cqid: 0,
            queue,
        };
    }
    pub fn create_io_submission_queue(
        qid: u16,
        cqid: u16,
        queue_size: u16,
        iv: u16,
        queue: u64,
    ) -> Self {
        return Self {
            opc: NVME_OPC_CREATE_IO_SUBMISSION_QUEUE as u8,
            qid,
            cqid,
            queue_size,
            iv,
            queue,
        };
    }
}

impl NVMeCommand for NVMeCreationCommand {
    fn write(&self, submission_entry: &mut NVMeSubmissionEntry, cid: u16) {
        submission_entry.cdw0 = self.opc as u32 | (cid as u32) << 16;
        submission_entry.nsid = 0xFFFFFFFF;
        submission_entry.mptr = 0;
        submission_entry.dptr.prps = [self.queue, 0];
        submission_entry.cdw10 = self.qid as u32 | (self.queue_size as u32) << 16;
        if self.opc == NVME_OPC_CREATE_IO_COMPLETION_QUEUE as u8 {
            submission_entry.cdw11 = 1 | 1 << 1 | (self.iv as u32) << 16;
            submission_entry.cdw12 = 0;
            submission_entry.cdw13 = 0;
            submission_entry.cdw14 = 0;
            submission_entry.cdw15 = 0;
        } else {
            submission_entry.cdw11 = 1 | 0b01 << 1 | (self.cqid as u32) << 16;
            submission_entry.cdw12 = 0;
            submission_entry.cdw13 = 0;
            submission_entry.cdw14 = 0;
            submission_entry.cdw15 = 0;
        }
    }
}

impl NVMeCommand for NVMeIdentifyCommand {
    fn write(&self, submission_entry: &mut NVMeSubmissionEntry, cid: u16) {
        submission_entry.cdw0 = NVME_OPC_IDENTIFY | (cid as u32) << 16;
        submission_entry.nsid = self.nsid;
        submission_entry.mptr = 0;
        submission_entry.dptr.prps = [self.data_buffer, 0];
        submission_entry.cdw10 = self.cns as u32 | (self.cntid as u32) << 16;
        submission_entry.cdw11 = self.nvmesetid as u32;
        submission_entry.cdw12 = 0;
        submission_entry.cdw13 = 0;
        submission_entry.cdw14 = 0; /* UUID = 0*/
        submission_entry.cdw15 = 0;
    }
}
