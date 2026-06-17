use core::{ffi::c_void, ptr::null_mut, slice};

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic},
    drivers::disk::{
        ControllerType, Disk, DiskController, DiskIOResult,
        nvme_queue::NVMeQueue,
        nvme_registers::NVMeBar,
        nvme_structures::{
            NVMeCommand, NVMeCompletionEntry, NVMeCreationCommand, NVMeIdentifyCommand,
            NVMeSubmissionEntry,
        },
    },
    fixed_vaddrs::{EHCI_BAR_FIXED_VADDR, NVME_BAR_FIXED_VADDR, ref_processor_mut},
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        pci_bus::PciBarIndex,
        print::{Module, simple_kernel_panic},
    },
    info,
    multithreading::{Multithreading, processors::Processor},
    success,
    time::sleep,
    utils::traits::Region,
};

pub struct NVMeDisk {
    nsid: u32,
    identifier_: u32,
    num_sectors_: u64,
    sector_size_: u32,
}

pub const POINTERS_ON_FIRST_PRP_LIST: u64 = 4096 / 4;
pub const POINTERS_ON_CONTINUED_PRP_LIST: u64 = (4096 / 4) - 1;
/* INFO: This could be improved!*/
impl Disk for NVMeDisk {
    fn identifier(&self) -> u32 {
        return self.identifier_;
    }
    fn num_sectors(&self) -> u64 {
        return self.num_sectors_;
    }
    fn write_from_buffer(
        &self,
        lba: u64,
        buffer: &crate::utils::buffer::Buffer,
        scheduler: u8,
    ) -> DiskIOResult {
        let nvme_controller = unsafe { NVME_CONTROLLER.as_mut().unwrap() };
        if scheduler > nvme_controller.num_queues_implemented - 1 || scheduler == 0 {
            return DiskIOResult::InvalidScheduler;
        }

        let mut sectors_to_be_written = buffer.get_size() / self.sector_size_ as u64;
        if buffer.get_size() % self.sector_size_ as u64 != 0 {
            sectors_to_be_written += 1;
        }

        if lba + sectors_to_be_written > self.num_sectors_ {
            return DiskIOResult::InvalidLba;
        }

        let mut bytes_to_be_written = sectors_to_be_written * self.sector_size_ as u64;
        let mut address = buffer.address();
        let mut current_lba = lba;

        while bytes_to_be_written > 0 {
            let fetched_size;
            if bytes_to_be_written > nvme_controller.bulk_size(self.sector_size_) {
                fetched_size = nvme_controller.bulk_size(self.sector_size_);
            } else {
                fetched_size = bytes_to_be_written;
            }

            let (prps, mb) = nvme_controller.construct_prp(fetched_size, address);
            let mut num_logical_blocks = (fetched_size / self.sector_size_ as u64) as u16;
            if (fetched_size % self.sector_size_ as u64) != 0 {
                num_logical_blocks += 1;
            }
            nvme_controller.dispatch_write(
                scheduler,
                num_logical_blocks - 1,
                current_lba,
                prps,
                self.nsid,
            );

            if let Option::Some(mb_) = mb {
                nvme_controller.allocator.free(&mb_).unwrap();
            }

            address += fetched_size;
            bytes_to_be_written -= fetched_size;
            current_lba += num_logical_blocks as u64;
        }
        return DiskIOResult::Success;
    }

    fn read_into_buffer(
        &self,
        lba: u64,
        buffer: &crate::utils::buffer::Buffer,
        scheduler: u8,
    ) -> DiskIOResult {
        let nvme_controller = unsafe { NVME_CONTROLLER.as_mut().unwrap() };
        if scheduler > nvme_controller.num_queues_implemented - 1 || scheduler == 0 {
            return DiskIOResult::InvalidScheduler;
        }
        if buffer.is_readonly() {
            return DiskIOResult::BufferReadonly;
        }

        let mut sectors_to_be_read = buffer.get_size() / self.sector_size_ as u64;
        if buffer.get_size() % self.sector_size_ as u64 != 0 {
            sectors_to_be_read += 1;
        }

        if lba + sectors_to_be_read > self.num_sectors_ {
            return DiskIOResult::InvalidLba;
        }

        let mut bytes_to_be_read = sectors_to_be_read * self.sector_size_ as u64;
        let mut address = buffer.address();
        let mut current_lba = lba;
        while bytes_to_be_read > 0 {
            let fetched_size;
            if bytes_to_be_read > nvme_controller.bulk_size(self.sector_size_) {
                fetched_size = nvme_controller.bulk_size(self.sector_size_);
            } else {
                fetched_size = bytes_to_be_read;
            }

            let (prps, mb) = nvme_controller.construct_prp(fetched_size, address);
            let mut num_logical_blocks = (fetched_size / self.sector_size_ as u64) as u16;
            if (fetched_size % self.sector_size_ as u64) != 0 {
                num_logical_blocks += 1;
            }
            nvme_controller.dispatch_read(
                scheduler,
                num_logical_blocks - 1,
                current_lba,
                prps,
                self.nsid,
            );
            if let Option::Some(mb_) = mb {
                nvme_controller.allocator.free(&mb_).unwrap();
            }
            address += fetched_size;
            bytes_to_be_read -= fetched_size;
            current_lba += num_logical_blocks as u64;
        }
        return DiskIOResult::Success;
    }

