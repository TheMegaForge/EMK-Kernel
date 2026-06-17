use core::arch::asm;

use crate::{
    arch::{
        gdt::{GdtDescriptor, HOST_CORE_GDT_DESCRIPTOR},
        interrupts::{HOST_CORE_INTERRUPT_DESCRIPTOR_TABLE, InterruptDescriptorTable},
        isr::{HOST_CORE_ISR_CALLS, ISRRegisters, invalid_isr},
        lapic::LocalApic,
    },
    drivers::usb::independent::UsbControllerType::UHC,
    fixed_vaddrs::{
        FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS,
        FIXED_KERNEL_IPI_DESCRIPTOR_PACKET_VIRTUAL_ADDRESS, FIXED_LOCAL_APIC_VIRTUAL_ADDRESS,
    },
    hal::{
        memory::{
            allocator::{Allocator, VirtualAllocator},
            pager::{
                HOST_CORE_PAGER, IA32_PAT_MSR, PAGER_PCD, PAGER_PRESENT, PAGER_RW, PAT_RESET_VALUE,
                Pager,
            },
        },
        print::{Module, simple_kernel_panic},
    },
    multithreading::{local_apic_spurious_interrupt, per_processor_function::kernel_ipi_dst},
    processes::{ApplicationComplex, IpiDescriptorPacket},
    success,
    utils::{
        cpuid,
        intrin::{get_cr3, rdmsr, wrmsr},
        memory::memset_qword,
        traits::Region,
    },
};

// is allways paged to an specific address
#[allow(dead_code)]
pub struct Processor {
    local_apic: LocalApic,
    physical_store_address: u64,
    gdt: *mut GdtDescriptor,
    idt: *mut InterruptDescriptorTable,
    pub isr: *mut [fn(&ISRRegisters); 224],
    pub pager: *mut Pager,
    pub physical_allocator: *mut Allocator,
    pub kernel_allocator: *mut Allocator,
    pub virtual_allocator: *mut VirtualAllocator,
    next_free_isr_vector: u8,
    activation_id: u8, // 0 => host
}

pub const HOST_CORE_ACTIVATION_ID: u8 = 0;

impl Processor {
    pub fn ref_mut_pager(&mut self) -> &mut Pager {
        return unsafe { self.pager.as_mut().unwrap() };
    }

    pub fn request_isr_vector(&mut self) -> u8 {
        let ret = self.next_free_isr_vector;
        self.next_free_isr_vector -= 1;
        return ret;
    }
    // has to be expanded, so index 0 = idt entry 0
    // index 255 = idt entry 255
    pub fn install_isr(&mut self, isr_func: fn(&ISRRegisters), index: u8) {
        unsafe {
            (*self.isr)[index as usize - 32] = isr_func;
        }
    }

    pub fn halt_processor() -> ! {
        unsafe { asm!("cli;hlt") };
        loop {}
    }

    pub fn get_activation_id(&self) -> u8 {
        return self.activation_id;
    }
    /* NOTICE: this is getting executed from another core
     * Also tests for syscall/sysret
     *
     * */
    pub fn new(
        virtual_allocator: &mut VirtualAllocator,
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
        activation_id: u8,
        physical_store_address: u64,
        ipi_packet_descriptor_paddr: u64,
        application_pool_complex_paddr: u64,
    ) -> Processor {
        let mut local_apic = LocalApic::from_local_core();
        let mb = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => {
                simple_kernel_panic(
                    "Processor/new",
                    "Could not allocate memory for gdt and idt\n",
                );
            }
        };

        if cpuid(0x8000_0001, 0).edx & (1 << 11) == 0 {
            simple_kernel_panic(
                "Processor/new",
                "Processor does not support syscall/sysret\n",
            );
        }

        /* activates syscall/sysret*/
        unsafe { wrmsr(0xC0000080, rdmsr(0xC0000080) | 1) };

