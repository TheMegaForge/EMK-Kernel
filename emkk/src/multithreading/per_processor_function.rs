use core::arch::asm;

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic},
    fixed_vaddrs::{
        FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS,
        FIXED_KERNEL_IPI_DESCRIPTOR_PACKET_VIRTUAL_ADDRESS, FIXED_PROCESSOR_VIRTUAL_ADDRESS,
    },
    hal::memory::{
        allocator::{Allocator, VirtualAllocator},
        pager::{PAGER_PRESENT, PAGER_RW},
    },
    multithreading::processors::Processor,
    processes::{ApplicationComplex, IpiDescriptorPacket},
};
#[unsafe(no_mangle)]
pub fn kernel_ipi_dst(_: &ISRRegisters) {
    let ipi = unsafe {
        &mut *(FIXED_KERNEL_IPI_DESCRIPTOR_PACKET_VIRTUAL_ADDRESS as *mut IpiDescriptorPacket)
    };
    ipi.status = crate::processes::IpiStatus::Recieved;
    let processor = unsafe { &mut *(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor) };
    let physical_allocator = unsafe { &mut *processor.physical_allocator };
    let pager = unsafe { &mut *processor.pager };
    let application_complex =
        unsafe { &mut *(FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS as *mut ApplicationComplex) };
    match ipi.request_type {
        crate::processes::IpiRequestType::Invalid => {}
        crate::processes::IpiRequestType::SyncVMem => {
            for i in 0..unsafe { ipi.request_data.vmem_sync.paddr.length / 0x1000 } {
                pager
                    .page_4_kb(
                        unsafe { ipi.request_data.vmem_sync.vaddr } + i * 0x1000,
                        unsafe { ipi.request_data.vmem_sync.paddr.base } + i * 0x1000,
                        PAGER_RW | PAGER_PRESENT,
                        physical_allocator,
                    )
                    .unwrap();
            }
        }
        crate::processes::IpiRequestType::SummonApplication => {
            match application_complex.summon_application(
                unsafe { ipi.request_data.application_to_summon },
                unsafe { &mut *processor.virtual_allocator },
                physical_allocator,
                unsafe { &mut *processor.kernel_allocator },
            ) {
                Ok(launcher) => {
                    LocalApic::from_local_core().send_eoi();
                    ipi.request_type = crate::processes::IpiRequestType::Invalid;
                    ipi.status = crate::processes::IpiStatus::Completed;
                    /* TODO: Tell the pager to page the launcher function as kernel space, so the cr3 switch is even possible*/
                    (launcher)(); // INFO: this destroys the current stack, since it launches the application.
                }
                Err(_e) => {
                    LocalApic::from_local_core().send_eoi();
                    ipi.request_type = crate::processes::IpiRequestType::Invalid;
                    ipi.status = crate::processes::IpiStatus::Failed;
                }
            }
        }
    }
    LocalApic::from_local_core().send_eoi();
    ipi.request_type = crate::processes::IpiRequestType::Invalid;
    ipi.status = crate::processes::IpiStatus::Completed;
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".ap_segment")]
pub extern "C" fn ap_function(
    physical_allocator: &mut Allocator,
    physical_store_address: u64,
    ipi_packet_descriptor_paddr: u64,
    application_pool_complex_paddr: u64,
    virtual_allocator: &mut VirtualAllocator,
) -> ! {
    //INFO: INITIALIZATION CODE DOES NOT WAIT.
    unsafe {
        *(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor) = Processor::new(
            virtual_allocator,
            physical_allocator,
            *(0x9048 as *mut &mut Allocator),
            *(0x9020 as *const u8),
            physical_store_address,
            ipi_packet_descriptor_paddr,
            application_pool_complex_paddr,
        );

        let complex = (FIXED_APPLICATION_COMPLEX_VIRTUAL_ADDRESS as *mut ApplicationComplex)
            .as_mut()
            .unwrap();
        *complex = ApplicationComplex::allocate(
            *(0x9020 as *const u8) + 1,
            physical_allocator,
            &mut virtual_allocator.allocator,
        );

        asm!("sti");
    }
    loop {}
}
