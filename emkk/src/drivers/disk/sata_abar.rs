use core::{ffi::c_void, ptr::null_mut, slice};

pub struct SataPortInterruptStatus<'a> {
    data: &'a mut u32,
}

impl<'a> SataPortInterruptStatus<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return SataPortInterruptStatus { data };
    }

    pub fn device_to_host_register_fis_interrupt(&self) -> bool {
        return 1 == *self.data & 1;
    }
    pub fn clear_device_to_register_fis_interrupt(&mut self) {
        *self.data |= 1;
    }
    pub fn pio_setup_fis_interrupt(&self) -> bool {
        return 1 == (*self.data >> 1) & 1;
    }
    pub fn clear_pio_setup_fis_interrupt(&mut self) {
        *self.data |= 1 << 1;
    }
    pub fn dma_setup_fis_interrupt(&self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn clear_dma_setup_fis_interrupt(&mut self) {
        *self.data |= 1 << 2;
    }
    /* __ since the actual bit is called ' Set Device Bits Interrupt'*/
    pub fn __set_device_bits_interrupt(&self) -> bool {
        return 1 == (*self.data >> 3) & 1;
    }
    pub fn clear_set_device_bits_interrupt(&mut self) {
        *self.data |= 1 << 3;
    }
    pub fn unkown_fis_interrupt(&self) -> bool {
        return 1 == (*self.data >> 4) & 1;
    }
    pub fn descriptor_processed(&self) -> bool {
        return 1 == (*self.data >> 5) & 1;
    }
    pub fn port_connect_change_status(&self) -> bool {
        return 1 == (*self.data >> 6) & 1;
    }
    pub fn device_mechanical_presence_status(&self) -> bool {
        return 1 == (*self.data >> 7) & 1;
    }
    pub fn clear_device_mechanical_presence_status(&mut self) {
        *self.data |= 1 << 7;
    }
    pub fn phyrdy_change_status(&self) -> bool {
        return 1 == (*self.data >> 22) & 1;
    }
    pub fn clear_phyrdy_change_status(&mut self) {
        *self.data |= 1 << 22;
    }
    pub fn incorrect_port_multiplier_status(&self) -> bool {
        return 1 == (*self.data >> 23) & 1;
    }
    pub fn clear_incorrect_port_multiplier_status(&mut self) {
        *self.data |= 1 << 23;
    }
    pub fn overflow_status(&self) -> bool {
        return 1 == (*self.data >> 24) & 1;
    }
    pub fn clear_overflow_status(&mut self) {
        *self.data |= 1 << 24;
    }
    pub fn interface_non_fatal_error_status(&self) -> bool {
        return 1 == (*self.data >> 26) & 1;
    }
    pub fn clear_interface_non_fatal_error_status(&mut self) {
        *self.data |= 1 << 26;
    }
    pub fn interface_fatal_error_status(&self) -> bool {
        return 1 == (*self.data >> 27) & 1;
    }
    pub fn clear_interface_fatal_error_status(&mut self) {
        *self.data |= 1 << 27;
    }
    pub fn host_bus_data_error_status(&self) -> bool {
        return 1 == (*self.data >> 28) & 1;
    }
    pub fn clear_host_bus_data_error_status(&mut self) {
        *self.data |= 1 << 28;
    }
    pub fn host_bus_fatal_error_status(&self) -> bool {
        return 1 == (*self.data >> 29) & 1;
    }
    pub fn clear_host_bus_fatal_error_status(&mut self) {
        *self.data |= 1 << 29;
    }
    pub fn task_file_error_status(&self) -> bool {
        return 1 == (*self.data >> 30) & 1;
    }
    pub fn clear_task_file_error_status(&mut self) {
        *self.data |= 1 << 30;
    }
    pub fn cold_port_detect_status(&self) -> bool {
        return 1 == (*self.data >> 31) & 1;
    }
    pub fn clear_cold_port_detect_status(&mut self) {
        *self.data |= 1 << 31;
    }
}

pub struct SataPortInterruptEnable<'a> {
    data: &'a mut u32,
}