        let gdt: *mut GdtDescriptor;
        if activation_id == 0 {
            gdt = &raw mut HOST_CORE_GDT_DESCRIPTOR;
        } else {
            gdt = mb.get_base() as *mut GdtDescriptor;
            unsafe {
                (*gdt).initialize(&mut virtual_allocator.allocator, activation_id, true);
            }
        }
        let idt: *mut InterruptDescriptorTable;
        if activation_id == 0 {
            idt = &raw mut HOST_CORE_INTERRUPT_DESCRIPTOR_TABLE;
        } else {
            idt = (mb.get_base() + 0x200) as *mut InterruptDescriptorTable;
            let table = match virtual_allocator.allocator.alloc_zero(1) {
                Ok(mb) => mb.as_mut_ptr(),
                Err(_e) => {
                    simple_kernel_panic("Processor/new", "Could not allocate table\n");
                }
            };
            unsafe {
                (*idt) = InterruptDescriptorTable::new_and_load(
                    &mut virtual_allocator.allocator,
                    table,
                    activation_id,
                );
            }
        }
        let pager: *mut Pager;
        if activation_id == 0 {
            pager = &raw mut HOST_CORE_PAGER;
        } else {
            let cr3 = unsafe { get_cr3() };
            let _pager: *mut Pager = match virtual_allocator.allocator.alloc_zero(1) {
                Ok(mb) => mb.as_mut_ptr(),
                Err(_e) => simple_kernel_panic("Processor/new", "Could not allocate pager\n"),
            };

            unsafe { wrmsr(IA32_PAT_MSR, PAT_RESET_VALUE) };

            pager = _pager;
            unsafe { (*pager) = Pager::new(cr3) };

            let mut module = Module::new("Pager");
            success!(module, "Initialized for Core {}\n", activation_id)
        }
        let isr: *mut [fn(&ISRRegisters); 256 - 32];
        if activation_id == 0 {
            isr = &raw mut HOST_CORE_ISR_CALLS;
        } else {
            let _isr = match virtual_allocator.allocator.alloc(1) {
                Ok(mb) => mb.as_mut_ptr(),
                Err(_e) => {
                    simple_kernel_panic("Processor/new", "Could not allocate memory for isrs\n");
                }
            };
            unsafe {
                memset_qword(_isr, invalid_isr as *const () as u64, 256 - 32);
            }
            isr = _isr as *mut [fn(&ISRRegisters); 256 - 32];
        }

        if activation_id != 0 {
            let local_apic_address = rdmsr(0x1B) & !0xFFF;
            unsafe {
                match (*pager).page_4_kb(
                    FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS,
                    application_pool_complex_paddr,
                    PAGER_PRESENT | PAGER_RW,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => simple_kernel_panic(
                        "Processor/new",
                        "Could not page application pool complex\n",
                    ),
                }

                match (*pager).page_4_kb(
                    FIXED_KERNEL_IPI_DESCRIPTOR_PACKET_VIRTUAL_ADDRESS,
                    ipi_packet_descriptor_paddr,
                    PAGER_PRESENT | PAGER_RW,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => simple_kernel_panic(
                        "Processor/new",
                        "Could not page ipi descriptor packet\n",
                    ),
                }

                let ipi = (FIXED_KERNEL_IPI_DESCRIPTOR_PACKET_VIRTUAL_ADDRESS
                    as *mut IpiDescriptorPacket)
                    .as_mut()
                    .unwrap();
                ipi.status = crate::processes::IpiStatus::Invalid;
                ipi.request_type = crate::processes::IpiRequestType::Invalid;

                match (*pager).page_4_kb(
                    FIXED_LOCAL_APIC_VIRTUAL_ADDRESS,
                    local_apic_address,
                    PAGER_PRESENT | PAGER_RW | PAGER_PCD,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => simple_kernel_panic("Processor/new", "Could not page local apic\n"),
                }
            }

            local_apic.reset();
        }

        let mut ret = Processor {
            local_apic,
            physical_store_address,
            gdt,
            isr,
            idt,
            pager,
            activation_id,
            next_free_isr_vector: 0xFD,
            physical_allocator: physical_allocator as *mut Allocator,
            kernel_allocator: kernel_allocator as *mut Allocator,
            virtual_allocator: virtual_allocator as *mut VirtualAllocator,
        };

        if activation_id != 0 {
            ret.install_isr(local_apic_spurious_interrupt, 0xFF);
            ret.install_isr(kernel_ipi_dst, 0xFE);
        }

        return ret;
    }

    pub fn get_lapic(&mut self) -> &mut LocalApic {
        return &mut self.local_apic;
    }
}