    fn sector_size(&self) -> u32 {
        return self.sector_size_;
    }
}

pub struct NVMeController {
    present_: bool,
    num_disks_present: u8,
    abar: NVMeBar,
    admin_queue: NVMeQueue<'static>,
    check_queue: u8,
    num_outstanding_completions: [u8; 8],
    num_queues_implemented: u8,
    controller_information: u64,
    disks: *mut NVMeDisk,
    io_queues: *mut NVMeQueue<'static>,
    maximum_data_transfer_size: u64,
    allocator: Allocator,
}

impl NVMeController {
    fn bulk_size(&self, sector_size: u32) -> u64 {
        if self.maximum_data_transfer_size == 0xFFFF {
            return self.maximum_data_transfer_size * sector_size as u64;
        } else {
            return self.maximum_data_transfer_size;
        }
    }

    fn construct_prp(&mut self, bytes: u64, address: u64) -> ([u64; 2], Option<MemoryBlock>) {
        let mut aligned_address = address & !0xFFF;
        let offset = address & 0xFFF;
        let mut ret = [0; 2];
        ret[0] = address;

        let took_size = 0x1000 - offset;
        /*
         * size > took_size => more bytes needed
         */
        if bytes > took_size {
            /*
             * more than 1 pages in total have to be read, but not more than 2 pages
             */
            if 0x1000 >= bytes - took_size {
                ret[1] = aligned_address + 0x1000;
            } else {
                /* More than 2 pages have to be read*/
                let mut p = (bytes - took_size) / 0x1000;
                if (bytes - took_size) % 0x1000 != 0 {
                    p += 1;
                }
                let depth;
                let n;
                if p > 512 {
                    /* Trust Me, just trust me please*/
                    if p == 511 * (p - 511) / 511 + 512 {
                        depth = (p - 511) / 511;
                        n = 512;
                    } else {
                        depth = p / 511;
                        n = p - (p / 511) * 511;
                    }
                    let tables = match self.allocator.alloc((depth) as u16) {
                        Ok(mb) => mb,
                        Err(_e) => simple_kernel_panic("NVMe", "Could not allocate PRP List\n"),
                    };
                    let mut base_adress = tables.get_base();
                    ret[1] = base_adress;
                    aligned_address += 0x1000;
                    let mut array =
                        unsafe { slice::from_raw_parts_mut(tables.as_mut_ptr() as *mut u64, 512) };
                    for _ in 0..depth {
                        for j in 0..511 {
                            array[j] = aligned_address;
                            aligned_address += 0x1000;
                        }
                        array[511] = base_adress + 0x1000;
                        base_adress += 0x1000;
                        array = unsafe { slice::from_raw_parts_mut(base_adress as *mut u64, 512) };
                    }
                    for _ in 0..n {
                        array[n as usize] = aligned_address;
                        aligned_address += 0x1000;
                    }
                    return (ret, Option::Some(tables));
                } else {
                    aligned_address += 0x1000;
                    let table = match self.allocator.alloc_zero(1) {
                        Ok(mb) => mb,
                        Err(_e) => simple_kernel_panic("NVMe", "Could not allocate PRP List\n"),
                    };
                    ret[1] = table.base;
                    let array = unsafe {
                        slice::from_raw_parts_mut(table.as_mut_ptr() as *mut u64, p as usize)
                    };
                    for i in 0..p {
                        array[i as usize] = aligned_address;
                        aligned_address += 0x1000;
                    }
                    return (ret, Option::Some(table));
                }
            }
        }

        return (ret, Option::None);
    }