impl<'a> SataPortInterruptEnable<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }
    pub fn set_device_to_host_register_fis_interrupt_enable(&mut self) {
        *self.data |= 1;
    }
    pub fn set_pio_setup_fis_interrupt_enable(&mut self) {
        *self.data |= 1 << 1;
    }
    pub fn set_dma_setup_fis_interrupt_enable(&mut self) {
        *self.data |= 1 << 2;
    }
    pub fn set_set_device_bits_fis_interrupt_enable(&mut self) {
        *self.data |= 1 << 3;
    }
    pub fn set_unknown_fis_interrupt_enable(&mut self) {
        *self.data |= 1 << 4;
    }
    pub fn set_descriptor_processed_interrupt_enable(&mut self) {
        *self.data |= 1 << 5;
    }
    pub fn set_port_change_interrupt_enable(&mut self) {
        *self.data |= 1 << 7;
    }
    pub fn set_phyrdy_change_interrupt_enable(&mut self) {
        *self.data |= 1 << 22;
    }
    pub fn set_incorrect_port_multiplier_enable(&mut self) {
        *self.data |= 1 << 23;
    }
    pub fn set_overflow_enable(&mut self) {
        *self.data |= 1 << 24;
    }
    pub fn set_interface_non_fatel_error_enable(&mut self) {
        *self.data |= 1 << 26;
    }
    pub fn set_interface_fatal_error_enable(&mut self) {
        *self.data |= 1 << 27;
    }
    pub fn set_host_bus_data_error_enable(&mut self) {
        *self.data |= 1 << 28;
    }
    pub fn set_host_bus_fatal_error_enable(&mut self) {
        *self.data |= 1 << 29;
    }
    pub fn set_task_file_error_enable(&mut self) {
        *self.data |= 1 << 30;
    }
    pub fn cold_presence_detect_enable(&self) -> bool {
        return 1 == (*self.data >> 31) & 1;
    }
    pub fn set_cold_presence_detect_enable(&mut self) {
        *self.data |= 1 << 31;
    }
}

pub struct SataPortCommandAndStatus<'a> {
    data: &'a mut u32,
}

impl<'a> SataPortCommandAndStatus<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }
    pub fn start(&self) -> bool {
        return 1 == (*self.data) & 1;
    }
    pub fn set_start(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }
    pub fn spin_up_device(&self) -> bool {
        return 1 == (*self.data >> 1) & 1;
    }
    pub fn set_spin_up_device(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 1)) | (val as u32) << 1
    }
    pub fn power_on_device(&self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn set_power_on_device(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 2)) | (val as u32) << 2;
    }
    pub fn command_list_override(&self) -> bool {
        return 1 == (*self.data >> 3) & 1;
    }
    pub fn set_command_list_override(&mut self) {
        *self.data |= 1 << 3;
    }
    pub fn fis_receive_enable(&self) -> bool {
        return 1 == (*self.data >> 4) & 1;
    }
    pub fn set_fis_receive_enable(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 4)) | (val as u32) << 4;
    }
    pub fn current_command_slot(&self) -> u8 {
        return ((*self.data >> 8) & 0b11111) as u8;
    }
    pub fn mechanical_presence_switch_state(&self) -> bool {
        return 1 == (*self.data >> 13) & 1;
    }
    pub fn fis_receive_running(&self) -> bool {
        return 1 == (*self.data >> 14) & 1;
    }
    pub fn command_list_running(&self) -> bool {
        return 1 == (*self.data >> 15) & 1;
    }
    pub fn cold_presence_state(&self) -> bool {
        return 1 == (*self.data >> 16) & 1;
    }
    pub fn port_multiplier_attached(&self) -> bool {
        return 1 == (*self.data >> 17) & 1;
    }
    pub fn set_port_multiplier_attached(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 17)) | (val as u32) << 17;
    }
    pub fn hot_plug_capable_port(&self) -> bool {
        return 1 == (*self.data >> 18) & 1;
    }
    pub fn mechanical_presence_switch_attached_to_port(&self) -> bool {
        return 1 == (*self.data >> 19) & 1;
    }
    pub fn cold_presence_detection(&self) -> bool {
        return 1 == (*self.data >> 20) & 1;
    }
    pub fn external_sata_port(&self) -> bool {
        return 1 == (*self.data >> 21) & 1;
    }
    pub fn fis_based_switchting_capable_port(&self) -> bool {
        return 1 == (*self.data >> 22) & 1;
    }
    pub fn automatic_partial_to_slumber_transitions_enabled(&self) -> bool {
        return 1 == (*self.data >> 23) & 1;
    }
    pub fn set_automatic_partial_to_slumber_transitions_enabled(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 23)) | (val as u32) << 23;
    }
    pub fn device_is_atapi(&self) -> bool {
        return 1 == (*self.data >> 24) & 1;
    }
    pub fn drive_led_on_atapi_enable(&self) -> bool {
        return 1 == (*self.data >> 25) & 1;
    }
    pub fn aggressive_link_power_management_enable(&self) -> bool {
        return 1 == (*self.data >> 26) & 1;
    }
    pub fn set_aggressive_link_power_management_enable(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 26)) | (val as u32) << 26;
    }
    pub fn aggressive_slumber_partial(&self) -> bool {
        return 1 == (*self.data >> 27) & 1;
    }
    pub fn set_aggressive_slumber_partial(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 27)) | (val as u32) << 27;
    }
    pub fn interface_communication_control(&self) -> u8 {
        return ((*self.data >> 28) & 0b1111) as u8;
    }
    pub fn set_interface_communication_control(&mut self, mut val: u8) {
        val &= 0b1111;
        *self.data = (*self.data & !(4 << 28)) | (val as u32) << 28;
    }
}

