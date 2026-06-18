use core::{
    ffi::c_void,
    ptr::{self, null_mut, slice_from_raw_parts_mut},
    slice,
};

use crate::{
    drivers::usb::{
        ehci::{
            Ehci,
            data_structures::QueueHead,
            structures::{endpoint::EhciEndpoint, interface::EhciInterface},
        },
        independent::ENDPOINT_DESCRIPTOR_TYPE,
        standard_requests::{
            UsbDefaultDescriptor, UsbEndpointDescriptor, UsbHID, UsbHIDDescriptor,
            UsbInterfaceDescriptor,
        },
        traits::{UsbConfiguration, UsbInterface},
    },
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
    utils::{allocators::PageAllocator, invalid_mut_slice, resize_slice, traits::Region},
};

pub struct EhciDeviceConfiguration {
    num_endpoints: u16,
    interfaces: &'static mut [EhciInterface],
    hid_interfaces: &'static mut [UsbHID],
}

impl UsbConfiguration for EhciDeviceConfiguration {
    fn get_hid_interface(&self, index: u8) -> Option<&UsbHID> {
        if index > self.hid_interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&self.hid_interfaces[index as usize]);
    }
    fn get_interface(&self, index: u8) -> Option<&dyn UsbInterface> {
        if index > self.interfaces.len() as u8 {
            return Option::None;
        }

        return Option::Some(&self.interfaces[index as usize]);
    }
    fn get_mut_interface(&mut self, index: u8) -> Option<&mut dyn UsbInterface> {
        if index > self.interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&mut self.interfaces[index as usize]);
    }
    fn get_interface_count(&self) -> u8 {
        return self.interfaces.len() as u8;
    }
}

impl EhciDeviceConfiguration {
    pub fn new(
        allocator: &mut Allocator,
        current_interfaces: &mut PageAllocator<EhciInterface>,
        current_endpoints: &mut PageAllocator<EhciEndpoint>,
        mut data: *const c_void,
        num_interfaces: u16,
    ) -> Self {
        let hid_interfaces = match allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic(
                "EhciDeviceConfiguration/new",
                "Could not allocate memory for hid interfaces\n",
            ),
        };

        let mut ret = Self {
            num_endpoints: 0,
            interfaces: unsafe {
                slice::from_raw_parts_mut(
                    current_interfaces
                        .as_mut_ptr(current_interfaces.size())
                        .unwrap(),
                    num_interfaces as usize,
                )
            },
            hid_interfaces: unsafe { slice::from_raw_parts_mut(hid_interfaces as *mut UsbHID, 0) },
        };

        for i in 0..num_interfaces {
            let interface_descriptor =
                unsafe { (data as *const UsbInterfaceDescriptor).as_ref().unwrap() };
            data = unsafe { data.add(9) };
            let peeked_descriptor = unsafe { &*(data as *const UsbDefaultDescriptor) };
            if peeked_descriptor.b_descriptor_type != ENDPOINT_DESCRIPTOR_TYPE {
                data = unsafe {
                    data.add(ret.handle_special_descriptor(peeked_descriptor, i as u8) as usize)
                };
            }

            let interface = EhciInterface::from_raw(interface_descriptor, unsafe {
                slice::from_raw_parts_mut(
                    current_endpoints
                        .as_mut_ptr(current_endpoints.size())
                        .unwrap(),
                    interface_descriptor.b_num_endpoints as usize,
                )
            });
            current_interfaces.push_back(interface);
            ret.num_endpoints += interface_descriptor.b_num_endpoints as u16;
            for _ep in 0..interface_descriptor.b_num_endpoints {
                let endpoint_descriptor = unsafe { &*(data as *const UsbEndpointDescriptor) };

                let qtd_base = match allocator.alloc(2) {
                    Ok(mb) => mb.as_mut_ptr(),
                    Err(_e) => simple_kernel_panic(
                        "EhciDeviceConfiguration/new",
                        "Could not allocate qtd_base for endpoint\n",
                    ),
                };

                current_endpoints.push_back(EhciEndpoint::full_new_from_raw(
                    endpoint_descriptor,
                    256,
                    qtd_base,
                    QueueHead::new(0),
                ));
                data = unsafe { data.add(7) };
            }
        }
        if ret.hid_interfaces.len() == 0 {
            match allocator.free(&MemoryBlock::new(
                0x1000,
                ret.hid_interfaces.as_mut_ptr().addr() as u64,
            )) {
                Ok(_) => {}
                Err(_e) => simple_kernel_panic(
                    "EhciDeviceConfiguration/new",
                    "Could not free hid interfaces\n",
                ),
            };
            ret.hid_interfaces = invalid_mut_slice();
        }
        return ret;
    }

    fn handle_special_descriptor(
        &mut self,
        descriptor: *const UsbDefaultDescriptor,
        interface_index: u8,
    ) -> u16 {
        match unsafe { (*descriptor).b_descriptor_type } {
            0x21 /* HID*/ => {
                let hid_descriptor = unsafe {&* (descriptor as *const UsbHIDDescriptor)};
                let new_size = self.hid_interfaces.len() + 1;
                resize_slice(&mut self.hid_interfaces, new_size);
                self.hid_interfaces[self.hid_interfaces.len() - 1] =
                    UsbHID {
                        bcd_hid: hid_descriptor.bcd_hid,
                        country_code: hid_descriptor.b_country_code,
                        num_descriptors: hid_descriptor.b_num_descriptors,
                        descriptor_type: hid_descriptor.b_descriptor_type1,
                        descriptor_length: hid_descriptor.w_descriptor_length,
                        interface_index
                };
                return 9;
            }
            _ => simple_kernel_panic(
                "EhciDeviceConfiguration/handle_special_descriptor",
                "Unhandled b_descriptor_type\n",
            ),
        }
    }
}