    pub fn not_present() -> Self {
        return Self {
            present_: false,
            num_disks_present: 0,
            abar: NVMeBar::new(0, null_mut()),
            admin_queue: NVMeQueue::new(
                align_of::<NVMeCompletionEntry>() as u64,
                align_of::<NVMeSubmissionEntry>() as u64,
                1,
                1,
                0,
                0,
            ),
            check_queue: 0,
            num_queues_implemented: 1,
            num_outstanding_completions: [0; 8],
            controller_information: 0,
            disks: null_mut(),
            io_queues: null_mut(),
            maximum_data_transfer_size: 0,
            allocator: Allocator::empty(),
        };
    }

    pub fn setup_controller(&mut self, allocator: &mut Allocator) {
        let abar = &mut self.abar;

        let asq = match allocator.alloc_zero(4) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic(
                "NVMeController",
                "Could not allocate Admin Submission Queue\n",
            ),
        };

        let acq = match allocator.alloc_zero(1) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic(
                "NVMeController",
                "Could not allocate Admin Completion Queue\n",
            ),
        };

        while !abar.csts().rdy() {}
        abar.cc().set_en(false);
        sleep(600);
        abar.aqa().set_asqs(255);
        abar.aqa().set_acqs(255);
        abar.set_acq(acq);
        abar.set_asq(asq);
        /* CC.AMS CC.MPS CC.CCS has been set to 0 after reset. Which is the value required for the Kernel*/

        abar.clear_intmc(0); // Unmasking 0th Interrupt Vector

        abar.cc().set_en(true);
        sleep(600);
        while !abar.csts().rdy() {}
        self.admin_queue = NVMeQueue::new(acq, asq, abar.hdbl(0), abar.tdbl(0), 256, 0);
    }

    pub fn identify_controller(&mut self, allocator: &mut Allocator) {
        let identify_controller_cmd = NVMeIdentifyCommand::identify_controller(allocator);
        self.dispatch_admin(&identify_controller_cmd);
        self.controller_information = identify_controller_cmd.data();

        let cqes = unsafe { *((self.controller_information + 513) as *const u8) } & 0b1111;
        let sqes = unsafe { *((self.controller_information + 512) as *const u8) } & 0b1111;

        if 1 << cqes != 16 || 1 << sqes != 64 {
            simple_kernel_panic(
                "NVMe",
                "Invalid Submission Queue or Completion Queue Entry size\n",
            )
        }

        let mdts = unsafe { *((self.controller_information + 77) as *const u8) };
        if mdts == 0 {
            self.maximum_data_transfer_size = 0xFFFF; // Maximum Sectors for Read and write
        } else {
            self.maximum_data_transfer_size = 0x1000 * (1 << mdts);
            let mut module = Module::new("NVmeController");
            info!(
                &mut module,
                "Maximum Transfer Size = {}\n", self.maximum_data_transfer_size
            );
        }

        self.abar.cc().set_iocqes(cqes);
        self.abar.cc().set_iosqes(sqes);
    }

    pub fn identify_namespaces(&mut self, allocator: &mut Allocator) {
        let active_nsid_list = NVMeIdentifyCommand::active_nsid_list(allocator);
        self.dispatch_admin(&active_nsid_list);

        self.disks = match allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic(
                "NVMeController",
                "identify_namespaces: Could not allocate disks\n",
            ),
        };

        let mut module = Module::new("NVMe");

        let list = unsafe { slice::from_raw_parts(active_nsid_list.data() as *const u32, 4096) };
        let mut count = 0;
        for i in 0..4096 {
            if list[i] != 0 {
                let mut identify_namespace =
                    NVMeIdentifyCommand::identify_namespace_data_structures(allocator, list[i]);
                self.dispatch_admin(&identify_namespace);

                let disk = unsafe { self.disks.add(count as usize).as_mut().unwrap() };
                let identification_data = identify_namespace.data();

                let formatted_lba = unsafe { *((identification_data + 26) as *const u8) };
                let lba_size = 1
                    << unsafe {
                        *((identification_data + 128 + (formatted_lba as u64 * 4) + 2) as *const u8)
                    };
                disk.sector_size_ = lba_size;
                disk.nsid = list[i];
                disk.num_sectors_ = unsafe { *((identification_data + 16) as *const u64) };
                disk.identifier_ = 1 << 20 | disk.nsid;
                info!(
                    &mut module,
                    "Disk 0x{:x}: sector size {} | num sectors {}\n",
                    disk.identifier_,
                    disk.sector_size_,
                    disk.num_sectors_
                );
                identify_namespace.release(allocator);

                count += 1;
            } else {
                break;
            }
        }
        self.num_disks_present = count;
    }

    pub fn create_basic_io_pairs(&mut self, allocator: &mut Allocator) {
        self.num_queues_implemented += 1;
        self.io_queues = match allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("NVMe", "Could not allocate I/O Queues\n"),
        };

        let queue_base = match allocator.alloc_zero(2) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic("NVMe", "Could not allocate basic io pair\n"),
        };

        let size;
        if 256 > self.abar.cap().mqes() {
            size = self.abar.cap().mqes();
        } else {
            size = 256;
        }

        let completion_queue =
            NVMeCreationCommand::create_io_completion_queue(1, size - 1, 0, queue_base);
        let submission_queue =
            NVMeCreationCommand::create_io_submission_queue(1, 1, size - 1, 0, queue_base + 0x1000);

        self.dispatch_admin(&completion_queue);
        self.dispatch_admin(&submission_queue);

        unsafe {
            (*self.io_queues) = NVMeQueue::new(
                queue_base,
                queue_base + 0x1000,
                self.abar.hdbl(1),
                self.abar.tdbl(1),
                size,
                1 << 14,
            )
        };
    }

    pub fn create_io_pair(&mut self, allocator: &mut Allocator) {
        let queue_base = match allocator.alloc_zero(2) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic("NVMe", "Could not allocate basic io pair\n"),
        };

        let size;
        if 256 > self.abar.cap().mqes() {
            size = self.abar.cap().mqes();
        } else {
            size = 256;
        }

        let completion_queue = NVMeCreationCommand::create_io_completion_queue(
            self.num_queues_implemented as u16,
            size - 1,
            0,
            queue_base,
        );
        let submission_queue = NVMeCreationCommand::create_io_submission_queue(
            self.num_queues_implemented as u16,
            1,
            size - 1,
            0,
            queue_base + 0x1000,
        );
        self.dispatch_admin(&completion_queue);
        self.dispatch_admin(&submission_queue);

        unsafe {
            (*self.io_queues.add(self.num_queues_implemented as usize - 1)) = NVMeQueue::new(
                queue_base,
                queue_base + 0x1000,
                self.abar.hdbl(self.num_queues_implemented as u32),
                self.abar.tdbl(self.num_queues_implemented as u32),
                size,
                1 << 15,
            );
        };
        self.num_queues_implemented += 1;
    }

    pub fn dispatch_admin(&mut self, command: &dyn NVMeCommand) {
        self.check_queue |= 1;
        self.num_outstanding_completions[0] = 1;
        self.admin_queue.issue_general(command);
        while self.check_queue & 1 == 1 {}
    }

    pub fn dispatch_read(
        &mut self,
        queue: u8,
        num_logical_blocks: u16,
        lba: u64,
        prps: [u64; 2],
        nsid: u32,
    ) {
        self.check_queue |= 1 << queue;
        self.num_outstanding_completions[queue as usize] += 1;
        unsafe {
            (*(self.io_queues.add((queue - 1) as usize))).issue_read(
                num_logical_blocks,
                lba,
                prps[0],
                prps[1],
                nsid,
            );
        }
        while self.check_queue & (1 << queue) != 0 {}
    }
    pub fn dispatch_write(
        &mut self,
        queue: u8,
        num_logical_blocks: u16,
        lba: u64,
        prps: [u64; 2],
        nsid: u32,
    ) {
        self.check_queue |= 1 << queue;
        self.num_outstanding_completions[queue as usize] += 1;
        unsafe {
            (*(self.io_queues.add((queue - 1) as usize))).issue_write(
                num_logical_blocks,
                lba,
                prps[0],
                prps[1],
                nsid,
            );
        }
        while self.check_queue & (1 << queue) != 0 {}
    }

    /**
     * Let´s the other cores access the queues
     */
    pub fn sync_vmem(&self, threading: &Multithreading) {
        threading.foreach_lapic(|lapic| {
            if lapic.get_id() as u32 != LocalApic::from_local_core().get_id() {
                threading.send_user_ipi(lapic.get_id(), |packet| {
                    packet.status = crate::processes::IpiStatus::Pending;
                    packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                    packet.request_data.vmem_sync.vaddr = NVME_BAR_FIXED_VADDR + 0x1000;
                    packet.request_data.vmem_sync.paddr = MemoryBlock {
                        base: self.abar.get_physical_address() + 0x1000,
                        length: 0x1000,
                    };
                });
                if !threading.await_ipi(lapic.get_id()) {
                    simple_kernel_panic("NVMeController/sync_vmem", "ipi got stuck\n")
                }
            }
        });
    }
}

