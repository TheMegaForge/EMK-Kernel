use core::ptr::{null, null_mut};

use crate::{
    hal::{
        memory::{
            allocator::Allocator,
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager},
        },
        print::{Module, simple_kernel_panic},
    },
    multithreading::{GsiOverride, IoApic},
    success,
    utils::list::List,
};

pub struct Apic {
    apics: *const List<IoApic>,
    gsi_overrides: *mut List<GsiOverride>,
}

impl Default for Apic {
    fn default() -> Self {
        return Self {
            apics: null(),
            gsi_overrides: null_mut(),
        };
    }
}

/*(c) Osdev wiki */
pub const IOAPICID: u8 = 0x00;
pub const IOAPICVER: u8 = 0x01;
pub const IOAPICARB: u8 = 0x02;

#[inline(always)]
#[allow(non_snake_case)]
pub fn IOAPICREDTBL(n: u8) -> u8 {
    return 0x10 + (2 * n);
}

pub const IO_APIC_DESTINATION_PHYSICAL: u8 = 0;
pub const IO_APIC_DESTINATION_LOGICAL: u8 = 1;
pub const IO_APIC_ACTIVE_HIGH: u8 = 0;
pub const IO_APIC_ACTIVE_LOW: u8 = 1;
pub const IO_APIC_EDGE: u8 = 0;
pub const IO_APIC_LEVEL: u8 = 1;

impl Apic {
    pub fn new(
        apics: *const List<IoApic>,
        gsi_overrides: *mut List<GsiOverride>,
        pager: &mut Pager,
        allocator: &mut Allocator,
        virtual_allocator: &mut Allocator,
    ) -> Self {
        unsafe {
            (*apics).for_each(|_, io_apic| -> bool {
                match pager.page_4_kb(
                    (*io_apic).virtual_address,
                    (*io_apic).physical_address,
                    PAGER_PRESENT | PAGER_RW | PAGER_PCD,
                    allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => simple_kernel_panic("Apic/new", "Could not page Apic\n"),
                }
                true
            })
        };

        let mut module = Module::new("Apic");
        success!(module, "Initialized \n");
        return Self {
            apics,
            gsi_overrides,
        };
    }

    pub fn register_gsi(&mut self, gsi: u32) -> bool {
        let mut wrote = false;
        unsafe {
            (*self.gsi_overrides).for_each_mut(|_, gsi_override| -> bool {
                if (*gsi_override).gsi == gsi {
                    if (*gsi_override).registered {
                        return false;
                    }
                    wrote = true;
                    (*gsi_override).registered = true;
                    false
                } else {
                    true
                }
            });
        }

        return wrote;
    }

    pub fn write_and_register_gsi(
        &mut self,
        gsi: u32,
        isr_vector: u8,
        destination_mode: u8,
        pin_polarity: u8,
        trigger_mode: u8,
        destination: u32,
    ) -> bool {
        if !self.register_gsi(gsi) {
            return false;
        }

        unsafe {
            (*self.gsi_overrides).for_each(|_, gsi_override| -> bool {
                if (*gsi_override).gsi == gsi {
                    let apic = (*self.apics)
                        .ref_const((*gsi_override).corresponding_io_apic_index as u32)
                        .unwrap();
                    let mut half0 = isr_vector as u32;
                    half0 |= (destination_mode as u32 & 1) << 11;
                    half0 |= (pin_polarity as u32 & 1) << 13;
                    half0 |= (trigger_mode as u32 & 1) << 15;
                    let mut half1 = destination;
                    half1 <<= 23;
                    apic.write_register(
                        IOAPICREDTBL((*gsi_override).io_apic_redirection_entry as u8),
                        half0,
                    );
                    apic.write_register(
                        IOAPICREDTBL((*gsi_override).io_apic_redirection_entry as u8) + 1,
                        half1,
                    );
                    false
                } else {
                    true
                }
            });
        }

        return true;
    }

    pub fn write_gsi(
        &mut self,
        gis: u32,
        isr_vector: u8,
        destination_mode: u8,
        pin_polarity: u8,
        trigger_mode: u8,
        destination: u32,
    ) -> bool {
        let ret = false;
        unsafe {
            (*self.apics).for_each(|_, apic_| -> bool {
                let apic = apic_.as_ref().unwrap();
                if gis >= apic.gis_base && apic.gis_base + 24 >= gis {
                    let entry = gis - apic.gis_base;
                    let mut half0 = isr_vector as u32;
                    half0 |= (destination_mode as u32 & 1) << 11;
                    half0 |= (pin_polarity as u32 & 1) << 13;
                    half0 |= (trigger_mode as u32 & 1) << 15;
                    let mut half1 = destination;
                    half1 <<= 23;
                    apic.write_register(IOAPICREDTBL(entry as u8), half0);
                    apic.write_register(IOAPICREDTBL(entry as u8) + 1, half1);
                    false
                } else {
                    true
                }
            });
        };
        return ret;
    }
}
