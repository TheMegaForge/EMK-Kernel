use core::{arch::asm, ffi::c_void, ptr::null_mut};

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic},
    drivers::usb::{
        ehci::{
            data_structures::{
                AsynchronousList, PeriodicFrameList, QueueElementTransferDescriptor, QueueHead,
            },
            registers::{Fladj, InterruptThresholdControl, LineStatus, UsbBase},
            structures::{device::EhciDevice, endpoint::EhciEndpoint},
        },
        independent::UsbControllerInformation,
        traits::{UsbController, UsbDevice, UsbInterruptPollerCallbackFn},
    },
    error,
    fixed_vaddrs::ref_processor_mut,
    hal::{
        memory::allocator::Allocator,
        pci_bus::PciBus,
        print::{Module, simple_kernel_panic},
    },
    info,
    multithreading::processors::Processor,
    time::sleep,
    utils::{allocators::PageAllocator, list::List, queue::Queue},
    warn,
};

pub struct EhciInterruptPoller {
    pub callback: Option<UsbInterruptPollerCallbackFn>,
    pub designated_qtds: [QueueElementTransferDescriptor; 2],
    pub designated_qhs: *mut QueueHead,
    pub queue_head_count: u8,
    pub transfer_size: u16,
    pub current_active: u8,
}

pub mod configuration_parser;
pub mod data_structures;
pub mod ehc_controller_impl;
pub mod extended_request_impl;
pub mod registers;
pub mod standard_request_impl;
pub mod structures;
#[repr(align(16))]
pub struct Ehci {
    present: bool,
    usbbase: UsbBase,
    fladj: Fladj,
    module: Module<'static>,
    periodic_list: PeriodicFrameList,
    asynchronous_list: AsynchronousList,
    isr_vector: u8,
    devices: PageAllocator<EhciDevice>,
    memory_space: Allocator,
    data_packet_base: u32,
    endpoint_interrupted: bool,
    qhs_to_disable: Queue<QueueHead>,
    preinserted_answered: u8,
    information: UsbControllerInformation,
    interrupt_pollers: PageAllocator<EhciInterruptPoller>,
    interrupt_pollers_designated_qhs: PageAllocator<QueueHead>,
    interrupt_pollers_qhs: *mut c_void,
    interrupt_pollers_qtds: *mut c_void,
}

