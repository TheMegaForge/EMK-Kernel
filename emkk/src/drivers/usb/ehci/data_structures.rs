use core::{ffi::c_void, ptr::null_mut};

use crate::{
    aml::Device,
    drivers::usb::{
        self,
        ehci::{
            Ehci,
            data_structures::{QueueHeadBitPart::T, QueueHeadPart::Type},
            registers::{
                AsyncListAddr, FrIndex, FrameListSize, HccParamsBitPart::ProgrammableFrameListFlag,
                HccParamsPart, UsbBase, UsbCmdPart,
            },
        },
        independent::Direction,
        ohci::structures::device,
    },
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
    utils::memory::memset_dword,
};
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum EndpointSpeed {
    FullSpeed = 0,
    LowSpeed = 1,
    HighSpeed = 2,
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
}
#[repr(u32)]
pub enum Mult {
    OneTransactionPerMicroframe = 1,
    TwoTransactionPerMicroframe = 2,
    ThreeTransactionPerMicroframe = 3,
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

#[repr(u32)]
pub enum QueueHeadPart {
    Type = (0x3 << 10) | (1 << 4) | 0,
    DeviceAddress = (0x3F << 10) | (0 << 4) | 1,
    /** Endpoint Number*/
    Endpt = (0xF << 10) | (8 << 4) | 1,
    /** Endpoint Speed*/
    Eps = (0x3 << 10) | (12 << 4) | 1,
    MaximumPacketLength = (0x7FF << 10) | (16 << 4) | 1,
    /** Nak Count Reload*/
    Nak = (0xF << 10) | (28 << 4) | 1,

    MikroFrameSMask = (0xFF << 10) | (0 << 4) | 2,
    MikroFrameCMask = (0xFF << 10) | (8 << 4) | 2,
    HubAddr = (0x7F << 10) | (16 << 4) | 2,
    PortNumber = (0x7F << 10) | (23 << 4) | 2,
    /** High-Bandwidth Pipe Multiplier*/
    Mult = (0x3 << 10) | (30 << 4) | 2,
    Status = (0xFF << 10) | (0 << 4) | 6,
    PidCode = (0x3 << 10) | (8 << 4) | 6,
    ErrorCounter = (0x3 << 10) | (10 << 4) | 6,
    CurrentPage = (0x7 << 10) | (12 << 4) | 6,
    TotalBytesToTransfer = (0x7FFFF << 10) | (16 << 4) | 6,
}
#[repr(u32)]
pub enum QueueHeadBitPart {
    /** Terminate ( for the Queue Head Horizontal Link Pointer) aka DWORD 0*/
    T = (0 << 16) | 0,
    /** Inactive on Next Transaction*/
    I = (7 << 16) | 1,
    /** Data Toggle Control*/
    Dtc = (14 << 16) | 1,
    /** Head of Reclamation List Flag*/
    H = (15 << 16) | 1,
    /** Control Endpoint Flag */
    C = (27 << 16) | 1,
    /** Interrupt on Complete */
    Ioc = (15 << 16) | 6,
    /** Data Toggle*/
    Dt = (31 << 16) | 6,
}

#[derive(Clone, Copy)]
pub struct QueueHead {
    val: *mut u32,
}

pub const MAXIMUM_PACKET_LENGTH_MASK: u32 = (1u32 << ((26 - 16) + 1)) - 1;

impl QueueHead {
    pub const SIZE: u32 = 0x30;
    #[inline(always)]
    pub fn new(address: u64) -> Self {
        return QueueHead {
            val: address as *mut u32,
        };
    }

    pub fn chain_next_qh(&mut self, address: u32) {
        self.set_part(Type, EhciLinkType::Qh as u32);
        self.set_horizontal_link_pointer(address);
        self.set(T, false);
    }

    pub fn set_common_info(
        &mut self,
        eps: EndpointSpeed,
        device_address: u8,
        maximum_packet_length: u16,
        endpoint_number: u8,
        mult: Mult,
    ) {
        self.set_part(QueueHeadPart::Eps, eps as u32);
        self.set_part(QueueHeadPart::DeviceAddress, device_address as u32);
        self.set_part(
            QueueHeadPart::MaximumPacketLength,
            maximum_packet_length as u32,
        );
        self.set_part(QueueHeadPart::Endpt, endpoint_number as u32);
        self.set_part(QueueHeadPart::Mult, mult as u32);
    }