static mut NVME_CONTROLLER: *mut NVMeController = null_mut();

fn nvme_irq(_: &ISRRegisters) {
    let nvme_controller = unsafe { NVME_CONTROLLER.as_mut().unwrap() };
    let mut interrupt_value = nvme_controller.check_queue;

    for i in 0..nvme_controller.num_queues_implemented {
        if interrupt_value & (1 << i) != 0 {
            interrupt_value ^= 1 << i;
            /* Admin Queue */
            if i == 0 {
                for _ in 0..nvme_controller.num_outstanding_completions[0] {
                    if unsafe { (*NVME_CONTROLLER).admin_queue.unissue_silent() } != 0 {
                        simple_kernel_panic("NVMe", "irq: Admin Command failed\n");
                    }
                }
                nvme_controller.num_outstanding_completions[0] = 0;
            } else {
                for _ in 0..nvme_controller.num_outstanding_completions[i as usize] {
                    if unsafe {
                        (*NVME_CONTROLLER)
                            .io_queues
                            .add((i - 1) as usize)
                            .as_mut()
                            .unwrap()
                    }
                    .unissue_silent()
                        != 0
                    {
                        simple_kernel_panic("NVMe", "irq: Queue Command failed\n")
                    }
                }
                nvme_controller.num_outstanding_completions[i as usize] = 0;
            }
        }
    }
    nvme_controller.check_queue = interrupt_value;

    let _ = LocalApic::from_local_core().send_eoi();
}