pub struct SataTaskFileData<'a> {
    data: &'a u32,
}

impl<'a> SataTaskFileData<'a> {
    pub fn new(data: &'a u32) -> Self {
        return Self { data };
    }
    pub fn status(&self) -> u8 {
        return (*self.data & 0xFF) as u8;
    }
    pub fn error(&self) -> u8 {
        return ((*self.data >> 8) & 0xFF) as u8;
    }
}

pub struct SerialAtaStatus<'a> {
    data: &'a u32,
}

impl<'a> SerialAtaStatus<'a> {
    pub fn new(data: &'a u32) -> Self {
        return Self { data };
    }
    pub fn device_detection(&self) -> u8 {
        return (*self.data & 0b1111) as u8;
    }
    pub fn current_interface_speed(&self) -> u8 {
        return ((*self.data >> 4) & 0b1111) as u8;
    }
    pub fn interface_power_management(&self) -> u8 {
        return ((*self.data >> 8) & 0b1111) as u8;
    }
}

pub struct SerialAtaControl<'a> {
    data: &'a mut u32,
}

impl<'a> SerialAtaControl<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }
    pub fn device_detection_initialization(&self) -> u8 {
        return (*self.data & 0b1111) as u8;
    }
    pub fn set_device_detection_initialization(&mut self, val: u8) {
        *self.data = (*self.data & (!0b1111)) | (val as u32);
    }
    pub fn speed_allowed(&self) -> u8 {
        return ((*self.data >> 4) & 0b1111) as u8;
    }
    pub fn set_speed_allowed(&mut self, val: u8) {
        *self.data = (*self.data & !(4 << 4)) | (val as u32) << 4;
    }
    pub fn interface_power_management_transitions_allowed(&self) -> u8 {
        return ((*self.data >> 8) & 0b1111) as u8;
    }
    pub fn set_interface_power_management_transitions_allowed(&mut self, val: u8) {
        *self.data = (*self.data & !(4 << 8)) | (val as u32) << 8;
    }
    pub fn select_power_management(&self) -> u8 {
        return ((*self.data >> 12) & 0b1111) as u8;
    }
    pub fn port_multiplier_port(&self) -> u8 {
        return ((*self.data >> 16) & 0b1111) as u8;
    }
}

