use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::{
        ehci::{
            Ehci,
            registers::{FrIndex, FrameListSize},
        },
        independent::Direction,
    },
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
    utils::memory::memset_dword,
};
#[derive(Clone, Copy)]
pub enum EndpointSpeed {
    FullSpeed,
    LowSpeed,
    HighSpeed,
}

impl EndpointSpeed {
    pub fn from_raw(val: u32) -> Self {
        return match val {
            0 => Self::FullSpeed,
            1 => Self::LowSpeed,
            2 => Self::HighSpeed,
            _ => simple_kernel_panic("EndpointSpeed/from_raw", "invalid value\n"),
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match *self {
            Self::FullSpeed => 0,
            Self::LowSpeed => 1,
            Self::HighSpeed => 2,
        };
    }
}

pub enum Mult {
    OneTransactionPerMicroframe,
    TwoTransactionPerMicroframe,
    ThreeTransactionPerMicroframe,
}

impl Mult {
    pub fn from_raw(value: u32) -> Mult {
        return match value {
            0b01 => Self::OneTransactionPerMicroframe,
            0b10 => Self::TwoTransactionPerMicroframe,
            0b11 => Self::ThreeTransactionPerMicroframe,
            _ => simple_kernel_panic("Mult/from_raw", "Invalid value\n"),
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::OneTransactionPerMicroframe => 1,
            Self::TwoTransactionPerMicroframe => 2,
            Self::ThreeTransactionPerMicroframe => 3,
        };
    }
}

pub struct QueueHeadHorizontalLinkPointer {
    address: *mut u32,
}

pub const QUEUE_HEAD_HORIZONTAL_LINK_POINTER_MASK: u32 = (1 << ((31 - 5) + 1)) - 1;

impl QueueHeadHorizontalLinkPointer {
    #[inline(always)]
    pub fn new(address: u64) -> QueueHeadHorizontalLinkPointer {
        return QueueHeadHorizontalLinkPointer {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    #[inline(always)]
    pub fn _type(&self) -> EhciCommonType0 {
        return EhciCommonType0::from_raw(unsafe { *self.address } >> 1);
    }
    #[inline(always)]
    pub fn link_pointer(&self) -> u32 {
        return unsafe { *self.address >> 5 } << 5;
    }
    #[inline(always)]
    pub fn set_terminate(&mut self, val: bool) {
        unsafe { *self.address = (*self.address & !1) | val as u32 };
    }
    #[inline(always)]
    pub fn set_type(&mut self, _type: EhciCommonType0) {
        unsafe { *self.address = (*self.address & !(0b11 << 1)) | _type.as_u32() << 1 };
    }
    #[inline(always)]
    pub fn set_link_pointer(&mut self, val: u32) {
        unsafe {
            *self.address =
                (*self.address & !(QUEUE_HEAD_HORIZONTAL_LINK_POINTER_MASK << 5)) | (val >> 5) << 5
        };
    }
}

pub struct AlternateNextqTDPointer {
    address: *mut u32,
}

impl AlternateNextqTDPointer {
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    #[inline(always)]
    pub fn nak_counter(&self) -> u8 {
        return ((unsafe { *self.address } >> 1) & 0b1111) as u8;
    }
}

pub struct QueueHeadBufferPointer {
    address: *mut u32,
}

impl QueueHeadBufferPointer {
    #[inline(always)]
    pub fn new(address: u64) -> QueueHeadBufferPointer {
        return QueueHeadBufferPointer {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn zero_out(&mut self) {
        unsafe { *self.address = 0 }
    }

    #[inline(always)]
    pub fn current_offset(&self) -> u16 {
        return ((unsafe { *self.address }) & 0b11111111_11111) as u16;
    }
    #[inline(always)]
    pub fn buffer_pointer(&self) -> u32 {
        return (unsafe { *self.address } >> 12) << 12;
    }

    #[inline(always)]
    pub fn c_prog_mask(&self) -> u8 {
        return (unsafe { *self.address } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn frame_tag(&self) -> u8 {
        return (unsafe { *self.address } & 0b11111) as u8;
    }
    #[inline(always)]
    pub fn s_bytes(&self) -> u8 {
        return (unsafe { *self.address >> 5 } & 0b1111111) as u8;
    }
}
#[derive(Clone, Copy)]
pub struct QueueHead {
    address: *mut u32,
}

pub const MAXIMUM_PACKET_LENGTH_MASK: u32 = (1u32 << ((26 - 16) + 1)) - 1;

impl QueueHead {
    pub const SIZE: u32 = 0x30;
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return QueueHead {
            address: address as *mut u32,
        };
    }

    pub fn high_speed_initialize(
        &mut self,
        endpoint_number: u8,
        device_address: u8,
        maximum_packet_size: u16,
        data_toggle_control: bool,
        next_qh: Option<&QueueHead>,
        next_qtd: &QueueElementTransferDescriptor,
        mult: Mult,
    ) {
        self.reset();
        self.set_endpoint_speed(EndpointSpeed::HighSpeed);
        self.set_endpoint_number(endpoint_number);
        self.set_device_address(device_address);
        self.set_maximum_packet_length(maximum_packet_size);
        self.set_data_toggle_control(data_toggle_control);
        self.set_mult(mult);

        if let Option::Some(qh) = next_qh {
            self.horizontal_link_pointer().set_type(EhciCommonType0::QH);
            self.horizontal_link_pointer()
                .set_link_pointer(qh.get_address());
            self.horizontal_link_pointer().set_terminate(false);
        } else {
            self.horizontal_link_pointer().set_terminate(true);
        }

        self.next_qtd_pointer()
            .set_transfer_element_pointer(next_qtd.get_address());
        self.next_qtd_pointer().set_terminate(false);
    }

    #[inline(always)]
    pub fn get_address(&self) -> u32 {
        return self.address as u32;
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        // 1´s written => terminate = 1
        unsafe {
            self.address.write_volatile(1);
            *self.address.add(1) = 0;
            *self.address.add(2) = 0;
            *self.address.add(3) = 0;
            *self.address.add(4) = 1;
            *self.address.add(5) = 1;
            *self.address.add(6) = 0;
            *self.address.add(7) = 0;
            *self.address.add(8) = 0;
            *self.address.add(9) = 0;
            *self.address.add(10) = 0;
            *self.address.add(11) = 0;
        }
    }

    #[inline(always)]
    pub fn horizontal_link_pointer(&self) -> QueueHeadHorizontalLinkPointer {
        return QueueHeadHorizontalLinkPointer::new(self.address as u64);
    }
    #[inline(always)]
    pub fn device_address(&self) -> u8 {
        return (unsafe { *self.address.add(1) } & 0b1111_111) as u8;
    }
    #[inline(always)]
    pub fn inactivate_on_next_transaction(&self) -> bool {
        return 1 == (unsafe { *self.address.add(1) } >> 7) & 1;
    }
    #[inline(always)]
    pub fn endpoint_number(&self) -> u8 {
        return ((unsafe { *self.address.add(1) } >> 8) & 0b1111) as u8;
    }
    #[inline(always)]
    pub fn endpoint_speed(&self) -> EndpointSpeed {
        return EndpointSpeed::from_raw((unsafe { *self.address.add(1) } >> 12) & 0b11);
    }
    #[inline(always)]
    pub fn data_toggle_control(&self) -> bool {
        return 1 == (unsafe { *self.address.add(1) } >> 14) & 1;
    }
    #[inline(always)]
    pub fn head_of_reclamation_list_flag(&self) -> bool {
        return 1 == (unsafe { *self.address.add(1) } >> 15) & 1;
    }
    #[inline(always)]
    pub fn maximum_packet_length(&self) -> u16 {
        return ((unsafe { *self.address.add(1) } >> 16) & MAXIMUM_PACKET_LENGTH_MASK) as u16;
    }
    #[inline(always)]
    pub fn control_endpoint_flag(&self) -> bool {
        return 1 == (unsafe { *self.address.add(1) } >> 27) & 1;
    }
    #[inline(always)]
    pub fn nak_count_reload(&self) -> u8 {
        return ((unsafe { *self.address.add(1) } >> 28) & 0b1111) as u8;
    }

    #[inline(always)]
    pub fn frame_s_mask(&self) -> u8 {
        return (unsafe { *self.address.add(2) } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn frame_c_mask(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 8) & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn hub_addr(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 16) & 0b1111111) as u8;
    }
    #[inline(always)]
    pub fn port_number(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 23) & 0b1111111) as u8;
    }
    #[inline(always)]
    pub fn mult(&self) -> Mult {
        return Mult::from_raw((unsafe { *self.address.add(2) } >> 30) & 0b11);
    }

    #[inline(always)]
    pub fn current_qtd_pointer(&self) -> u32 {
        return unsafe { *self.address.add(3) >> 5 } << 5;
    }

    #[inline(always)]
    pub fn next_qtd_pointer(&self) -> qTDPointer {
        return qTDPointer::new(unsafe { self.address.add(4) } as u64);
    }

    #[inline(always)]
    pub fn alternate_next_qtd_pointer(&self) -> AlternateNextqTDPointer {
        return AlternateNextqTDPointer::new(unsafe { self.address.add(5) } as u64);
    }

    #[inline(always)]
    pub fn status(&self) -> u8 {
        return (unsafe { *self.address.add(6) } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn pid_code(&self) -> PidCode {
        return PidCode::from_raw((unsafe { *self.address.add(6) } >> 8) & 0b11);
    }
    #[inline(always)]
    pub fn error_counter(&self) -> u8 {
        return ((unsafe { *self.address.add(6) } >> 10) & 0b11) as u8;
    }
    #[inline(always)]
    pub fn current_page(&self) -> u8 {
        return ((unsafe { *self.address.add(6) } >> 12) & 0b111) as u8;
    }
    #[inline(always)]
    pub fn interrupt_on_complete(&self) -> bool {
        return 1 == (unsafe { *self.address.add(6) } >> 15) & 1;
    }
    #[inline(always)]
    pub fn total_bytes_to_transfer(&self) -> u16 {
        return ((unsafe { *self.address.add(6) } >> 16) & TOTAL_BYTES_TO_TRANSFER_MASK) as u16;
    }
    #[inline(always)]
    pub fn data_toggle(&self) -> bool {
        return 1 == (unsafe { *self.address.add(6) } >> 31) & 1;
    }

    #[inline(always)]
    fn buffer_pointer(&self, page: u8) -> QueueHeadBufferPointer {
        return QueueHeadBufferPointer::new(unsafe { self.address.add(7 + page as usize) } as u64);
    }

    #[inline(always)]
    pub fn buffer_pointer0(&self) -> QueueHeadBufferPointer {
        return self.buffer_pointer(0);
    }
    #[inline(always)]
    pub fn buffer_pointer1(&self) -> QueueHeadBufferPointer {
        return self.buffer_pointer(1);
    }
    #[inline(always)]
    pub fn buffer_pointer2(&self) -> QueueHeadBufferPointer {
        return self.buffer_pointer(2);
    }
    #[inline(always)]
    pub fn buffer_pointer3(&self) -> QueueHeadBufferPointer {
        return self.buffer_pointer(3);
    }
    #[inline(always)]
    pub fn buffer_pointer4(&self) -> QueueHeadBufferPointer {
        return self.buffer_pointer(4);
    }

    #[inline(always)]
    pub fn set_status_bit(&mut self, bit: u8) {
        unsafe {
            self.address
                .add(6)
                .write(self.address.add(6).read() | (1 << bit) as u32)
        };
    }
    #[inline(always)]
    pub fn clear_status_bit(&mut self, bit: u8) {
        unsafe {
            self.address
                .add(6)
                .write(self.address.add(6).read() & !((1 << bit) as u32))
        };
    }
    #[inline(always)]
    pub fn set_device_address(&mut self, val: u8) {
        unsafe {
            self.address
                .add(1)
                .write((self.address.add(1).read() & !0b1111_111) | (val as u32) & 0b1111_111)
        };
    }
    #[inline(always)]
    pub fn set_inactive_on_next_transaction(&mut self, val: bool) {
        unsafe {
            self.address
                .add(1)
                .write((self.address.add(1).read() & !(1 << 7)) | (val as u32) << 7)
        };
    }
    #[inline(always)]
    pub fn set_endpoint_number(&mut self, endpoint_number: u8) {
        unsafe {
            self.address.add(1).write(
                (self.address.add(1).read() & !(0b1111 << 8))
                    | ((endpoint_number as u32) & 0b1111) << 8,
            );
        }
    }
    #[inline(always)]
    pub fn set_endpoint_speed(&mut self, endpoint_speed: EndpointSpeed) {
        unsafe {
            self.address.add(1).write(
                (self.address.add(1).read() & !(0b11 << 12)) | endpoint_speed.as_u32() << 12,
            );
        }
    }
    #[inline(always)]
    pub fn set_data_toggle_control(&mut self, val: bool) {
        unsafe {
            self.address
                .add(1)
                .write((self.address.add(1).read() & !(1 << 14)) | (val as u32) << 14);
        }
    }
    #[inline(always)]
    pub fn set_head_of_reclaimation_list_flag(&mut self, val: bool) {
        unsafe {
            self.address
                .add(1)
                .write((self.address.add(1).read() & !(1 << 15)) | (val as u32) << 15);
        }
    }
    #[inline(always)]
    pub fn set_maximum_packet_length(&mut self, length: u16) {
        unsafe {
            self.address.add(1).write(
                (self.address.add(1).read() & !(MAXIMUM_PACKET_LENGTH_MASK << 16))
                    | (length as u32) << 16,
            )
        };
    }
    #[inline(always)]
    pub fn set_endpoint_control_flag(&mut self, val: bool) {
        unsafe {
            self.address
                .add(1)
                .write((self.address.add(1).read() & !(1 << 27)) | (val as u32) << 27);
        }
    }
    #[inline(always)]
    pub fn set_nak_count_reload(&mut self, val: u8) {
        unsafe {
            self.address.add(1).write(
                (self.address.add(1).read() & !(0b1111 << 28)) | ((val as u32) & 0b1111) << 28,
            );
        }
    }

    #[inline(always)]
    pub fn set_frame_s_mask(&mut self, val: u8) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & !0b11111111) | val as u32);
        }
    }
    #[inline(always)]
    pub fn set_frame_c_mask(&mut self, val: u8) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & !(0b11111111 << 8)) | (val as u32) << 8);
        }
    }
    #[inline(always)]
    pub fn set_hub_addr(&mut self, val: u8) {
        unsafe {
            self.address.add(2).write(
                (self.address.add(2).read() & !(0b1111111 << 16))
                    | ((val as u32) & 0b1111111) << 16,
            );
        }
    }
    #[inline(always)]
    pub fn set_port_number(&mut self, val: u8) {
        unsafe {
            self.address.add(2).write(
                (self.address.add(2).read() & !(0b1111111 << 23))
                    | ((val as u32) & 0b1111111) << 23,
            );
        }
    }
    #[inline(always)]
    pub fn set_mult(&mut self, val: Mult) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & !(0b11 << 30)) | val.as_u32() << 30);
        }
    }
}

