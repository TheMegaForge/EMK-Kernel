use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::{
        ehci::{
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
    utils::{allocators::PageAllocator, traits::Region},
};

pub struct EhciDeviceConfiguration {
    num_interfaces: u16,
    num_endpoints: u16,
    interfaces: *mut EhciInterface,
    hid_interfaces: *mut UsbHID,
    num_hid_interfaces: u8,
}

impl UsbConfiguration for EhciDeviceConfiguration {
    fn get_hid_interface(&self, index: u8) -> Option<&UsbHID> {
        if index > self.num_hid_interfaces {
            return Option::None;
        }
        return unsafe { self.hid_interfaces.add(index as usize).as_ref() };
    }
    fn get_interface(&self, index: u8) -> Option<&dyn UsbInterface> {
        if index > self.num_interfaces as u8 {
            return Option::None;
        }

        unsafe {
            let ptr = self.interfaces.add(index as usize);
            let obj = &*ptr;
            return Option::Some(obj as &dyn UsbInterface);
        }
    }
    fn get_interface_count(&self) -> u8 {
        return self.num_interfaces as u8;
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
            num_interfaces,
            num_endpoints: 0,
            interfaces: current_interfaces
                .as_mut_ptr(current_interfaces.size())
                .unwrap(),
            hid_interfaces,
            num_hid_interfaces: 0,
        };
        for i in 0..num_interfaces {
            let interface_descriptor =
                unsafe { (data as *const UsbInterfaceDescriptor).as_ref().unwrap() };
            data = unsafe { data.add(9) };
            let peeked_descriptor = data as *const UsbDefaultDescriptor;
            if unsafe { (*peeked_descriptor).b_descriptor_type } != ENDPOINT_DESCRIPTOR_TYPE {
                data = unsafe {
                    data.add(ret.handle_special_descriptor(peeked_descriptor, i as u8) as usize)
                };
            }
            current_interfaces.push_back(EhciInterface::new(
                interface_descriptor.b_num_endpoints,
                current_endpoints
                    .as_mut_ptr(current_endpoints.size())
                    .unwrap(),
                interface_descriptor.b_interface_class,
                interface_descriptor.b_interface_sub_class,
                interface_descriptor.b_interface_protocol,
                interface_descriptor.i_interface,
            ));
            ret.num_endpoints += interface_descriptor.b_num_endpoints as u16;
            for _ep in 0..interface_descriptor.b_num_endpoints {
                let endpoint_descriptor =
                    unsafe { (data as *const UsbEndpointDescriptor).as_ref().unwrap() };

                let qtd_base = match allocator.alloc(2) {
                    Ok(mb) => mb.as_mut_ptr(),
                    Err(_e) => simple_kernel_panic(
                        "EhciDeviceConfiguration/new",
                        "Could not allocate qtd_base for endpoint\n",
                    ),
                };

                current_endpoints.push_back(EhciEndpoint::full_new(
                    endpoint_descriptor.b_endpoint_address & 0b1111,
                    256,
                    qtd_base,
                    QueueHead::new(0),
                    endpoint_descriptor.b_endpoint_address,
                    endpoint_descriptor.bm_attributes,
                    endpoint_descriptor.w_max_packet_size,
                    endpoint_descriptor.b_interval,
                ));
                data = unsafe { data.add(7) };
            }
        }
        if ret.num_hid_interfaces == 0 {
            match allocator.free(&MemoryBlock::new(0x1000, ret.hid_interfaces as u64)) {
                Ok(_) => {}
                Err(_e) => simple_kernel_panic(
                    "EhciDeviceConfiguration/new",
                    "Could not free hid interfaces\n",
                ),
            };
            ret.hid_interfaces = null_mut();
        }
        return ret;
    }

    pub fn get_interfaces(&self) -> *const EhciInterface {
        return self.interfaces;
    }

    fn handle_special_descriptor(
        &mut self,
        descriptor: *const UsbDefaultDescriptor,
        interface_index: u8,
    ) -> u16 {
        match unsafe { (*descriptor).b_descriptor_type } {
            0x21 /* HID*/ => {
                let hid_descriptor = unsafe {(descriptor as *const UsbHIDDescriptor).as_ref().unwrap()};
                let hid = unsafe {self.hid_interfaces.add(self.num_hid_interfaces as usize).as_mut().unwrap()};
                *hid = UsbHID {
                    bcd_hid: hid_descriptor.bcd_hid,
                    country_code: hid_descriptor.b_country_code,
                    num_descriptors: hid_descriptor.b_num_descriptors,
                    descriptor_type: hid_descriptor.b_descriptor_type1,
                    descriptor_length: hid_descriptor.w_descriptor_length,
                    interface_index
                };
                self.num_hid_interfaces += 1;
                return 9;
            }
            _ => simple_kernel_panic(
                "EhciDeviceConfiguration/handle_special_descriptor",
                "Unhandled b_descriptor_type\n",
            ),
        }
    }
}