pub struct SerialAtaError<'a> {
    data: &'a mut u32,
}

impl<'a> SerialAtaError<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }

    pub fn clear_all(&mut self) {
        *self.data = 0xFFFFFFFF;
    }

    pub fn is_error_bit_set(&self, mut bit: u8) -> bool {
        bit &= 0b1111;
        return 1 == (*self.data >> bit) & 1;
    }
    pub fn clear_error_bit(&mut self, mut bit: u8) {
        bit &= 0b1111;
        *self.data = *self.data & !(1 << bit);
    }
    pub fn is_diagnostics_bit_set(&self, mut bit: u8) -> bool {
        bit &= 0b1111;
        return 1 == (*self.data >> (16 + bit)) & 1;
    }
    pub fn clear_diagnostics_bit(&mut self, mut bit: u8) {
        bit &= 0b1111;
        *self.data = *self.data & !(1 << (16 + bit));
    }
}

pub struct SataFisBasedSwitchingControl<'a> {
    data: &'a mut u32,
}

impl<'a> SataFisBasedSwitchingControl<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }
    pub fn set_enable(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }
    pub fn set_device_error_clear(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 1)) | (val as u32) << 1;
    }
    pub fn single_device_error(&mut self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn device_to_issue(&self) -> u8 {
        return ((*self.data >> 8) & 0b1111) as u8;
    }
    pub fn set_device_to_issue(&mut self, val: u8) {
        *self.data = (*self.data & !(4 << 8)) | (val as u32) << 8;
    }
    pub fn active_device_optimization(&self) -> u8 {
        return ((*self.data >> 12) & 0b1111) as u8;
    }
    pub fn device_with_error(&self) -> u8 {
        return ((*self.data >> 16) & 0b1111) as u8;
    }
}

pub struct DeviceSleep<'a> {
    data: &'a mut u32,
}

impl<'a> DeviceSleep<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }
    pub fn aggressive_device_sleep_enable(&self) -> bool {
        return 1 == *self.data & 1;
    }
    pub fn set_aggressive_device_sleep_enable(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }
    pub fn device_sleep_present(&self) -> bool {
        return 1 == (*self.data >> 1) & 1;
    }
    pub fn device_sleep_exit_timeout(&self) -> u8 {
        return ((*self.data >> 2) & 0b11111111) as u8;
    }
    pub fn set_device_sleep_exit_timeout(&mut self, val: u8) {
        *self.data = (*self.data & !(8 << 2)) | (val as u32) << 2;
    }
    pub fn minimum_device_sleep_assertion_time(&self) -> u8 {
        return (((*self.data) >> 10) & 0b11111) as u8;
    }
    pub fn set_minimum_device_sleep_assertion_time(&mut self, val: u8) {
        *self.data = (*self.data & !(5 << 10)) | (val as u32) << 10;
    }
    pub fn device_sleep_idle_timeout(&self) -> u16 {
        return ((*self.data >> 15) & 0b11111111111) as u16;
    }
    pub fn set_device_sleep_idle_timeout(&mut self, val: u16) {
        *self.data = (*self.data & !(11 << 15)) | (val as u32) << 15;
    }
    pub fn dito_multiplier(&self) -> u8 {
        return (((*self.data) >> 25) & 0b1111) as u8;
    }
}

pub struct SataPort {
    data: &'static mut [u32],
}