pub struct IsochronousTransferDescriptor {
    address: *mut u32,
}

pub struct IsochronousTransferDescriptorBuffer {
    address: *mut u32,
}
// Ehci Specification Table 3-6
pub const BUFFER_POINTER_MASK: u32 = (1 << ((31 - 12) + 1)) - 1;

impl IsochronousTransferDescriptorBuffer {
    #[inline(always)]
    pub fn new(address: u64) -> IsochronousTransferDescriptorBuffer {
        return IsochronousTransferDescriptorBuffer {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn buffer_pointer(&self) -> u32 {
        return ((unsafe { *self.address }) >> 12) << 12;
    }

    #[inline(always)]
    pub fn endpoint_number(&self) -> u8 {
        return ((unsafe { *self.address } >> 8) & 0b1111) as u8;
    }
    #[inline(always)]
    pub fn device_address(&self) -> u8 {
        return (unsafe { *self.address } & 0b1111111) as u8;
    }
    #[inline(always)]
    pub fn direction(&self) -> Direction {
        return Direction::from_bool(1 == unsafe { *self.address } >> 11);
    }
    #[inline(always)]
    pub fn maximum_packet_size(&self) -> u16 {
        return (unsafe { *self.address } & 0b11111111_111) as u16;
    }
    #[inline(always)]
    pub fn mult(&self) -> Mult {
        return Mult::from_raw(unsafe { *self.address } & 0b11);
    }
    #[inline(always)]
    pub fn set_buffer_pointer(&mut self, buffer_pointer: u32) {
        unsafe { *self.address = (*self.address & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer }
    }
    #[inline(always)]
    pub fn set_endpoint_number(&mut self, endpointer_number: u8) {
        unsafe {
            *self.address = (*self.address & !(0b1111 << 8)) | (endpointer_number as u32) << 8
        }
    }
    #[inline(always)]
    pub fn set_device_address(&mut self, device_address: u8) {
        unsafe { *self.address = (*self.address & !0b1111111) | (device_address as u32) }
    }

    #[inline(always)]
    pub fn set_direction(&mut self, direction: Direction) {
        unsafe { *self.address = (*self.address & !(1 << 11)) | direction.as_u32() << 11 }
    }

    #[inline(always)]
    pub fn set_maximum_packet_size(&mut self, maximum_packet_size: u16) {
        unsafe { *self.address = (*self.address & !(0b11111111_111)) | maximum_packet_size as u32 }
    }

    #[inline(always)]
    pub fn set_mult(&mut self, mult: Mult) {
        unsafe { *self.address = (*self.address & !(0b11)) | mult.as_u32() }
    }
}

pub struct IsochronousTransferDescriptorTransaction {
    address: *mut u32,
}

pub const TRANSACTION_OFFSET_MASK: u32 = 0b11111111_1111;
pub const TRANSACTION_LENGTH_MASK: u32 = 0b11111111_1111;

impl IsochronousTransferDescriptorTransaction {
    #[inline(always)]
    pub fn new(address: u64) -> IsochronousTransferDescriptorTransaction {
        return IsochronousTransferDescriptorTransaction {
            address: address as *mut u32,
        };
    }

    pub fn zero_out(&mut self) {
        unsafe { *self.address = 0 };
    }

    #[inline(always)]
    pub fn transaction_offset(&self) -> u16 {
        return (unsafe { *self.address } & TRANSACTION_OFFSET_MASK) as u16;
    }
    #[inline(always)]
    pub fn page_select(&self) -> u8 {
        return (unsafe { *self.address >> 12 } & 0b11) as u8;
    }
    #[inline(always)]
    pub fn interrupt_on_complete(&self) -> bool {
        return 1 == unsafe { *self.address } >> 15 & 1;
    }
    #[inline(always)]
    pub fn transaction_length(&self) -> u16 {
        return (unsafe { *self.address >> 16 } & TRANSACTION_LENGTH_MASK) as u16;
    }
    #[inline(always)]
    pub fn status(&self) -> u8 {
        return (unsafe { *self.address >> 28 } & 0b1111) as u8;
    }

    #[inline(always)]
    pub fn set_transaction_offset(&mut self, val: u16) {
        unsafe { (*self.address) = ((*self.address) & !TRANSACTION_OFFSET_MASK) | (val as u32) }
    }
    #[inline(always)]
    pub fn set_page_select(&mut self, val: u8) {
        unsafe { (*self.address) = ((*self.address) & !(0b11 << 12)) | (val as u32) << 12 }
    }
    #[inline(always)]
    pub fn set_interrupt_on_complete(&mut self, ioc: bool) {
        unsafe { (*self.address) = ((*self.address) & !(1 << 15)) | (ioc as u32) << 15 }
    }
    #[inline(always)]
    pub fn set_transaction_length(&mut self, val: u16) {
        unsafe {
            (*self.address) =
                ((*self.address) & !(TRANSACTION_LENGTH_MASK << 16)) | (val as u32) << 16
        }
    }

    //status = 0
    #[inline(always)]
    pub fn clear_status(&mut self) {
        unsafe { (*self.address) = (*self.address) & !(0b1111 << 28) };
    }
    //status.active = 1
    #[inline(always)]
    pub fn set_status(&mut self) {
        unsafe { (*self.address) = ((*self.address) & !(0b1111 << 28)) | (1 << 31) };
    }
}

impl IsochronousTransferDescriptor {
    #[inline(always)]
    pub fn new(address: u64) -> IsochronousTransferDescriptor {
        return IsochronousTransferDescriptor {
            address: address as *mut u32,
        };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            *self.address = 0;
            *self.address.add(1) = 0;
            *self.address.add(2) = 0;
            *self.address.add(3) = 0;
            *self.address.add(4) = 0;
            *self.address.add(5) = 0;
            *self.address.add(6) = 0;
            *self.address.add(7) = 0;
            *self.address.add(8) = 0;
            *self.address.add(9) = 0;
            *self.address.add(10) = 0;
            *self.address.add(11) = 0;
            *self.address.add(12) = 0;
            *self.address.add(13) = 0;
            *self.address.add(14) = 0;
            *self.address.add(15) = 0;
        }
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return unsafe { self.address.read() & 1 } == 1;
    }
    #[inline(always)]
    pub fn _type(&self) -> EhciCommonType0 {
        return EhciCommonType0::from_raw(unsafe { (self.address.read() >> 1) & 0b11 });
    }
    #[inline(always)]
    pub fn next_link_pointer(&self) -> u32 {
        return unsafe { self.address.read() >> 5 } << 5;
    }

    #[inline(always)]
    fn transaction(&self, index: u8) -> IsochronousTransferDescriptorTransaction {
        return IsochronousTransferDescriptorTransaction::new(unsafe {
            self.address.add(1 + index as usize) as u64
        });
    }

    #[inline(always)]
    pub fn transaction0(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(0)
    }
    #[inline(always)]
    pub fn transaction1(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(1)
    }
    #[inline(always)]
    pub fn transaction2(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(2)
    }
    #[inline(always)]
    pub fn transaction3(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(3)
    }
    #[inline(always)]
    pub fn transaction4(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(4)
    }
    #[inline(always)]
    pub fn transaction5(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(5)
    }
    #[inline(always)]
    pub fn transaction6(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(6)
    }
    #[inline(always)]
    pub fn transaction7(&self) -> IsochronousTransferDescriptorTransaction {
        self.transaction(7)
    }
    #[inline(always)]

    fn buffer_pointer(&self, page: u8) -> IsochronousTransferDescriptorBuffer {
        return IsochronousTransferDescriptorBuffer::new(unsafe {
            self.address.add(6 + page as usize) as u64
        });
    }

    #[inline(always)]
    pub fn buffer_pointer0(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(0)
    }
    #[inline(always)]
    pub fn buffer_pointer1(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(1)
    }
    #[inline(always)]
    pub fn buffer_pointer2(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(2)
    }
    #[inline(always)]
    pub fn buffer_pointer3(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(3)
    }
    #[inline(always)]
    pub fn buffer_pointer4(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(4)
    }
    #[inline(always)]
    pub fn buffer_pointer5(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(5)
    }
    #[inline(always)]
    pub fn buffer_pointer6(&self) -> IsochronousTransferDescriptorBuffer {
        self.buffer_pointer(6)
    }
}

pub struct SplitTransactionTransferDescriptorBackPointer {
    address: *mut u32,
}

impl SplitTransactionTransferDescriptorBackPointer {
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    pub fn back_pointer(&self) -> u32 {
        return unsafe { *self.address >> 5 } << 5;
    }
}

pub enum TransactionPosition {
    All,
    Begin,
    Mid,
    End,
}

impl TransactionPosition {
    pub fn from_raw(val: u32) -> Self {
        return match val {
            0 => Self::All,
            1 => Self::Begin,
            2 => Self::Mid,
            3 => Self::End,
            _ => simple_kernel_panic("TransactionPosition/from_raw", "Invalid value\n"),
        };
    }
}

//TODO: Implement this!
pub struct SplitTransactionTransferDescriptor {
    address: *mut u32,
}

impl SplitTransactionTransferDescriptor {
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            *self.address = 0;
            *self.address.add(1) = 0;
            *self.address.add(2) = 0;
            *self.address.add(3) = 0;
            *self.address.add(4) = 0;
            *self.address.add(5) = 0;
            *self.address.add(6) = 0;
        }
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return unsafe { self.address.read() & 1 } == 1;
    }
    #[inline(always)]
    pub fn _type(&self) -> EhciCommonType0 {
        return EhciCommonType0::from_raw(unsafe { (self.address.read() >> 1) & 0b11 });
    }
    #[inline(always)]
    pub fn next_link_pointer(&self) -> u32 {
        return unsafe { self.address.read() >> 5 } << 5;
    }

