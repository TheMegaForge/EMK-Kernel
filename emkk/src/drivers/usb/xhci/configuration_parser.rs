use core::{ffi::c_void, slice};

use crate::{
    aml::OperationRegion,
    drivers::usb::{
        independent::UsbDescriptorType,
        standard_requests::{
            UsbDefaultDescriptor, UsbEndpointDescriptor, UsbHID, UsbHIDDescriptor,
            UsbInterfaceDescriptor, UsbSuperSpeedDeviceCapabilityDescriptor,
            UsbSuperSpeedEndpointCompanionDescriptor,
        },
        traits::UsbConfiguration,
        xhci::structures::{
            endpoint::{XhciEndpoint, XhciEndpointDescriptor},
            interface::XhciInterface,
        },
    },
    hal::print::simple_kernel_panic,
    utils::{
        allocators::PageAllocator,
        slices::{change_mut_slice_size, invalid_mut_slice, resize_slice},
    },
};

pub struct XhciDeviceConfiguration {
    num_endpoints: u16,
    pub(in crate::drivers::usb::xhci) interfaces: &'static mut [XhciInterface],
    hid_interfaces: &'static mut [UsbHID],
    u1_exit_latency: Option<u8>,
    u2_exit_latency: Option<u16>,
}

impl UsbConfiguration for XhciDeviceConfiguration {
    fn get_hid_interface(
        &self,
        index: u8,
    ) -> Option<&crate::drivers::usb::standard_requests::UsbHID> {
        if index >= self.hid_interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&self.hid_interfaces[index as usize]);
    }
    fn get_hid_interface_count(&self) -> u8 {
        return self.hid_interfaces.len() as u8;
    }
    fn get_interface(&self, index: u8) -> Option<&dyn crate::drivers::usb::traits::UsbInterface> {
        if index >= self.interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&self.interfaces[index as usize]);
    }
    fn get_interface_count(&self) -> u8 {
        return self.interfaces.len() as u8;
    }
    fn get_mut_interface(
        &mut self,
        index: u8,
    ) -> Option<&mut dyn crate::drivers::usb::traits::UsbInterface> {
        if index >= self.interfaces.len() as u8 {
            return Option::None;
        }
        return Option::Some(&mut self.interfaces[index as usize]);
    }
}

impl XhciDeviceConfiguration {
    pub const fn empty() -> Self {
        return Self {
            num_endpoints: 0,
            interfaces: invalid_mut_slice(),
            hid_interfaces: invalid_mut_slice(),
            u1_exit_latency: Option::None,
            u2_exit_latency: Option::None,
        };
    }

    pub fn new(
        hid_descriptors: &'static mut [UsbHID],
        current_interfaces: &mut PageAllocator<XhciInterface>,
        current_endpoints: &mut PageAllocator<XhciEndpointDescriptor>,
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
            u1_exit_latency: Option::None,
            u2_exit_latency: Option::None,
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
            let interface = XhciInterface::from_raw(interface_descriptor, unsafe {
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
                let mut endpoint_descriptor =
                    XhciEndpointDescriptor::from_descriptor(endpoint_descriptor);
                data = unsafe { data.add(7) };
                let mut superspeed_endpoint_descriptor =
                    unsafe { &*(data as *const UsbSuperSpeedEndpointCompanionDescriptor) };
                if superspeed_endpoint_descriptor.b_descriptor_type == 48 {
                    endpoint_descriptor.superspeed_max_burst =
                        superspeed_endpoint_descriptor.b_max_burst;
                    endpoint_descriptor.superspeed_bm_attributes =
                        superspeed_endpoint_descriptor.bm_attributes;
                    data = unsafe { data.add(6) };
                }
                current_endpoints.push_back(endpoint_descriptor);
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
                0x10 /* Device Capability*/=> {
                    let capability_type = unsafe { *((descriptor as *const c_void).add(2) as *const u8) };
                    if capability_type == 3 {
                        let desc = unsafe { &*((descriptor as *const c_void).add(3) as *const UsbSuperSpeedDeviceCapabilityDescriptor) };
                        self.u1_exit_latency = Option::Some(desc.b_u1_dev_exit_lat);
                        self.u2_exit_latency = Option::Some(desc.w_u2_dev_exit_lat);
                    }
                    return unsafe { (*descriptor).b_length } as u16;
                }
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
