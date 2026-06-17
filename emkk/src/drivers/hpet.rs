use core::ffi::c_char;

use crate::{
    acpi_tables::{AcpiTable, GenericAddressStructure},
    arch::{
        apic::{Apic, IO_APIC_ACTIVE_HIGH, IO_APIC_DESTINATION_PHYSICAL, IO_APIC_EDGE},
        isr::ISRRegisters,
    },
    error,
    fixed_vaddrs::{FIXED_PROCESSOR_VIRTUAL_ADDRESS, HPET_BAR_FIXED_VADDR, ref_processor_mut},
    hal::{
        memory::{
            allocator::Allocator,
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager},
        },
        print::{Module, simple_kernel_panic},
    },
    info,
    multithreading::processors::Processor,
};

pub struct Hpet {
    address: u64,
}

impl Default for Hpet {
    fn default() -> Self {
        return Self { address: 0 };
    }
}

#[repr(C, packed)]
struct HPET {
    signature: [c_char; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [c_char; 6],
    oemtableid: [c_char; 8],
    oemrevision: u32,
    creator_id: u32,
    creator_revision: u32,
    hardware_revision_id: u8, //hardware_rev_id;

    /*
    *
    uint8_t ComparatorCount:5;//comparator_count:5;
    uint8_t CounterSize:1;//counter_size:1;
    uint8_t Reserved:1;//reserved:1;
    uint8_t LegacyReplacement:1;//legacy_replacement:1;
    */
    info0: u8,
    pci_vendor_id: u16, //pci_vendor_id;
    address: GenericAddressStructure,
    hpet_number: u8,     // hpet_number;
    minimum_tick: u16,   //minimum_tick;
    page_protection: u8, //page_protection;
}

pub const TIMER_0_INTERVAL_FS: u64 = 10000000000000;
pub const TIMER_1_INTERVAL_FS: u64 = 32_000_000_000_000; // 32 ms
pub const TIMER_COUNT_BITMASK: u64 = 0x1F00;

pub const TIMER_INT_ENB_CNF: u32 = 1 << 2;
pub const TIMER_32_BIT_MODE_CNF: u32 = 1 << 8;
pub const TIMER_FSB_EN_CNF: u32 = 1 << 14;
pub const TIMER_TYPE_CNF: u32 = 1 << 3;
pub const TIMER_VAL_SET_CNF: u32 = 1 << 6;
pub const ENABLE_CNF: u32 = 1;

pub const TIMER_INT_ROUTE_CNF: u64 = 0x1F << 9;

unsafe extern "C" {
    static mut m_Counter: u64;
}
#[unsafe(no_mangle)]
pub fn counter_increment(_registers: &ISRRegisters) {
    unsafe {
        m_Counter += 1;
        ref_processor_mut().get_lapic().send_eoi();
    }
}

/* Initialization:
 *      new()
 *      reset()
 *      enable()
 */
impl Hpet {
    pub fn new(
        hpet_table: *const AcpiTable,
        pager: &mut Pager,
        allocator: &mut Allocator,
        apic: &mut Apic,
    ) -> Self {
        let mut module = Module::new("HPET");
        let hpet = unsafe { ((*hpet_table).get_ptr() as *const HPET).as_ref().unwrap() };
        match pager.page_4_kb(
            HPET_BAR_FIXED_VADDR,
            hpet.address.address,
            PAGER_PRESENT | PAGER_RW | PAGER_PCD,
            allocator,
        ) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic(module.name(), "Could not page hpet\n");
            }
        };
        let address = HPET_BAR_FIXED_VADDR;
        let mut general_capabilities;
        /* enables legacy routing */
        unsafe {
            ((address + 0x10) as *mut u64).write_volatile(1 << 1);
            general_capabilities = (address as *const u64).read_volatile();
            general_capabilities >>= 32;
            info!(module, "clock period = {}\n", general_capabilities);
        };