    #[inline(always)]
    pub fn device_address(&self) -> u8 {
        return (unsafe { *self.address.add(1) } & 0b1111_111) as u8;
    }
    #[inline(always)]
    pub fn endpoint_number(&self) -> u8 {
        return ((unsafe { *self.address.add(1) } >> 8) & 0b1111) as u8;
    }
    #[inline(always)]
    pub fn hub_address(&self) -> u8 {
        return ((unsafe { *self.address.add(1) } >> 16) & 0b1111_111) as u8;
    }
    #[inline(always)]
    pub fn port_number(&self) -> u8 {
        return ((unsafe { *self.address.add(1) } >> 24) & 0b1111_111) as u8;
    }
    #[inline(always)]
    pub fn direction(&self) -> Direction {
        return Direction::from_bool(1 == (unsafe { *self.address.add(1) } >> 31));
    }

    #[inline(always)]
    pub fn frame_s_mask(&self) -> u8 {
        return (unsafe { *self.address.add(2) } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn frame_c_mask(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 8) & 0b11111111) as u8;
    }

    #[inline(always)]
    pub fn status(&self) -> u8 {
        return (unsafe { *self.address.add(3) } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn frame_c_prog_mask(&self) -> u8 {
        return ((unsafe { *self.address.add(3) } >> 8) & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn total_bytes_to_transfer(&self) -> u16 {
        return ((unsafe { *self.address.add(3) } >> 16) & 0b11111111_11) as u16;
    }
    #[inline(always)]
    pub fn page_select(&self) -> bool {
        return 1 == unsafe { *self.address.add(3) } >> 30;
    }
    #[inline(always)]
    pub fn interrupt_on_complete(&self) -> bool {
        return 1 == unsafe { *self.address.add(3) } >> 31;
    }

    #[inline(always)]
    pub fn current_offset(&self) -> u16 {
        return (unsafe { *self.address.add(4) } & 0b11111111_1111) as u16;
    }
    #[inline(always)]
    pub fn buffer_pointer0(&self) -> u32 {
        return unsafe { *self.address.add(4) >> 12 } << 12;
    }

    #[inline(always)]
    pub fn transaction_count(&self) -> u8 {
        return (unsafe { *self.address.add(5) } & 0b111) as u8;
    }
    #[inline(always)]
    pub fn transaction_position(&self) -> TransactionPosition {
        return TransactionPosition::from_raw((unsafe { *self.address.add(5) } >> 3) & 0b11);
    }
    #[inline(always)]
    pub fn buffer_pointer1(&self) -> u32 {
        return unsafe { *self.address.add(5) >> 12 } << 12;
    }

    #[inline(always)]
    pub fn back_pointer(&self) -> SplitTransactionTransferDescriptorBackPointer {
        return SplitTransactionTransferDescriptorBackPointer::new(
            unsafe { self.address.add(6) } as u64
        );
    }
}

#[allow(non_camel_case_types)]
pub struct qTDPointer {
    address: *mut u32,
}

impl qTDPointer {
    pub const TRANSFER_ELEMENT_POINTER_MASK: u32 = (1 << ((31 - 5) + 1)) - 1;

    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }
    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    #[inline(always)]
    pub fn transfer_element_pointer(&self) -> u32 {
        return unsafe { *self.address >> 5 } << 5;
    }

    #[inline(always)]
    pub fn set_terminate(&mut self, val: bool) {
        unsafe { *self.address = (*self.address & (!1)) | val as u32 };
    }

    #[inline(always)]
    pub fn set_transfer_element_pointer(&self, ptr: u32) {
        unsafe {
            *self.address = (*self.address & !(qTDPointer::TRANSFER_ELEMENT_POINTER_MASK << 5))
                | (ptr >> 5) << 5;
        }
    }
}

pub enum PidCode {
    OutToken,
    InToken,
    SetupToken,
}

impl PidCode {
    pub fn from_raw(val: u32) -> Self {
        return match val {
            0 => Self::OutToken,
            1 => Self::InToken,
            2 => Self::SetupToken,
            _ => simple_kernel_panic("PidCode/from_raw", "Invalid val\n"),
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::OutToken => 0,
            Self::InToken => 1,
            Self::SetupToken => 2,
        };
    }
    // true  => In
    // false => Out
    pub fn inout_from_bool(val: bool) -> Self {
        return match val {
            true => Self::InToken,
            false => Self::OutToken,
        };
    }
}
pub const PID_CODE_MASK: u32 = 0b11 << 8;
pub const TOTAL_BYTES_TO_TRANSFER_MASK: u32 = (1 << ((30 - 16) + 1)) - 1;
pub const C_PAGE_MASK: u32 = 0b111;

pub struct QueueElementTransferDescriptor {
    address: *mut u32,
}

impl QueueElementTransferDescriptor {
    pub fn initialize(
        &mut self,
        next_qtd: Option<&QueueElementTransferDescriptor>,
        pid_code: PidCode,
        bytes_to_transfer: u16,
        ioc: bool,
    ) {
        self.reset();
        self.set_pid_code(pid_code);
        self.set_total_bytes_to_transfer(bytes_to_transfer);
        self.set_interrupt_on_completion(ioc);
        if let Option::Some(qtd) = next_qtd {
            self.next_qtd_pointer()
                .set_transfer_element_pointer(qtd.get_address());
            self.next_qtd_pointer().set_terminate(false);
        } else {
            self.next_qtd_pointer().set_terminate(true);
        }
    }

    #[inline(always)]
    pub fn new(address: u64) -> QueueElementTransferDescriptor {
        return Self {
            address: address as *mut u32,
        };
    }

    pub fn get_address(&self) -> u32 {
        return self.address as u32;
    }

    pub fn reset(&mut self) {
        unsafe { *self.address = 1 }; // Invalid
        unsafe { *self.address.add(1) = 1 }; // Invalid
        unsafe { *self.address.add(2) = 0b11 << 10 };
        unsafe { *self.address.add(3) = 0 };
        unsafe { *self.address.add(4) = 0 };
        unsafe { *self.address.add(5) = 0 };
        unsafe { *self.address.add(6) = 0 };
        unsafe { *self.address.add(7) = 0 };
    }

    #[inline(always)]
    pub fn next_qtd_pointer(&self) -> qTDPointer {
        return qTDPointer::new(self.address as u64);
    }

    #[inline(always)]
    pub fn alternate_next_qtd_pointer(&self) -> qTDPointer {
        return qTDPointer::new(unsafe { self.address.add(1) } as u64);
    }

    #[inline(always)]
    pub fn status(&self) -> u8 {
        return (unsafe { *self.address.add(2) } & 0b11111111) as u8;
    }
    #[inline(always)]
    pub fn pid_code(&self) -> PidCode {
        return PidCode::from_raw((unsafe { *self.address.add(2) >> 8 } & 0b11));
    }
    #[inline(always)]
    pub fn error_counter(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 10) & 0b11) as u8;
    }
    #[inline(always)]
    pub fn current_page(&self) -> u8 {
        return ((unsafe { *self.address.add(2) } >> 12) & 0b111) as u8;
    }
    #[inline(always)]
    pub fn interrupt_on_complete(&self) -> bool {
        return 1 == (unsafe { *self.address.add(2) } >> 15) & 1;
    }
    #[inline(always)]
    pub fn total_bytes_to_transfer(&self) -> u32 {
        return (unsafe { *self.address.add(2) } >> 16) & TOTAL_BYTES_TO_TRANSFER_MASK;
    }
    #[inline(always)]
    pub fn data_toggle(&self) -> bool {
        return 1 == (unsafe { *self.address.add(2) } >> 31) & 1;
    }

    #[inline(always)]
    pub fn current_offset(&self) -> u16 {
        return (unsafe { *self.address.add(3) } & 0b11111111_1111) as u16;
    }
    #[inline(always)]
    pub fn buffer_pointer0(&self) -> u32 {
        return (unsafe { *self.address.add(3) } >> 12) << 12;
    }
    #[inline(always)]
    pub fn buffer_pointer1(&self) -> u32 {
        return (unsafe { *self.address.add(4) } >> 12) << 12;
    }
    #[inline(always)]
    pub fn buffer_pointer2(&self) -> u32 {
        return (unsafe { *self.address.add(5) } >> 12) << 12;
    }
    #[inline(always)]
    pub fn buffer_pointer3(&self) -> u32 {
        return (unsafe { *self.address.add(6) } >> 12) << 12;
    }
    #[inline(always)]
    pub fn buffer_pointer4(&self) -> u32 {
        return (unsafe { *self.address.add(7) } >> 12) << 12;
    }

    #[inline(always)]
    pub fn set_pid_code(&mut self, pid_code: PidCode) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & (!PID_CODE_MASK)) | pid_code.as_u32() << 8);
        }
    }
    #[inline(always)]
    pub fn set_c_page(&mut self, c_page: u8) {
        unsafe {
            self.address.add(2).write(
                (self.address.add(2).read() & !(C_PAGE_MASK << 8))
                    | ((c_page as u32) & C_PAGE_MASK) << 8,
            );
        }
    }

    #[inline(always)]
    pub fn set_interrupt_on_completion(&mut self, ioc: bool) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & !(1 << 15)) | (ioc as u32) << 15);
        }
    }
    #[inline(always)]
    pub fn set_total_bytes_to_transfer(&mut self, bytes: u16) {
        unsafe {
            self.address.add(2).write(
                (self.address.add(2).read() & !(TOTAL_BYTES_TO_TRANSFER_MASK << 16))
                    | ((bytes as u32) & TOTAL_BYTES_TO_TRANSFER_MASK) << 16,
            );
        }
    }
    #[inline(always)]
    pub fn set_dt(&mut self, dt: bool) {
        unsafe {
            self.address
                .add(2)
                .write((self.address.add(2).read() & !(1 << 31)) | (dt as u32) << 31);
        }
    }
    #[inline(always)]
    pub fn set_current_offset(&mut self, current_offset: u16) {
        unsafe {
            self.address.add(3).write(
                ((self.address.add(3).read() >> 12) << 12) | (current_offset as u32) & 0xFFF,
            );
        }
    }

    #[inline(always)]
    pub fn set_buffer_pointer0(&mut self, buffer_pointer0: u32) {
        unsafe {
            self.address.add(3).write(
                (self.address.add(3).read() & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer0,
            );
        }
    }
    #[inline(always)]
    pub fn set_buffer_pointer1(&mut self, buffer_pointer1: u32) {
        unsafe {
            self.address.add(4).write(
                (self.address.add(4).read() & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer1,
            );
        }
    }
    #[inline(always)]
    pub fn set_buffer_pointer2(&mut self, buffer_pointer2: u32) {
        unsafe {
            self.address.add(5).write(
                (self.address.add(5).read() & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer2,
            );
        }
    }
    #[inline(always)]
    pub fn set_buffer_pointer3(&mut self, buffer_pointer3: u32) {
        unsafe {
            self.address.add(6).write(
                (self.address.add(6).read() & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer3,
            );
        }
    }
    #[inline(always)]
    pub fn set_buffer_pointer4(&mut self, buffer_pointer4: u32) {
        unsafe {
            self.address.add(7).write(
                (self.address.add(7).read() & !(BUFFER_POINTER_MASK << 12)) | buffer_pointer4,
            );
        }
    }

    #[inline(always)]
    pub fn set_status_bit(&mut self, bit: u8) {
        unsafe {
            self.address
                .add(2)
                .write(self.address.add(2).read() | (1 << bit) as u32);
        }
    }
    #[inline(always)]
    pub fn clear_status_bit(&mut self, bit: u8) {
        unsafe {
            self.address
                .add(2)
                .write(self.address.add(2).read() & !((1 << bit) as u32));
        }
    }
}

pub struct NormalPathLinkPointer {
    address: *mut u32,
}

impl NormalPathLinkPointer {
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    #[inline(always)]
    pub fn _type(&self) -> EhciCommonType0 {
        return EhciCommonType0::from_raw(unsafe { *self.address >> 1 } & 0b11);
    }
    #[inline(always)]
    pub fn link_pointer(&self) -> u32 {
        return unsafe { *self.address >> 5 } << 5;
    }

    #[inline(always)]
    pub fn set_terminate(&mut self, val: bool) {
        unsafe { *self.address = (*self.address & !1) | val as u32 };
    }
    #[inline(always)]
    pub fn set_type(&mut self, _type: EhciCommonType0) {
        unsafe { *self.address = (*self.address & !(0b11 << 1)) | _type.as_u32() << 1 };
    }
    #[inline(always)]
    pub fn set_link_pointer(&self, pointer: u32) {
        unsafe { *self.address = (*self.address & !(SET_LINK_POINTER_MASK << 5)) | pointer };
    }
}

pub struct BackPathLinkPointer {
    address: *mut u32,
}

pub const SET_LINK_POINTER_MASK: u32 = (1 << ((31 - 5) + 1)) - 1;

impl BackPathLinkPointer {
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn terminate(&self) -> bool {
        return 1 == unsafe { *self.address } & 1;
    }
    #[inline(always)]
    pub fn _type(&self) -> EhciCommonType0 {
        return EhciCommonType0::from_raw(unsafe { *self.address >> 1 } & 0b11);
    }
    #[inline(always)]
    pub fn link_pointer(&self) -> u32 {
        return unsafe { *self.address >> 5 } << 5;
    }

    #[inline(always)]
    pub fn set_terminate(&mut self, val: bool) {
        unsafe { *self.address = (*self.address & !1) | val as u32 };
    }
    #[inline(always)]
    pub fn set_type(&mut self, _type: EhciCommonType0) {
        unsafe { *self.address = (*self.address & !(0b11 << 1)) | _type.as_u32() << 1 };
    }
    #[inline(always)]
    pub fn set_link_pointer(&self, pointer: u32) {
        unsafe { *self.address = (*self.address & !(SET_LINK_POINTER_MASK << 5)) | pointer };
    }
}

pub struct PeriodicFrameSpanTraversalNode {
    address: *mut u32,
}

impl PeriodicFrameSpanTraversalNode {
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return Self {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn normal_path_link_pointer(&self) -> NormalPathLinkPointer {
        return NormalPathLinkPointer::new(self.address as u64);
    }
    #[inline(always)]
    pub fn back_path_link_pointer(&self) -> BackPathLinkPointer {
        return BackPathLinkPointer::new(unsafe { self.address.add(1) } as u64);
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
pub enum EhciCommonType0 {
    iTD,
    QH,
    siTD,
    FSTN,
}

impl EhciCommonType0 {
    pub fn from_raw(value: u32) -> Self {
        return match value {
            0b00 => Self::iTD,
            0b01 => Self::QH,
            0b10 => Self::siTD,
            0b11 => Self::FSTN,
            _ => simple_kernel_panic("FrameListElementPointerType/from_raw", "Invalid value\n"),
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::iTD => 0,
            Self::QH => 1,
            Self::siTD => 2,
            Self::FSTN => 3,
        };
    }
}

pub struct FrameListElementPointer {
    link_pointer: u32,
    terminate: bool,
    _type: EhciCommonType0,
}

impl FrameListElementPointer {
    pub fn from_raw(val: u32) -> FrameListElementPointer {
        let link_pointer = (val >> 5) << 5;
        let terminate = 1 == val & 1;
        let _type = EhciCommonType0::from_raw(val >> 1);
        return FrameListElementPointer {
            link_pointer,
            terminate,
            _type,
        };
    }

    #[inline(always)]
    pub fn get_type(&self) -> EhciCommonType0 {
        return self._type.clone();
    }

    #[inline(always)]
    pub fn get_terminate(&self) -> bool {
        return self.terminate;
    }
    #[inline(always)]
    pub fn get_link_pointer(&self) -> u32 {
        return self.link_pointer;
    }
}

pub struct PeriodicFrameList {
    address: *mut u32,
    frame_threshold: u16,
    frindex: FrIndex,
}

impl PeriodicFrameList {
    pub const fn empty() -> Self {
        return Self {
            address: null_mut(),
            frame_threshold: 0,
            frindex: FrIndex::empty(),
        };
    }
}

impl Default for PeriodicFrameList {
    fn default() -> Self {
        return Self {
            address: null_mut(),
            frame_threshold: 0,
            frindex: FrIndex::empty(),
        };
    }
}

impl PeriodicFrameList {
    pub const BASE_THRESHOLD: u16 = 6; // In micro-frames (so 1/2 of an iTD)

    pub fn new(allocator: &mut Allocator) -> PeriodicFrameList {
        let address = match allocator.alloc_zero(2) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("PeriodicFrameList/new", "Allocating failed\n"),
        };
        if address as u64 > 0xFFFFFFFF {
            simple_kernel_panic(
                "PeriodicFrameList/new",
                "Allocated address if above 32 bit limit\n",
            );
        }

        let periodic_frame_list = PeriodicFrameList {
            address,
            frame_threshold: 0,
            frindex: FrIndex::empty(),
        };

        unsafe {
            memset_dword(periodic_frame_list.address as *mut c_void, 1, 1024);
        }

        return periodic_frame_list;
    }
    pub fn set(&mut self, ehci: &mut Ehci) {
        self.frindex = ehci.usbbase.frindex().clone();

        // Ehci Specification - Chapter 3.1
        if ehci.usbbase.hccparams().programmable_frame_list_flag() {
            // also the default value
            ehci.usbbase
                .usbcmd()
                .set_frame_list_size(FrameListSize::Count1024);
        }

        let base = ehci.usbbase.hccparams().isochronous_scheduling_threshold() as u16;

        if base == 0 {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD;
        } else if base == 1 << 7 {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD + 16;
            // Chapter 4.7.2.1 says that if the current micro-frame % 8 == 7 => iTD can be added safetly after 2 Frames.
        } else if base & ((1 << 3) - 1) != 0 {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD + 2 + base & ((1 << 3) - 1);
            // Chapter 4.7.2.1. + 2 is just for safety
        }

        ehci.usbbase.frindex().set_frame_index(0);
        ehci.usbbase
            .periodiclistbase()
            .set_base_address(self.address as u32);
    }

    pub fn get_element(&self, pos: u16) -> FrameListElementPointer {
        let val = unsafe { *self.address.add(pos as usize) };
        return FrameListElementPointer::from_raw(val);
    }

    pub fn set_element(&self, frame: u16, qh_address: u32, _type: EhciCommonType0) {
        unsafe { *self.address.add(frame as usize) = qh_address | _type.as_u32() << 1 };
    }

    #[inline(always)]
    pub fn is_position_valid(&self, mut pos: u16) -> bool {
        // Chapter 4.7.2.1, pos is an iTD which takes 8 micro-frames
        pos *= 8;
        let frame = self.frindex.raw_frame_index();
        if frame > pos {
            return (0x3FFFu16 - frame) + pos >= self.frame_threshold;
        } else {
            return (pos - frame) >= self.frame_threshold;
        }
    }
}

pub struct AsynchronousList {
    base_address: *mut c_void,
}

impl AsynchronousList {
    pub const fn empty() -> Self {
        return Self {
            base_address: null_mut(),
        };
    }
}

impl Default for AsynchronousList {
    fn default() -> Self {
        return Self {
            base_address: null_mut(),
        };
    }
}

impl AsynchronousList {
    pub const NUMBER_OF_ENTRIES: u32 = 1024;
    pub const NUMBER_OF_BYTES: u32 = AsynchronousList::NUMBER_OF_ENTRIES * QueueHead::SIZE;
    pub const SPACING_BYTES_NEEDED: u32 = (AsynchronousList::NUMBER_OF_ENTRIES - 1) * 16;
    pub const DATA_BYTES_NEEDED: u32 = QueueHead::SIZE * AsynchronousList::NUMBER_OF_ENTRIES;
    pub fn new(allocator: &mut Allocator) -> AsynchronousList {
        let total_bytes =
            AsynchronousList::SPACING_BYTES_NEEDED + AsynchronousList::DATA_BYTES_NEEDED;
        let mut pages = 0;
        if total_bytes % 0x1000 != 0 {
            pages = 1;
        }
        pages += total_bytes / 0x1000;
        let base: *mut c_void = match allocator.alloc(pages as u16) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("AsynchronousList/new", "Allocating failed\n"),
        };
        if base as u64 > 0xFFFFFFFF {
            simple_kernel_panic(
                "AsynchronousList/new",
                "Allocated memory is above 32 bits\n",
            )
        }

        let ret = AsynchronousList { base_address: base };

        for i in 0..AsynchronousList::NUMBER_OF_ENTRIES {
            let mut qh = QueueHead::new(ret.address_of_index(i as u16) as u64);
            qh.reset();
            if i != AsynchronousList::NUMBER_OF_BYTES - 1 {
                qh.horizontal_link_pointer()
                    .set_link_pointer(ret.address_of_index(i as u16 + 1));
                qh.horizontal_link_pointer().set_terminate(false);
            } else {
                qh.horizontal_link_pointer()
                    .set_link_pointer(ret.address_of_index(0));
                qh.horizontal_link_pointer().set_terminate(false);
            }
            if i == 0 {
                qh.set_head_of_reclaimation_list_flag(true);
            }
            qh.horizontal_link_pointer().set_type(EhciCommonType0::QH);
        }

        return ret;
    }

    pub fn address_of_index(&self, index: u16) -> u32 {
        let inc: u32 = (16 * index as u32) * (index != 0) as u32;
        return self.base_address as u32 + (index as u32) * QueueHead::SIZE + inc;
    }

    pub fn set(&self, ehci: &Ehci) {
        ehci.usbbase
            .asynclistaddr()
            .set_address(self.address_of_index(0));
    }
}