impl SataPort {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { slice::from_raw_parts_mut((ptr as *mut u32).as_mut().unwrap(), 32) },
        };
    }

    pub fn set_clb(&mut self, val: u32) {
        self.data[0] = val;
    }
    pub fn set_clbu(&mut self, val: u32) {
        self.data[1] = val;
    }
    pub fn set_fb(&mut self, val: u32) {
        self.data[2] = val;
    }
    pub fn set_fbu(&mut self, val: u32) {
        self.data[3] = val;
    }
    pub fn is<'a>(&'a mut self) -> SataPortInterruptStatus<'a> {
        return SataPortInterruptStatus::new(&mut self.data[4]);
    }
    pub fn ie<'a>(&'a mut self) -> SataPortInterruptEnable<'a> {
        return SataPortInterruptEnable::new(&mut self.data[5]);
    }
    pub fn cmd<'a>(&'a mut self) -> SataPortCommandAndStatus<'a> {
        return SataPortCommandAndStatus::new(&mut self.data[6]);
    }
    pub fn tfd<'a>(&'a mut self) -> SataTaskFileData<'a> {
        return SataTaskFileData::new(&self.data[8]);
    }
    pub fn signature(&self) -> u32 {
        return self.data[9];
    }
    pub fn ssts<'a>(&'a self) -> SerialAtaStatus<'a> {
        return SerialAtaStatus::new(&self.data[10]);
    }
    pub fn sctl<'a>(&'a mut self) -> SerialAtaControl<'a> {
        return SerialAtaControl::new(&mut self.data[11]);
    }
    pub fn serr<'a>(&'a mut self) -> SerialAtaError<'a> {
        return SerialAtaError::new(&mut self.data[12]);
    }

    pub fn serial_ata_active(&self) -> u32 {
        return self.data[13];
    }
    pub fn set_ata_active(&mut self, bit: u8) {
        self.data[13] |= 1 << bit;
    }
    pub fn command_issue(&self) -> u32 {
        return self.data[14];
    }
    pub fn set_command_issue(&mut self, bit: u8) {
        self.data[14] |= 1 << bit;
    }
    pub fn serial_ata_notification(&self) -> u32 {
        return self.data[15];
    }
    pub fn clear_serial_ata_notification(&mut self, bit: u8) {
        self.data[15] |= 1 << bit;
    }
    pub fn fbs<'a>(&'a mut self) -> SataFisBasedSwitchingControl<'a> {
        return SataFisBasedSwitchingControl::new(&mut self.data[16]);
    }
    pub fn devslp<'a>(&'a mut self) -> DeviceSleep<'a> {
        return DeviceSleep::new(&mut self.data[17]);
    }
}

pub struct SataHBACapabilities {
    data: &'static u32,
}

impl SataHBACapabilities {
    pub fn new(ptr: *const c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *const u32).as_ref().unwrap() },
        };
    }

    pub fn number_of_ports(&self) -> u8 {
        return (*self.data & 0b11111) as u8;
    }

    pub fn supports_external_sata(&self) -> bool {
        return 1 == (*self.data >> 5) & 1;
    }
    pub fn enclsoure_management_supported(&self) -> bool {
        return 1 == (*self.data >> 6) & 1;
    }
    pub fn command_completion_coalescing_supported(&self) -> bool {
        return 1 == (*self.data >> 7) & 1;
    }
    pub fn number_of_command_slots(&self) -> u8 {
        return ((*self.data >> 8) & 0b11111) as u8;
    }
    pub fn partial_state_capable(&self) -> bool {
        return 1 == (*self.data >> 13) & 1;
    }
    pub fn slumber_state_capable(&self) -> bool {
        return 1 == (*self.data >> 14) & 1;
    }
    pub fn pio_multiple_drq_block(&self) -> bool {
        return 1 == (*self.data >> 15) & 1;
    }
    pub fn fis_based_switching_supported(&self) -> bool {
        return 1 == (*self.data >> 16) & 1;
    }
    pub fn supports_port_multiplier(&self) -> bool {
        return 1 == (*self.data >> 17) & 1;
    }
    pub fn supports_ahci_mode_only(&self) -> bool {
        return 1 == (*self.data >> 18) & 1;
    }
    pub fn interface_speed_support(&self) -> u8 {
        return ((*self.data >> 20) & 0b1111) as u8;
    }
    pub fn supports_command_list_override(&self) -> bool {
        return 1 == (*self.data >> 24) & 1;
    }
    pub fn supports_activity_led(&self) -> bool {
        return 1 == (*self.data >> 25) & 1;
    }
    pub fn supports_aggressive_link_power_management(&self) -> bool {
        return 1 == (*self.data >> 26) & 1;
    }
    pub fn supports_staggered_spin_up(&self) -> bool {
        return 1 == (*self.data >> 27) & 1;
    }
    pub fn supports_mechanical_presence_switch(&self) -> bool {
        return 1 == (*self.data >> 28) & 1;
    }
    pub fn supports_snotification_register(&self) -> bool {
        return 1 == (*self.data >> 29) & 1;
    }
    pub fn supports_nmative_command_queuing(&self) -> bool {
        return 1 == (*self.data >> 30) & 1;
    }
    pub fn supports_64bit_addressing(&self) -> bool {
        return 1 == (*self.data >> 31) & 1;
    }
}