    pub fn high_speed_initialize(
        &mut self,
        endpoint_number: u8,
        device_address: u8,
        maximum_packet_length: u16,
        data_toggle_control: bool,
        next_qh: Option<&QueueHead>,
        next_qtd: &QueueElementTransferDescriptor,
        mult: Mult,
    ) {
        self.reset();
        self.set_part(QueueHeadPart::Eps, EndpointSpeed::HighSpeed as u32);
        self.set_part(QueueHeadPart::Endpt, endpoint_number as u32);
        self.set_part(QueueHeadPart::DeviceAddress, device_address as u32);
        self.set_part(
            QueueHeadPart::MaximumPacketLength,
            maximum_packet_length as u32,
        );
        self.set(QueueHeadBitPart::Dtc, data_toggle_control);
        self.set_part(QueueHeadPart::Mult, mult as u32);

        if let Option::Some(qh) = next_qh {
            self.chain_next_qh(qh.get_address());
        } else {
            self.set(T, true);
        }

        self.next_qtd_pointer()
            .set_transfer_element_pointer(next_qtd.get_address());
        self.next_qtd_pointer().set_terminate(false);
    }

    pub fn is_set(&self, bit_part: QueueHeadBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: QueueHeadBitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn get_part(&self, part: QueueHeadPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: QueueHeadPart, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    #[inline(always)]
    pub fn get_address(&self) -> u32 {
        return self.val as u32;
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        // 1´s written => terminate = 1
        unsafe {
            self.val.write_volatile(1);
            *self.val.add(1) = 0;
            *self.val.add(2) = 0;
            *self.val.add(3) = 0;
            *self.val.add(4) = 1;
            *self.val.add(5) = 1;
            *self.val.add(6) = 0;
            *self.val.add(7) = 0;
            *self.val.add(8) = 0;
            *self.val.add(9) = 0;
            *self.val.add(10) = 0;
            *self.val.add(11) = 0;
        }
    }

    #[inline(always)]
    pub fn horizontal_link_pointer(&self) -> u32 {
        return unsafe { (self.val.add(0).read_volatile() >> 5) } << 5;
    }

    #[inline(always)]
    pub fn set_horizontal_link_pointer(&mut self, address: u32) {
        let mut val = unsafe { self.val.add(0).read_volatile() };
        val &= !(0x7FFFFFF << 5);
        val |= address & !0x1F;
        unsafe { self.val.add(0).write_volatile(val) }
    }

    #[inline(always)]
    pub fn current_qtd_pointer(&self) -> u32 {
        return unsafe { *self.val.add(3) >> 5 } << 5;
    }

    #[inline(always)]
    pub fn next_qtd_pointer(&self) -> qTDPointer {
        return qTDPointer::new(unsafe { self.val.add(4) } as u64);
    }

    #[inline(always)]
    pub fn alternate_next_qtd_pointer(&self) -> AlternateNextqTDPointer {
        return AlternateNextqTDPointer::new(unsafe { self.val.add(5) } as u64);
    }

    #[inline(always)]
    fn buffer_pointer(&self, page: u8) -> QueueHeadBufferPointer {
        return QueueHeadBufferPointer::new(unsafe { self.val.add(7 + page as usize) } as u64);
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
            self.val
                .add(6)
                .write(self.val.add(6).read() | (1 << bit) as u32)
        };
    }
    #[inline(always)]
    pub fn clear_status_bit(&mut self, bit: u8) {
        unsafe {
            self.val
                .add(6)
                .write(self.val.add(6).read() & !((1 << bit) as u32))
        };
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
        unsafe { *self.address = (*self.address & !(0b11)) | mult as u32 }
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
    pub fn _type(&self) -> EhciLinkType {
        return EhciLinkType::from_raw(unsafe { (self.address.read() >> 1) & 0b11 });
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
    pub fn _type(&self) -> EhciLinkType {
        return EhciLinkType::from_raw(unsafe { (self.address.read() >> 1) & 0b11 });
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
    pub fn _type(&self) -> EhciLinkType {
        return EhciLinkType::from_raw(unsafe { *self.address >> 1 } & 0b11);
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
    pub fn set_type(&mut self, _type: EhciLinkType) {
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
    pub fn _type(&self) -> EhciLinkType {
        return EhciLinkType::from_raw(unsafe { *self.address >> 1 } & 0b11);
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
    pub fn set_type(&mut self, _type: EhciLinkType) {
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

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum EhciLinkType {
    Itd = 0,
    Qh = 1,
    Sitd = 2,
    Fstn = 3,
}

impl EhciLinkType {
    pub fn from_raw(value: u32) -> Self {
        return match value {
            0b00 => Self::Itd,
            0b01 => Self::Qh,
            0b10 => Self::Sitd,
            0b11 => Self::Fstn,
            _ => simple_kernel_panic("FrameListElementPointerType/from_raw", "Invalid value\n"),
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::Itd => 0,
            Self::Qh => 1,
            Self::Sitd => 2,
            Self::Fstn => 3,
        };
    }
}

pub struct FrameListElementPointer {
    link_pointer: u32,
    terminate: bool,
    r#type: EhciLinkType,
}

impl FrameListElementPointer {
    pub fn from_raw(val: u32) -> FrameListElementPointer {
        let link_pointer = (val >> 5) << 5;
        let terminate = 1 == val & 1;
        let r#type = EhciLinkType::from_raw(val >> 1);
        return FrameListElementPointer {
            link_pointer,
            terminate,
            r#type,
        };
    }

    #[inline(always)]
    pub fn get_type(&self) -> EhciLinkType {
        return self.r#type.clone();
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

pub const ISOCHRONOUS_CHACHING_POSSIBLE: u16 = 1 << 3;
pub const EHC_ISOCHRONOUS_STATE_HOLD_IN_MICRO_FRAMES_MASK: u16 = 0x7; /* Section 2.2.4*/

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
    pub fn set(&mut self, usb_base: &mut UsbBase) {
        self.frindex = usb_base.frindex().clone();

        // Ehci Specification - Chapter 3.1
        if usb_base.hccparams().is_set(ProgrammableFrameListFlag) {
            // also the default value
            usb_base
                .usbcmd()
                .set_part(UsbCmdPart::FrameListSize, FrameListSize::Count1024 as u32);
        }

        let base = usb_base
            .hccparams()
            .get(HccParamsPart::IsochronousSchedulingThreshold) as u16;

        if base == 0 {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD;
        } else if base == ISOCHRONOUS_CHACHING_POSSIBLE {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD + 16;
            // Chapter 4.7.2.1 says that if the current micro-frame % 8 == 7 => iTD can be added safetly after 2 Frames.
        } else if base & EHC_ISOCHRONOUS_STATE_HOLD_IN_MICRO_FRAMES_MASK != 0 {
            self.frame_threshold = PeriodicFrameList::BASE_THRESHOLD + 2 + base
                & EHC_ISOCHRONOUS_STATE_HOLD_IN_MICRO_FRAMES_MASK;
            // Chapter 4.7.2.1. + 2 is just for safety
        }

        usb_base.frindex().set_frame_index(0);
        usb_base
            .periodiclistbase()
            .set_base_address(self.address as u32);
    }

    pub fn get_element(&self, pos: u16) -> FrameListElementPointer {
        let val = unsafe { *self.address.add(pos as usize) };
        return FrameListElementPointer::from_raw(val);
    }

    pub fn set_element(&self, frame: u16, qh_address: u32, _type: EhciLinkType) {
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
            if i == 0 {
                qh.set(QueueHeadBitPart::H, true);
            }

            if i != AsynchronousList::NUMBER_OF_ENTRIES - 1 {
                /*
                 *
                 */
                qh.chain_next_qh(ret.address_of_index(i as u16 + 1));
            } else {
                /* Original line was: qh.chain_next_qh(0)
                 */
                qh.chain_next_qh(ret.address_of_index(0));
            }
        }

        return ret;
    }

    pub fn address_of_index(&self, index: u16) -> u32 {
        let inc: u32 = (16 * index as u32) * (index != 0) as u32;
        return self.base_address as u32 + (index as u32) * QueueHead::SIZE + inc;
    }

    pub fn index_to_qh(&self, index: u16) -> QueueHead {
        let inc: u32 = (16 * index as u32) * (index != 0) as u32;
        let addr = self.base_address as u32 + (index as u32) * QueueHead::SIZE + inc;
        return QueueHead::new(addr as u64);
    }

    pub fn set(&self, addr: &mut AsyncListAddr) {
        addr.set_address(self.address_of_index(0));
    }
}