// TODO: Implement proper handler
fn ehci_interrupt(_registers: &ISRRegisters) {
    #[allow(static_mut_refs)]
    let ehci_controller = unsafe { &mut EHCI_CONTROLLER };
    let original_status = ehci_controller.usbbase.usbsts().as_u32();
    let mut _status = original_status;
    _status &= !(0b1111 << 12);
    _status &= !(1 << 3);
    match _status {

        3 /* UsbErrInt*/ => {
            simple_kernel_panic("ehci_interrupt", "Error Interrupt\n")
        }

        1 /* USBInt*/ => {
            let mut from_periodic_list = false;
            for i in 0..ehci_controller.interrupt_pollers.size() {
                let poller = ehci_controller.interrupt_pollers.as_mut(i).unwrap();

                for qh in 0..poller.queue_head_count {
                    let queue_head = unsafe {poller.designated_qhs.add(qh as usize).as_mut().unwrap()};
                    let mut current_qtd = QueueElementTransferDescriptor::new(queue_head.current_qtd_pointer() as u64);

                    // Queue Head has made an transaction
                    if current_qtd.status() & (1<<7) == 0 {

                        /*
                         * This code is being executed after the current qtd has been used up.
                         * But it <*this code*> activates the last qtd, since activating the current one would not be the smartest idea.
                         * Let´s assume the code executed a second time. the current qtd from the last execution would be the next_qtd on the second time.
                         * While the next qtd from the first execution would be the current qtd on the second execution
                         */

                        let next_qtd = &mut poller.designated_qtds[((!poller.current_active) & 1) as usize];
                        next_qtd.set_status_bit(7);
                        let next_qtd_address = next_qtd.get_address();
                        let transfer_size = poller.transfer_size;
                        poller.current_active = (!poller.current_active) & 1;
                        if let Option::Some(callback_fn) = poller.callback {
                            (callback_fn)(ehci_controller);
                        }

                        queue_head.next_qtd_pointer().set_transfer_element_pointer(next_qtd_address);
                        queue_head.next_qtd_pointer().set_terminate(false);

                        current_qtd.set_total_bytes_to_transfer(transfer_size);
                        current_qtd.set_current_offset(current_qtd.current_offset() - transfer_size);



                        from_periodic_list = true;
                        break;
                    }
                }
            }
            if from_periodic_list {
                ehci_controller.usbbase.usbsts().clear_usbint();
                LocalApic::from_local_core().send_eoi();
                return;
            }

            // FIXME: Check for Short packet
            ehci_controller.endpoint_interrupted = true;
            if ehci_controller.qhs_to_disable.num_occupied() == 0 {
                ehci_controller.preinserted_answered += 1;
            }else {
                let mut qh = ehci_controller.qhs_to_disable.dequeue();
                qh.clear_status_bit(7);
                qh.next_qtd_pointer().set_terminate(true);
            }

            ehci_controller.usbbase.usbsts().clear_usbint();
        }

        4 /* Port Change Detected*/ => {

            for p in 0..ehci_controller.usbbase.hcsparams().n_ports() {
                let mut port = ehci_controller.usbbase.portsc(p);
                let _portu32 = port.as_u32();
                if port.connect_status_change() {
                    if !port.current_connect_status() {
                        let mut device_detached = false;
                        // Find device which corresponds to the port and detach it.
                        for d in 0..ehci_controller.devices.size() {
                           let device = ehci_controller.devices.as_mut(d).unwrap();
                           if device.get_port() == p {
                               device.detach();
                               device_detached = true;
                           }
                        }
                        if !device_detached {
                            warn!(&mut ehci_controller.module, "undetected device deattached from eHC\n");
                        }
                    }else {
                        //TODO: Implement USB Device Attaching.
                        simple_kernel_panic("ehci_interrupt", "USB Device Attached\n");

                    }
                    port.clear_connect_status_change();
                }
            }
            ehci_controller.usbbase.usbsts().clear_port_change_detected();
        }
        _ => {
            error!(&mut ehci_controller.module, "Unhandled Status Interrupt 0x{:x}\n", _status);
            unsafe {asm!("cli;hlt")};
            loop {}
        }
    }
    LocalApic::from_local_core().send_eoi();
}

pub(in crate::drivers::usb) static mut EHCI_CONTROLLER: Ehci = Ehci::not_present();
pub fn create_ehci(
    pci_bus: &PciBus,
    pci_device: u64,
    physical_allocator: &mut Allocator,
    isr_vector: u8,
) {
    unsafe {
        let ehci_controller = &raw mut EHCI_CONTROLLER;
        (*ehci_controller).initialize_controller(
            pci_bus,
            physical_allocator,
            isr_vector,
            pci_device,
        );
        (*ehci_controller).untraited_work0();
        (*ehci_controller).gather_device_information();
        (*ehci_controller).configure_devices();
        (*ehci_controller).untraited_work2();
    }
}

/**
 * NOTICE: Probes Ports after Resetting and claiming, due to the detect port change interrupt not working after reset.
 * But hotplugging PCD works just fine.
 */
impl Ehci {
    pub const POLLING_INTERRUPTS_QUEUE_HEADS: u16 = 170;
    pub const POLLING_INTERRUPTS_QUEUE_TRANSFER_DESCRIPTORS: u16 = 256;
    pub const fn not_present() -> Ehci {
        return Ehci {
            usbbase: UsbBase::empty(),
            fladj: Fladj::new(0),
            present: false,
            module: Module::empty(),
            periodic_list: PeriodicFrameList::empty(),
            asynchronous_list: AsynchronousList::empty(),
            devices: PageAllocator::empty(),
            isr_vector: 0,
            memory_space: Allocator::empty(),
            data_packet_base: 0,
            endpoint_interrupted: false,
            qhs_to_disable: Queue::<QueueHead>::empty(),
            preinserted_answered: 0,
            information: UsbControllerInformation::empty(),
            interrupt_pollers_qhs: null_mut(),
            interrupt_pollers_qtds: null_mut(),
            interrupt_pollers: PageAllocator::empty(),
            interrupt_pollers_designated_qhs: PageAllocator::empty(),
        };
    }

