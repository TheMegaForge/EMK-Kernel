#![no_std]
#![no_main]

use crate::{
    arch::{
        gdt::{gdt_switch_address, initialize_host_core_gdt},
        interrupts::{deactivate_interrupts, idt_switch_address, initialize_host_core_idt},
    },
    fixed_vaddrs::FIXED_LOCAL_APIC_VIRTUAL_ADDRESS,
    hal::{
        ImageAllocation, SystemTable, hal_init,
        memory::{
            allocator::{Allocator, VirtualAllocator},
            pager::{HOST_CORE_PAGER, PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager},
        },
        print::{GopFramebuffer, Module, simple_kernel_panic, switch_framebuffer_location},
    },
    time::sleep,
    utils::intrin::rdmsr,
};
use core::{error, ffi::c_void, panic::PanicInfo};

pub mod acpi_tables;
pub mod aml;
pub mod arch;
pub mod drivers;
pub mod fixed_handle_basis;
pub mod fixed_vaddrs;
pub mod hal;
pub mod multithreading;
pub mod pfs;
pub mod processes;
pub mod time;
pub mod utils;
pub mod vfs;
#[unsafe(no_mangle)]
#[unsafe(link_section = ".kentry")]
pub extern "C" fn kentry(
    pma_util: *mut c_void,
    framebuffer: *mut GopFramebuffer,
    memory_descriptors: *mut c_void,
    descriptor_size: u32,
    descriptor_count: u32,
    num_allocations: u32,
    font: *const c_void,
    rsdp: *const c_void,
    image_allocations: *const c_void,
    stack: *const c_void,
) -> ! {
    let mut system_table: SystemTable;

    system_table = match hal_init(
        pma_util,
        framebuffer,
        font,
        descriptor_size,
        descriptor_count,
        memory_descriptors,
        num_allocations,
        image_allocations,
    ) {
        Ok(sys_table) => sys_table,
        Err(_e) => {
            let mut module = Module::new("HAL");
            error!(module, "HAL initialization failed (Critical Error)\n");
            loop {}
        }
    };
    let physical_allocator: &mut Allocator = &mut system_table.physical_allocator;
    let virtual_allocator: &mut VirtualAllocator = &mut system_table.virtual_allocator;
    deactivate_interrupts();
    initialize_host_core_gdt(physical_allocator);
    initialize_host_core_idt(physical_allocator);
    let cr3: *mut u64 = match physical_allocator.alloc_zero(1) {
        Ok(mb) => mb.as_mut_ptr(),
        Err(_e) => simple_kernel_panic("Kernel", "Could not allocate cr3\n"),
    };
    unsafe {
        HOST_CORE_PAGER = match Pager::host_core(
            physical_allocator,
            &mut system_table.root_allocator,
            virtual_allocator,
            num_allocations,
            image_allocations,
            stack,
            cr3,
        ) {
            Ok(pager) => pager,
            Err(_e) => {
                simple_kernel_panic("Kernel", "Initializing Page for host core failed\n");
            }
        };
        let local_apic_address = rdmsr(0x1B) & !0xFFF;
        #[allow(static_mut_refs)]
        HOST_CORE_PAGER
            .page_4_kb(
                FIXED_LOCAL_APIC_VIRTUAL_ADDRESS,
                local_apic_address,
                PAGER_PRESENT | PAGER_RW | PAGER_PCD,
                physical_allocator,
            )
            .unwrap();
        #[allow(static_mut_refs)]
        switch_framebuffer_location(&mut HOST_CORE_PAGER, physical_allocator);
        gdt_switch_address(physical_allocator, virtual_allocator);
        idt_switch_address(physical_allocator, virtual_allocator);
    }
    system_table.discover_acpi_tables(rsdp);
    system_table.discover_pci_devices();
    system_table.initialize_multithreading();
    system_table.initialize_apic();
    system_table.initialize_hpet();
    system_table.enable_interrupts();
    /* TODO: Do more virtual stuff*/
    system_table.initialize_cores(num_allocations, image_allocations as *const ImageAllocation);
    system_table.initialize_aml();
    system_table.route_interrupts();
    system_table.initialize_usb();
    system_table.initialize_keyboard();
    system_table.initialize_disk_drivers();
    system_table.initialize_file_systems();
    let resources = system_table.initialize_executable_loader();
    system_table.load_system_libraries(resources);
    //system_table.test();
    sleep(200);
    /*
     * TODO: activate apic counter for task switching
     * INFO: User core got interrupt request + now make it load a process + implement file descriptors
     */
    let mut module = Module::new("EMK Kernel");
    success!(&mut module, "Successfully initialized\n");
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let mut module = Module::new("Rust/Panic");
    if let Some(loc) = _info.location() {
        error!(module, "Thrown ; Reason = {}\n", _info.message());
        error!(
            module,
            "In File {} on Line {} ; Column = {}\n",
            loc.file(),
            loc.line(),
            loc.column()
        );
    } else {
        error!(module, "Thrown ; Reason = {}\n", _info.message());
    }
    loop {}
}