pub struct SataGlobalHBAControl {
    data: &'static mut u32,
}

impl SataGlobalHBAControl {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *mut u32).as_mut().unwrap() },
        };
    }
    pub fn set_hba_reset(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }

    pub fn hba_reset(&self) -> bool {
        return 1 == (*self.data & 1);
    }

    pub fn set_interrupt_enable(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 1)) | (val as u32) << 1;
    }
    pub fn msi_revert_to_single_message(&self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn set_ahci_enable(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 31)) | (val as u32) << 31;
    }
}

pub struct SataCommandCompletionCoalescingControl {
    data: &'static mut u32,
}

impl SataCommandCompletionCoalescingControl {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *mut u32).as_mut().unwrap() },
        };
    }
    pub fn set_enable(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }
    pub fn interrupt(&self) -> u8 {
        return ((*self.data >> 3) & 0b11111) as u8;
    }
    pub fn set_command_completions(&mut self, mut val: u8) {
        val &= 0b11111111;
        *self.data = (*self.data & !(0b11111111 << 8)) | ((val as u32) << 8);
    }
    pub fn set_timeout_value(&mut self, tv: u16) {
        *self.data = (*self.data & !(0xFFFF << 16)) | ((tv as u32) << 16);
    }
}

pub struct SataEnclosureManagementLocation {
    data: &'static mut u32,
}

impl SataEnclosureManagementLocation {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *mut u32).as_mut().unwrap() },
        };
    }
    pub fn buffer_size(&self) -> u16 {
        return (*self.data & 0xFFFF) as u16;
    }
    pub fn offset(&self) -> u16 {
        return ((*self.data >> 16) & 0xFFFF) as u16;
    }
}

pub struct SataEnclosureManagementControl {
    data: &'static mut u32,
}

impl SataEnclosureManagementControl {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *mut u32).as_mut().unwrap() },
        };
    }

    pub fn message_received(&self) -> bool {
        return 1 == *self.data & 1;
    }

    pub fn clear_message_received(&mut self) {
        *self.data |= 1;
    }

    pub fn set_transmite_message(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 8)) | (val as u32) << 8;
    }

    pub fn set_reset(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 9)) | (val as u32) << 9;
    }
    pub fn led_message_types(&self) -> bool {
        return 1 == *self.data & (1 << 16);
    }
    pub fn saf_te_enclosure_management_messages(&self) -> bool {
        return 1 == (*self.data >> 17) & 1;
    }
    pub fn ses_2_enclosure_management_messages(&self) -> bool {
        return 1 == (*self.data >> 18) & 1;
    }
    pub fn sgpio_enclosure_managment_messages(&self) -> bool {
        return 1 == (*self.data >> 19) & 1;
    }
    pub fn single_messsage_buffer(&self) -> bool {
        return 1 == (*self.data >> 24) & 1;
    }
    pub fn transmit_only(&self) -> bool {
        return 1 == (*self.data >> 25) & 1;
    }
    pub fn activity_led_hardware_driven(&self) -> bool {
        return 1 == (*self.data >> 26) & 1;
    }
    pub fn port_multiplier_support(&self) -> bool {
        return 1 == (*self.data >> 27) & 1;
    }
}

pub struct SataHBACapabilitiesExtended {
    data: &'static u32,
}

