use core::slice;

use crate::drivers::disk::nvme_structures::{
    NVME_OPC_READ, NVME_OPC_WRITE, NVMeCommand, NVMeCompletionEntry, NVMeDataPointer,
    NVMeSubmissionEntry,
};

pub struct NVMeQueue<'a> {
    completion_base: &'a mut [NVMeCompletionEntry],
    submission_base: &'a mut [NVMeSubmissionEntry],
    head: &'static mut u32,
    tail: &'static mut u32,
    size: u32,
    cid_mask: u16,
    current_cid: u16,
    head_value: u32,
    tail_value: u32,
}

pub const NVME_QUEUE_MAX_CID: u16 = 0x7FF;

impl<'a> NVMeQueue<'a> {
    pub fn new(
        completion_base: u64,
        submission_base: u64,
        head: u64,
        tail: u64,
        size: u16,
        cid_mask: u16,
    ) -> Self {
        return Self {
            completion_base: unsafe {
                slice::from_raw_parts_mut(
                    completion_base as *mut NVMeCompletionEntry,
                    size as usize,
                )
            },
            submission_base: unsafe {
                slice::from_raw_parts_mut(
                    submission_base as *mut NVMeSubmissionEntry,
                    size as usize,
                )
            },
            head: unsafe { (head as *mut u32).as_mut().unwrap() },
            tail: unsafe { (tail as *mut u32).as_mut().unwrap() },
            size: size as u32,
            cid_mask,
            current_cid: 0,
            head_value: 0,
            tail_value: 0,
        };
    }

    fn current_submit(&mut self) -> &mut NVMeSubmissionEntry {
        return &mut self.submission_base[self.tail_value as usize];
    }

    fn current_completion(&mut self) -> &mut NVMeCompletionEntry {
        return &mut self.completion_base[self.head_value as usize];
    }

    fn advance_submit(&mut self, submits: u32) {
        self.tail_value = (self.tail_value + submits) % (self.size - 1);
        *self.tail = self.tail_value;
    }

    fn advance_completion(&mut self, completions: u32) {
        self.head_value = (self.head_value + completions) % (self.size - 1);
        *self.head = self.head_value;
    }

    pub fn issue_general(&mut self, command: &dyn NVMeCommand) {
        let cid = self.cid_mask | self.current_cid;
        self.current_cid = (self.current_cid + 1) % NVME_QUEUE_MAX_CID;

        let current_submit = self.current_submit();
        command.write(current_submit, cid);
        self.advance_submit(1);
    }

    /*
     * num_logical_blocks is 0th based.
     *  => 0 = 1 block
     *  => 1 = 2 blocks
     *  => etc...
     */

    pub fn issue_read(
        &mut self,
        num_logical_blocks: u16,
        lba: u64,
        prp1: u64,
        prp2: u64,
        nsid: u32,
    ) {
        let cid = self.cid_mask | self.current_cid;
        self.current_cid = (self.current_cid + 1) % NVME_QUEUE_MAX_CID;

        let current_submit = self.current_submit();
        current_submit.cdw0 = NVME_OPC_READ | (cid as u32) << 16;
        current_submit.nsid = nsid;
        current_submit.mptr = 0;
        current_submit.dptr = NVMeDataPointer::prps(prp1, prp2);
        current_submit.cdw10 = (lba & 0xFFFFFFFF) as u32;
        current_submit.cdw11 = (lba >> 32) as u32;
        current_submit.cdw12 = num_logical_blocks as u32 | 1 << 31; // 1 << 31 = limited retry
        current_submit.cdw13 = 0; // no latency information ...
        current_submit.cdw14 = 0;
        current_submit.cdw15 = 0;
        self.advance_submit(1);
    }
    pub fn issue_write(
        &mut self,
        num_logical_blocks: u16,
        lba: u64,
        prp1: u64,
        prp2: u64,
        nsid: u32,
    ) {
        let cid = self.cid_mask | self.current_cid;
        self.current_cid = (self.current_cid + 1) % NVME_QUEUE_MAX_CID;

        let current_submit = self.current_submit();
        current_submit.cdw0 = NVME_OPC_WRITE | (cid as u32) << 16;
        current_submit.mptr = 0;
        current_submit.nsid = nsid;
        current_submit.dptr = NVMeDataPointer::prps(prp1, prp2);
        current_submit.cdw10 = (lba & 0xFFFFFFFF) as u32;
        current_submit.cdw11 = (lba >> 32) as u32;
        current_submit.cdw12 = num_logical_blocks as u32 | 1 << 31; // 1 << 31 = limited retry
        current_submit.cdw13 = 0; // no latency information ...
        current_submit.cdw14 = 0;
        current_submit.cdw15 = 0;
        self.advance_submit(1);
    }

    pub fn unissue(&mut self) -> Result<u32, u16> {
        let ret;
        {
            let completion = self.current_completion();
            if completion.status_field() & 0b1111_1111_111 == 0 {
                return Result::Err(completion.status_field());
            }
            ret = completion.cdw0();
            completion.clear_phase();
        }
        self.advance_completion(1);
        return Result::Ok(ret);
    }

    pub fn unissue_silent(&'a mut self) -> u16 {
        let comp = self.current_completion();
        let ret;
        if comp.status_field() & 0b1111_1111_111 == 0 {
            ret = 0;
            self.current_completion().clear_phase();
        } else {
            ret = comp.status_field();
        }
        self.advance_completion(1);
        return ret;
    }
}