    // Chapter 4.2.2
    fn reset_port(&mut self, port_: u8, default_control_endpoint_setup_data_packet_base: u32) {
        let mut port = self.usbbase.portsc(port_);
        {
            port.set_port_reset(true);
            port.set_port_enabled_disabled(false);
            sleep(200);
            port.set_port_reset(false);

            if port.port_enabled_disabled() {
                info!(
                    &mut self.module,
                    "Port {} is connected to a high speed device\n", port_
                );
            } else {
                port.set_port_owner(true);
                self.usbbase.usbsts().clear_port_change_detected();
                port.clear_port_enable_disable_change();
                return;
            }

            if let LineStatus::K = port.line_status() {
                port.set_port_owner(true);
                self.usbbase.usbsts().clear_port_change_detected();
                port.clear_port_enable_disable_change();
                return;
            }

            self.devices.push_back(EhciDevice::new_reset(
                &mut self.memory_space,
                port_,
                port_,
                default_control_endpoint_setup_data_packet_base,
            ));
        }
        self.usbbase.usbsts().clear_port_change_detected();
    }

    /*
     * Sets Periodic List and Async List
     * Disables Periodic List and Async List
     * Starts the Host Controller
     */
    fn initialize(&mut self) {
        ref_processor_mut().install_isr(ehci_interrupt, self.isr_vector);

        self.stop();
        self.reset();
        self.usbbase.usbsts().clear_port_change_detected();
        let mut usbintr = self.usbbase.usbintr();
        usbintr.set_usb_error_interrupt_enable(true);
        usbintr.set_usb_interrupt_enable(true);
        //usbintr.set_port_change_interrupt_enable(true); // THIS WILL BE SET AFTER PORT RESET
        usbintr.set_host_system_error_enable(true);
        let mut periodic_list = PeriodicFrameList::new(&mut self.memory_space);
        periodic_list.set(self);
        self.periodic_list = periodic_list;
        let async_list = AsynchronousList::new(&mut self.memory_space);
        async_list.set(self);
        self.asynchronous_list = async_list;
        let mut usbcmd = self.usbbase.usbcmd();
        usbcmd.set_interrupt_threshold_control(InterruptThresholdControl::MicroFrames32);
        usbcmd.set_periodic_schedule_enable(false);
        usbcmd.set_asynchronous_schedule_enable(false);

        self.start();
        //INFO: Must be set after fully configuring the host controller.
        self.usbbase.configflag().set_cf(true);
    }
}

// Utils
impl Ehci {
    /**
     *  1. If the Controller is running , stop it
     *  2. Reset the Controller by setting HCRESET
     */
    pub fn reset(&mut self) {
        self.stop();
        self.usbbase.usbcmd().set_hcreset(true);
        // Waits until the controller is resetted
        while self.usbbase.usbcmd().hcreset() {}
    }

    fn stop_async(&mut self) {
        self.usbbase
            .usbcmd()
            .set_asynchronous_schedule_enable(false);
        while self.usbbase.usbsts().asynchronous_schedule_status() {}
    }
    fn start_async(&mut self) {
        self.usbbase.usbcmd().set_asynchronous_schedule_enable(true);
        while !self.usbbase.usbsts().asynchronous_schedule_status() {}
    }
    fn start_periodic(&mut self) {
        self.usbbase.usbcmd().set_periodic_schedule_enable(true);
        while !self.usbbase.usbsts().periodic_schedule_status() {}
    }
}