impl SataHBACapabilitiesExtended {
    pub fn new(ptr: *const c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *const u32).as_ref().unwrap() },
        };
    }
    pub fn bios_os_handoff(&self) -> bool {
        return 1 == (*self.data) & 1;
    }
    pub fn nvmhci_present(&self) -> bool {
        return 1 == (*self.data >> 1) & 1;
    }
    pub fn automatic_partial_to_slumber_transitions(&self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn supports_device_sleep(&self) -> bool {
        return 1 == (*self.data >> 3) & 1;
    }
    pub fn supports_aggressive_device_sleep_management(&self) -> bool {
        return 1 == (*self.data >> 5) & 1;
    }
}

#[allow(non_camel_case_types)]
pub struct SataBIOS_OSHandoffControlAndStatus {
    data: &'static mut u32,
}

impl SataBIOS_OSHandoffControlAndStatus {
    pub fn new(ptr: *mut c_void) -> Self {
        return Self {
            data: unsafe { (ptr as *mut u32).as_mut().unwrap() },
        };
    }

    pub fn bios_owned_semaphore(&self) -> bool {
        return 1 == (*self.data) & 1;
    }

    pub fn os_owned_semaphore(&self) -> bool {
        return 1 == (*self.data >> 1) & 1;
    }

    pub fn set_os_owned_semaphore(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 1)) | (val as u32) << 1;
    }

    pub fn smi_on_os_ownership_change_enable(&self) -> bool {
        return 1 == (*self.data >> 2) & 1;
    }
    pub fn set_smi_on_os_ownership_change_enable(&mut self, val: bool) {
        *self.data = (*self.data & !(1 << 2)) | (val as u32) << 2;
    }
    pub fn os_ownership_change(&self) -> bool {
        return 1 == (*self.data >> 3) & 1;
    }
    pub fn clear_os_ownership_change(&mut self) {
        *self.data |= 1 << 3;
    }
    pub fn bios_busy(&self) -> bool {
        return 1 == (*self.data >> 4) & 1;
    }
}

pub struct Abar {
    ptr: *mut c_void,
}

impl Abar {
    pub const fn empty() -> Self {
        return Self { ptr: null_mut() };
    }

    pub fn new(ptr: *mut c_void) -> Self {
        return Self { ptr };
    }

    pub fn cap(&self) -> SataHBACapabilities {
        return SataHBACapabilities::new(self.ptr);
    }
    pub fn ghc(&self) -> SataGlobalHBAControl {
        return SataGlobalHBAControl::new(unsafe { self.ptr.add(0x04) });
    }
    pub fn is(&self) -> u32 {
        return unsafe { *(self.ptr.add(0x08) as *const u32) };
    }
    pub fn pi(&self) -> u32 {
        return unsafe { *(self.ptr.add(0x0C) as *const u32) };
    }
    pub fn vs(&self) -> u32 {
        return unsafe { *(self.ptr.add(0x10) as *const u32) };
    }
    pub fn ccc_ctl(&self) -> SataCommandCompletionCoalescingControl {
        return SataCommandCompletionCoalescingControl::new(unsafe { self.ptr.add(0x14) });
    }
    pub fn ccc_ports(&self) -> u32 {
        return unsafe { *(self.ptr.add(0x18) as *const u32) };
    }
    pub fn em_loc(&self) -> SataEnclosureManagementLocation {
        return SataEnclosureManagementLocation::new(unsafe { self.ptr.add(0x1C) });
    }
    pub fn em_ctl(&self) -> SataEnclosureManagementControl {
        return SataEnclosureManagementControl::new(unsafe { self.ptr.add(0x20) });
    }
    pub fn cap2(&self) -> SataHBACapabilitiesExtended {
        return SataHBACapabilitiesExtended::new(unsafe { self.ptr.add(0x24) });
    }
    pub fn bohc(&self) -> SataBIOS_OSHandoffControlAndStatus {
        return SataBIOS_OSHandoffControlAndStatus::new(unsafe { self.ptr.add(0x24) });
    }

    pub fn port(&self, index: u8) -> SataPort {
        return SataPort::new(unsafe { self.ptr.add(0x100 + (0x80 * index as usize)) });
    }
}
