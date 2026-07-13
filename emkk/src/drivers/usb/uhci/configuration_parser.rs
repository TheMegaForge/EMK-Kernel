use core::{ffi::c_void, slice};

use crate::{
    drivers::usb::{
        independent::UsbDescriptorType,
        standard_requests::{
            UsbDefaultDescriptor, UsbEndpointDescriptor, UsbHID, UsbHIDDescriptor,
            UsbInterfaceDescriptor,
        },
        traits::{UsbConfiguration, UsbInterface},
        uhci::{
            data_structures::RawUhciTransferDescriptor,
            structures::{
                endpoint::{UhciControlEndpoint, UhciGeneralEndpoint},
                interface::UhciInterface,
            },
        },
    },
    hal::print::simple_kernel_panic,
    utils::{
        allocators::PageAllocator,
        slices::{change_mut_slice_size, invalid_mut_slice, resize_slice},
    },
};

pub struct UhciDeviceConfiguration {
    num_endpoints: u16,
    pub(in crate::drivers::usb::uhci) interfaces: &'static mut [UhciInterface],
    hid_interfaces: &'static mut [UsbHID],
}

impl UhciDeviceConfiguration {
    pub fn empty() -> Self {
        return Self {
            num_endpoints: 0,
            interfaces: invalid_mut_slice(),
            hid_interfaces: invalid_mut_slice(),
        };
    }
}

impl UsbConfiguration for UhciDeviceConfiguration {
    fn get_hid_interface(&self, index: u8) -> Option<&UsbHID> {
        if index >= self.hid_interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&self.hid_interfaces[index as usize]);
    }
    fn get_hid_interface_count(&self) -> u8 {
        return self.hid_interfaces.len() as u8;
    }
    fn get_interface(&self, index: u8) -> Option<&dyn UsbInterface> {
        if index >= self.interfaces.len() as u8 {
            return Option::None;
        }

        return Option::Some(&self.interfaces[index as usize]);
    }
    fn get_mut_interface(&mut self, index: u8) -> Option<&mut dyn UsbInterface> {
        if index >= self.interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&mut self.interfaces[index as usize]);
    }
    fn get_interface_count(&self) -> u8 {
        return self.interfaces.len() as u8;
    }
}

impl UhciDeviceConfiguration {
    pub fn new(
        hid_descriptors: &'static mut [UsbHID],
        current_interfaces: &mut PageAllocator<UhciInterface>,
        current_endpoints: &mut PageAllocator<UhciGeneralEndpoint>,
        mut data: *const c_void,
        num_interfaces: u16,
    ) -> Self {
        let mut ret = Self {
            num_endpoints: 0,
            interfaces: current_interfaces
                .whole_as_mut_slice(current_interfaces.size())
                .unwrap(),
            hid_interfaces: unsafe {
                slice::from_raw_parts_mut(hid_descriptors.as_mut_ptr() as *mut UsbHID, 0)
            },
        };
        resize_slice(&mut ret.interfaces, num_interfaces as usize);

        for i in 0..num_interfaces {
            let interface_descriptor =
                unsafe { (data as *const UsbInterfaceDescriptor).as_ref().unwrap() };
            data = unsafe { data.add(9) };
            let peeked_descriptor = unsafe { &*(data as *const UsbDefaultDescriptor) };
            if peeked_descriptor.b_descriptor_type != UsbDescriptorType::Endpoint as u8 {
                data = unsafe {
                    data.add(ret.handle_special_descriptor(peeked_descriptor, i as u8) as usize)
                };
            }
            let interface = UhciInterface::from_raw(interface_descriptor, unsafe {
                change_mut_slice_size(
                    current_endpoints
                        .whole_as_mut_slice(current_endpoints.size())
                        .unwrap(),
                    interface_descriptor.b_num_endpoints as usize,
                )
            });
            current_interfaces.push_back(interface);
            ret.num_endpoints += interface_descriptor.b_num_endpoints as u16;
            for _ep in 0..interface_descriptor.b_num_endpoints {
                let endpoint_descriptor = unsafe { &*(data as *const UsbEndpointDescriptor) };
                current_endpoints.push_back(UhciGeneralEndpoint::from_raw(endpoint_descriptor));
                data = unsafe { data.add(7) };
            }
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
                    "UhciDeviceConfiguration/handle_special_descriptor",
                    "Unhandled b_descriptor_type\n",
                ),
            }
    }
}