        let period_fs = general_capabilities; // tick period in femtoseconds
        let mut ticks = (TIMER_0_INTERVAL_FS + period_fs / 2) / period_fs;
        let mut timer0_configuration = unsafe { ((address + 0x100) as *const u64).read_volatile() };
        if (timer0_configuration & 1 << 4) == 0 {
            simple_kernel_panic(module.name(), "timer 0: Periodic Mode is not supported\n");
        }
        if (timer0_configuration & 1 << 5) != 0 {
            info!(module, "timer 0 is 64 bits\n");
            timer0_configuration &= !TIMER_32_BIT_MODE_CNF as u64; // Disables force 32 bit mode
        } else {
            info!(module, "timer 0 is 32 bits\n");
        }

        timer0_configuration &= !TIMER_INT_ENB_CNF as u64; // Disables Timer
        timer0_configuration &= !TIMER_FSB_EN_CNF as u64; // Disables FSB delivery
        timer0_configuration |= TIMER_TYPE_CNF as u64; // Periodic Interrupts
        timer0_configuration |= TIMER_VAL_SET_CNF as u64; // software can set a periodic´s timer accumulator
        timer0_configuration &= !(1 << 4 | 1 << 5);
        unsafe {
            ((address + 0x100) as *mut u64).write_volatile(timer0_configuration);
            ((address + 0x108) as *mut u64).write_volatile(ticks);
        };

        let processor = FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor;
        let isr_vector = unsafe { (*processor).request_isr_vector() };

        apic.write_and_register_gsi(
            0x2,
            isr_vector,
            IO_APIC_DESTINATION_PHYSICAL,
            IO_APIC_ACTIVE_HIGH,
            IO_APIC_EDGE,
            unsafe { (*processor).get_lapic().get_id() } as u32,
        );
        unsafe { (*processor).install_isr(counter_increment, isr_vector) };
        let hpet = Hpet { address };

        return hpet;
    }

    #[inline(always)]
    fn general_config_register(&mut self) -> *mut u64 {
        return (self.address + 0x10) as *mut u64;
    }

    #[inline(always)]
    fn main_counter_register(&mut self) -> *mut u64 {
        return (self.address + 0xF0) as *mut u64;
    }

    fn flush_timer(&mut self) {
        unsafe {
            self.general_config_register().write_volatile(
                self.general_config_register().read_volatile() & !ENABLE_CNF as u64,
            ); //Disables general timer
            self.main_counter_register().write_volatile(0);
            self.general_config_register()
                .write_volatile(self.general_config_register().read_volatile() | ENABLE_CNF as u64); //Enables general timer
        }
    }

    fn fully_disable(&mut self) {
        let capabilities = unsafe {
            (((self.address as *const u64).read_volatile() & TIMER_COUNT_BITMASK) >> 8) + 1
            //this is the number of timers
        };
        for i in 0..capabilities {
            let timer_address = self.address + 0x100 + 0x20 * i;
            let mut original_configuration =
                unsafe { (timer_address as *const u64).read_volatile() };
            original_configuration = original_configuration & !TIMER_INT_ENB_CNF as u64; // disables timer
            unsafe { (timer_address as *mut u64).write_volatile(original_configuration) };
        }
        let mut general_config = unsafe { self.general_config_register().read_volatile() };
        general_config &= !ENABLE_CNF as u64; //Disables the HPET in general.
        unsafe {
            self.general_config_register()
                .write_volatile(general_config)
        };
    }

    pub fn reset_fully(&mut self) {
        self.fully_disable();
        self.flush_timer();
    }

    fn enable_general(&mut self) {
        unsafe {
            ((self.address + 0x10) as *mut u64).write_volatile(
                ((self.address + 0x10) as *const u64).read_volatile() | ENABLE_CNF as u64,
            );
        }
    }

    fn enable_timer0(&mut self) {
        let mut timer_configuration =
            unsafe { ((self.address + 0x100) as *const u64).read_volatile() };
        timer_configuration &= !(1 << 4 | 1 << 5 | 1 << 7 | 1 << 5);
        timer_configuration |= TIMER_INT_ENB_CNF as u64;
        unsafe { ((self.address + 0x100) as *mut u64).write_volatile(timer_configuration) };
    }

    /* only enables timer 0*/
    pub fn enable_all(&mut self) {
        self.enable_general();
        self.enable_timer0();
    }
}