impl DiskController for NVMeController {
    fn new(
        pci_bus: &crate::hal::pci_bus::PciBus,
        pci_device: u64,
        physical_allocator: &mut crate::hal::memory::allocator::Allocator,
        pager: &mut crate::hal::memory::pager::Pager,
        isr_vector: u8,
        dst: &mut NVMeController,
    ) {
        pci_bus.enable_bus_master(pci_device);
        pci_bus.enable_interrupts(pci_device);

        let bar = pci_bus.get_bar(pci_device, PciBarIndex::Index0).unwrap();

        bar.map_to_virtual(pager, NVME_BAR_FIXED_VADDR, physical_allocator);

        ref_processor_mut().install_isr(nvme_irq, isr_vector);

        dst.present_ = true;
        dst.abar = NVMeBar::new(bar.get_address(), NVME_BAR_FIXED_VADDR as *mut c_void);
        dst.setup_controller(physical_allocator);
        unsafe { NVME_CONTROLLER = dst as *mut NVMeController };
        dst.identify_controller(physical_allocator);
        dst.identify_namespaces(physical_allocator);
        dst.create_basic_io_pairs(physical_allocator);
        let mut module = Module::new("NVMe");
        dst.allocator = physical_allocator.subdivide(32); /* Mainly used for PRPs*/
        success!(
            &mut module,
            "Initialized Controller and found {} connected disks\n",
            dst.num_disks_present
        );
    }
    fn get_disk_mut(&mut self, identifier: u32) -> Option<&'static mut dyn super::Disk> {
        for i in 0..self.num_disks_present {
            let disk = unsafe { self.disks.add(i as usize) };
            if unsafe { (*disk).nsid } == identifier {
                return Option::Some(unsafe { self.disks.add(i as usize).as_mut().unwrap() });
            }
        }
        Option::None
    }

    fn get_disk_indexed_mut(&mut self, index: u8) -> Option<(u32, &'static mut dyn Disk)> {
        if index < self.num_disks_present {
            let disk = unsafe { self.disks.add(index as usize) };
            let ident = unsafe { (*disk).nsid };
            return Option::Some((ident, unsafe { disk.as_mut().unwrap() }));
        }
        Option::None
    }

    fn get_disk(&self, identifier: u32) -> Option<&'static dyn Disk> {
        for i in 0..self.num_disks_present {
            let disk = unsafe { self.disks.add(i as usize) };
            if unsafe { (*disk).identifier_ } == identifier {
                return Option::Some(unsafe { self.disks.add(i as usize).as_ref().unwrap() });
            }
        }
        Option::None
    }
    fn get_disk_indexed(&self, index: u8) -> Option<(u32, &'static dyn Disk)> {
        if index < self.num_disks_present {
            let disk = unsafe { self.disks.add(index as usize) };
            let ident = unsafe { (*disk).identifier_ };
            return Option::Some((ident, unsafe { disk.as_ref().unwrap() }));
        }
        Option::None
    }

    fn hotplugging_is_supported(&self) -> bool {
        false
    }
    fn identify(&self) -> super::ControllerType {
        return ControllerType::NVMe;
    }
    fn num_disks_present(&self) -> u8 {
        return self.num_disks_present;
    }
    fn present(&self) -> bool {
        return self.present_;
    }
}
