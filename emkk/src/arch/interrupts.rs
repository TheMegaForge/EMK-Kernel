use crate::{
    arch::gdt::KERNEL_CODE_SEGMENT,
    hal::{
        memory::allocator::{Allocator, MemoryBlock, VirtualAllocator},
        print::{Module, simple_kernel_panic},
    },
    info,
    multithreading::processors::HOST_CORE_ACTIVATION_ID,
    success,
    utils::{io_wait, memory::memcpy, outb, slices::invalid_mut_slice, traits::Region},
};
use core::{arch::asm, ffi::c_void, ptr::null_mut, slice};
#[repr(packed, C)]
pub struct IdtDescriptor {
    size: u16,
    offset: u64,
}

#[repr(C)]
struct IdtEntry {
    offset_0: u16,
    segment: u16,
    ist: u8,
    //P+DPL+0+GateType
    properties: u8,
    offset_1: u16,
    offset_2: u32,
    reserved_0: u32,
}

pub const PIC1: u16 = 0x20;
pub const PIC2: u16 = 0xA0;
pub const PIC1_DATA: u16 = PIC1 + 1;
pub const PIC2_DATA: u16 = PIC2 + 1;

pub fn deactivate_interrupts() {
    unsafe {
        asm!("cli");
        outb(PIC1_DATA, 0xFF);
        io_wait();
        outb(PIC2_DATA, 0xFF);
        io_wait();
    };
    let mut module = Module::new("Interrupts");
    info!(module, "Deactivated\n");
}

pub fn activate_interrupts(module: &mut Module) {
    unsafe { asm!("sti") };
    info!(module, "activated interrupts\n");
}
unsafe extern "C" {
    fn rce_divide();
    fn rce_debug();
    fn rce_nmi();
    fn rce_breakpoint();
    fn rce_overflow();
    fn rce_bound();
    fn rce_invalid_opcode();
    fn rce_device_not_available();
    fn rce_double_fault();
    fn rce_coprocessor_overrun();
    fn rce_invalid_tss();
    fn rce_segment_not_present();
    fn rce_stack_segment();
    fn rce_general_protection();
    fn rce_page_fault();
    fn rce_x87_floating();
    fn rce_alignment();
    fn rce_machine_check();
    fn rce_simd_floating();
    fn rce_virtualization();
    fn rce_control_protection();

    fn isr32();
    fn isr33();
    fn isr34();
    fn isr35();
    fn isr36();
    fn isr37();
    fn isr38();
    fn isr39();
    fn isr40();
    fn isr41();
    fn isr42();
    fn isr43();
    fn isr44();
    fn isr45();
    fn isr46();
    fn isr47();
    fn isr48();
    fn isr49();
    fn isr50();
    fn isr51();
    fn isr52();
    fn isr53();
    fn isr54();
    fn isr55();
    fn isr56();
    fn isr57();
    fn isr58();
    fn isr59();
    fn isr60();
    fn isr61();
    fn isr62();
    fn isr63();
    fn isr64();
    fn isr65();
    fn isr66();
    fn isr67();
    fn isr68();
    fn isr69();
    fn isr70();
    fn isr71();
    fn isr72();
    fn isr73();
    fn isr74();
    fn isr75();
    fn isr76();
    fn isr77();
    fn isr78();
    fn isr79();
    fn isr80();
    fn isr81();
    fn isr82();
    fn isr83();
    fn isr84();
    fn isr85();
    fn isr86();
    fn isr87();
    fn isr88();
    fn isr89();
    fn isr90();
    fn isr91();
    fn isr92();
    fn isr93();
    fn isr94();
    fn isr95();
    fn isr96();
    fn isr97();
    fn isr98();
    fn isr99();
    fn isr100();
    fn isr101();
    fn isr102();
    fn isr103();
    fn isr104();
    fn isr105();
    fn isr106();
    fn isr107();
    fn isr108();
    fn isr109();
    fn isr110();
    fn isr111();
    fn isr112();
    fn isr113();
    fn isr114();
    fn isr115();
    fn isr116();
    fn isr117();
    fn isr118();
    fn isr119();
    fn isr120();
    fn isr121();
    fn isr122();
    fn isr123();
    fn isr124();
    fn isr125();
    fn isr126();
    fn isr127();
    fn isr128();
    fn isr129();
    fn isr130();
    fn isr131();
    fn isr132();
    fn isr133();
    fn isr134();
    fn isr135();
    fn isr136();
    fn isr137();
    fn isr138();
    fn isr139();
    fn isr140();
    fn isr141();
    fn isr142();
    fn isr143();
    fn isr144();
    fn isr145();
    fn isr146();
    fn isr147();
    fn isr148();
    fn isr149();
    fn isr150();
    fn isr151();
    fn isr152();
    fn isr153();
    fn isr154();
    fn isr155();
    fn isr156();
    fn isr157();
    fn isr158();
    fn isr159();
    fn isr160();
    fn isr161();
    fn isr162();
    fn isr163();
    fn isr164();
    fn isr165();
    fn isr166();
    fn isr167();
    fn isr168();
    fn isr169();
    fn isr170();
    fn isr171();
    fn isr172();
    fn isr173();
    fn isr174();
    fn isr175();
    fn isr176();
    fn isr177();
    fn isr178();
    fn isr179();
    fn isr180();
    fn isr181();
    fn isr182();
    fn isr183();
    fn isr184();
    fn isr185();
    fn isr186();
    fn isr187();
    fn isr188();
    fn isr189();
    fn isr190();
    fn isr191();
    fn isr192();
    fn isr193();
    fn isr194();
    fn isr195();
    fn isr196();
    fn isr197();
    fn isr198();
    fn isr199();
    fn isr200();
    fn isr201();
    fn isr202();
    fn isr203();
    fn isr204();
    fn isr205();
    fn isr206();
    fn isr207();
    fn isr208();
    fn isr209();
    fn isr210();
    fn isr211();
    fn isr212();
    fn isr213();
    fn isr214();
    fn isr215();
    fn isr216();
    fn isr217();
    fn isr218();
    fn isr219();
    fn isr220();
    fn isr221();
    fn isr222();
    fn isr223();
    fn isr224();
    fn isr225();
    fn isr226();
    fn isr227();
    fn isr228();
    fn isr229();
    fn isr230();
    fn isr231();
    fn isr232();
    fn isr233();
    fn isr234();
    fn isr235();
    fn isr236();
    fn isr237();
    fn isr238();
    fn isr239();
    fn isr240();
    fn isr241();
    fn isr242();
    fn isr243();
    fn isr244();
    fn isr245();
    fn isr246();
    fn isr247();
    fn isr248();
    fn isr249();
    fn isr250();
    fn isr251();
    fn isr252();
    fn isr253();
    fn isr254();
    fn isr255();

}

#[unsafe(link_section = ".host_core")]
static mut HOST_CORE_IDT_DESCRIPTOR: IdtDescriptor = IdtDescriptor { size: 0, offset: 0 };
#[unsafe(link_section = ".host_core")]
pub static mut HOST_CORE_INTERRUPT_DESCRIPTOR_TABLE: InterruptDescriptorTable =
    InterruptDescriptorTable {
        entries: invalid_mut_slice(),
        table: null_mut(),
    };
unsafe extern "C" {
    fn load_idt(descriptor: *const IdtDescriptor);
    pub fn get_idt_base() -> u64;
}

fn write_idt_entry(entry: &mut IdtEntry, func: unsafe extern "C" fn(), gate_type: u8) {
    let func_addr = func as u64;

    entry.offset_0 = (func_addr & 0xFFFF) as u16;
    entry.offset_1 = ((func_addr & 0xFFFF0000) >> 16) as u16;
    entry.offset_2 = (func_addr >> 32) as u32;
    entry.ist = 0;
    entry.reserved_0 = 0;
    entry.segment = KERNEL_CODE_SEGMENT;
    entry.properties = 0b10000000 | gate_type;
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn write_idt_entries(entries: &mut [IdtEntry]) {
    write_idt_entry(&mut entries[0], rce_divide, 0xF);
    write_idt_entry(&mut entries[1], rce_debug, 0xF);
    write_idt_entry(&mut entries[2], rce_nmi, 0xF);
    write_idt_entry(&mut entries[3], rce_breakpoint, 0xF);
    write_idt_entry(&mut entries[4], rce_overflow, 0xF);
    write_idt_entry(&mut entries[5], rce_bound, 0xF);
    write_idt_entry(&mut entries[6], rce_invalid_opcode, 0xF);
    write_idt_entry(&mut entries[7], rce_device_not_available, 0xF);
    write_idt_entry(&mut entries[8], rce_double_fault, 0xF);
    write_idt_entry(&mut entries[9], rce_coprocessor_overrun, 0xF);
    write_idt_entry(&mut entries[10], rce_invalid_tss, 0xF);
    write_idt_entry(&mut entries[11], rce_segment_not_present, 0xF);
    write_idt_entry(&mut entries[12], rce_stack_segment, 0xF);
    write_idt_entry(&mut entries[13], rce_general_protection, 0xF);
    write_idt_entry(&mut entries[14], rce_page_fault, 0xF);
    write_idt_entry(&mut entries[16], rce_x87_floating, 0xF);
    write_idt_entry(&mut entries[17], rce_alignment, 0xF);
    write_idt_entry(&mut entries[18], rce_machine_check, 0xF);
    write_idt_entry(&mut entries[19], rce_simd_floating, 0xF);
    write_idt_entry(&mut entries[20], rce_virtualization, 0xF);
    write_idt_entry(&mut entries[21], rce_control_protection, 0xF);

    write_idt_entry(&mut entries[32], isr32, 0b1110);
    write_idt_entry(&mut entries[33], isr33, 0b1110);
    write_idt_entry(&mut entries[34], isr34, 0b1110);
    write_idt_entry(&mut entries[35], isr35, 0b1110);
    write_idt_entry(&mut entries[36], isr36, 0b1110);
    write_idt_entry(&mut entries[37], isr37, 0b1110);
    write_idt_entry(&mut entries[38], isr38, 0b1110);
    write_idt_entry(&mut entries[39], isr39, 0b1110);
    write_idt_entry(&mut entries[40], isr40, 0b1110);
    write_idt_entry(&mut entries[41], isr41, 0b1110);
    write_idt_entry(&mut entries[42], isr42, 0b1110);
    write_idt_entry(&mut entries[43], isr43, 0b1110);
    write_idt_entry(&mut entries[44], isr44, 0b1110);
    write_idt_entry(&mut entries[45], isr45, 0b1110);
    write_idt_entry(&mut entries[46], isr46, 0b1110);
    write_idt_entry(&mut entries[47], isr47, 0b1110);
    write_idt_entry(&mut entries[48], isr48, 0b1110);
    write_idt_entry(&mut entries[49], isr49, 0b1110);
    write_idt_entry(&mut entries[50], isr50, 0b1110);
    write_idt_entry(&mut entries[51], isr51, 0b1110);
    write_idt_entry(&mut entries[52], isr52, 0b1110);
    write_idt_entry(&mut entries[53], isr53, 0b1110);
    write_idt_entry(&mut entries[54], isr54, 0b1110);
    write_idt_entry(&mut entries[55], isr55, 0b1110);
    write_idt_entry(&mut entries[56], isr56, 0b1110);
    write_idt_entry(&mut entries[57], isr57, 0b1110);
    write_idt_entry(&mut entries[58], isr58, 0b1110);
    write_idt_entry(&mut entries[59], isr59, 0b1110);
    write_idt_entry(&mut entries[60], isr60, 0b1110);
    write_idt_entry(&mut entries[61], isr61, 0b1110);
    write_idt_entry(&mut entries[62], isr62, 0b1110);
    write_idt_entry(&mut entries[63], isr63, 0b1110);
    write_idt_entry(&mut entries[64], isr64, 0b1110);
    write_idt_entry(&mut entries[65], isr65, 0b1110);
    write_idt_entry(&mut entries[66], isr66, 0b1110);
    write_idt_entry(&mut entries[67], isr67, 0b1110);
    write_idt_entry(&mut entries[68], isr68, 0b1110);
    write_idt_entry(&mut entries[69], isr69, 0b1110);
    write_idt_entry(&mut entries[70], isr70, 0b1110);
    write_idt_entry(&mut entries[71], isr71, 0b1110);
    write_idt_entry(&mut entries[72], isr72, 0b1110);
    write_idt_entry(&mut entries[73], isr73, 0b1110);
    write_idt_entry(&mut entries[74], isr74, 0b1110);
    write_idt_entry(&mut entries[75], isr75, 0b1110);
    write_idt_entry(&mut entries[76], isr76, 0b1110);
    write_idt_entry(&mut entries[77], isr77, 0b1110);
    write_idt_entry(&mut entries[78], isr78, 0b1110);
    write_idt_entry(&mut entries[79], isr79, 0b1110);
    write_idt_entry(&mut entries[80], isr80, 0b1110);
    write_idt_entry(&mut entries[81], isr81, 0b1110);
    write_idt_entry(&mut entries[82], isr82, 0b1110);
    write_idt_entry(&mut entries[83], isr83, 0b1110);
    write_idt_entry(&mut entries[84], isr84, 0b1110);
    write_idt_entry(&mut entries[85], isr85, 0b1110);
    write_idt_entry(&mut entries[86], isr86, 0b1110);
    write_idt_entry(&mut entries[87], isr87, 0b1110);
    write_idt_entry(&mut entries[88], isr88, 0b1110);
    write_idt_entry(&mut entries[89], isr89, 0b1110);
    write_idt_entry(&mut entries[90], isr90, 0b1110);
    write_idt_entry(&mut entries[91], isr91, 0b1110);
    write_idt_entry(&mut entries[92], isr92, 0b1110);
    write_idt_entry(&mut entries[93], isr93, 0b1110);
    write_idt_entry(&mut entries[94], isr94, 0b1110);
    write_idt_entry(&mut entries[95], isr95, 0b1110);
    write_idt_entry(&mut entries[96], isr96, 0b1110);
    write_idt_entry(&mut entries[97], isr97, 0b1110);
    write_idt_entry(&mut entries[98], isr98, 0b1110);
    write_idt_entry(&mut entries[99], isr99, 0b1110);
    write_idt_entry(&mut entries[100], isr100, 0b1110);
    write_idt_entry(&mut entries[101], isr101, 0b1110);
    write_idt_entry(&mut entries[102], isr102, 0b1110);
    write_idt_entry(&mut entries[103], isr103, 0b1110);
    write_idt_entry(&mut entries[104], isr104, 0b1110);
    write_idt_entry(&mut entries[105], isr105, 0b1110);
    write_idt_entry(&mut entries[106], isr106, 0b1110);
    write_idt_entry(&mut entries[107], isr107, 0b1110);
    write_idt_entry(&mut entries[108], isr108, 0b1110);
    write_idt_entry(&mut entries[109], isr109, 0b1110);
    write_idt_entry(&mut entries[110], isr110, 0b1110);
    write_idt_entry(&mut entries[111], isr111, 0b1110);
    write_idt_entry(&mut entries[112], isr112, 0b1110);
    write_idt_entry(&mut entries[113], isr113, 0b1110);
    write_idt_entry(&mut entries[114], isr114, 0b1110);
    write_idt_entry(&mut entries[115], isr115, 0b1110);
    write_idt_entry(&mut entries[116], isr116, 0b1110);
    write_idt_entry(&mut entries[117], isr117, 0b1110);
    write_idt_entry(&mut entries[118], isr118, 0b1110);
    write_idt_entry(&mut entries[119], isr119, 0b1110);
    write_idt_entry(&mut entries[120], isr120, 0b1110);
    write_idt_entry(&mut entries[121], isr121, 0b1110);
    write_idt_entry(&mut entries[122], isr122, 0b1110);
    write_idt_entry(&mut entries[123], isr123, 0b1110);
    write_idt_entry(&mut entries[124], isr124, 0b1110);
    write_idt_entry(&mut entries[125], isr125, 0b1110);
    write_idt_entry(&mut entries[126], isr126, 0b1110);
    write_idt_entry(&mut entries[127], isr127, 0b1110);
    write_idt_entry(&mut entries[128], isr128, 0b1110);
    write_idt_entry(&mut entries[129], isr129, 0b1110);
    write_idt_entry(&mut entries[130], isr130, 0b1110);
    write_idt_entry(&mut entries[131], isr131, 0b1110);
    write_idt_entry(&mut entries[132], isr132, 0b1110);
    write_idt_entry(&mut entries[133], isr133, 0b1110);
    write_idt_entry(&mut entries[134], isr134, 0b1110);
    write_idt_entry(&mut entries[135], isr135, 0b1110);
    write_idt_entry(&mut entries[136], isr136, 0b1110);
    write_idt_entry(&mut entries[137], isr137, 0b1110);
    write_idt_entry(&mut entries[138], isr138, 0b1110);
    write_idt_entry(&mut entries[139], isr139, 0b1110);
    write_idt_entry(&mut entries[140], isr140, 0b1110);
    write_idt_entry(&mut entries[141], isr141, 0b1110);
    write_idt_entry(&mut entries[142], isr142, 0b1110);
    write_idt_entry(&mut entries[143], isr143, 0b1110);
    write_idt_entry(&mut entries[144], isr144, 0b1110);
    write_idt_entry(&mut entries[145], isr145, 0b1110);
    write_idt_entry(&mut entries[146], isr146, 0b1110);
    write_idt_entry(&mut entries[147], isr147, 0b1110);
    write_idt_entry(&mut entries[148], isr148, 0b1110);
    write_idt_entry(&mut entries[149], isr149, 0b1110);
    write_idt_entry(&mut entries[150], isr150, 0b1110);
    write_idt_entry(&mut entries[151], isr151, 0b1110);
    write_idt_entry(&mut entries[152], isr152, 0b1110);
    write_idt_entry(&mut entries[153], isr153, 0b1110);
    write_idt_entry(&mut entries[154], isr154, 0b1110);
    write_idt_entry(&mut entries[155], isr155, 0b1110);
    write_idt_entry(&mut entries[156], isr156, 0b1110);
    write_idt_entry(&mut entries[157], isr157, 0b1110);
    write_idt_entry(&mut entries[158], isr158, 0b1110);
    write_idt_entry(&mut entries[159], isr159, 0b1110);
    write_idt_entry(&mut entries[160], isr160, 0b1110);
    write_idt_entry(&mut entries[161], isr161, 0b1110);
    write_idt_entry(&mut entries[162], isr162, 0b1110);
    write_idt_entry(&mut entries[163], isr163, 0b1110);
    write_idt_entry(&mut entries[164], isr164, 0b1110);
    write_idt_entry(&mut entries[165], isr165, 0b1110);
    write_idt_entry(&mut entries[166], isr166, 0b1110);
    write_idt_entry(&mut entries[167], isr167, 0b1110);
    write_idt_entry(&mut entries[168], isr168, 0b1110);
    write_idt_entry(&mut entries[169], isr169, 0b1110);
    write_idt_entry(&mut entries[170], isr170, 0b1110);
    write_idt_entry(&mut entries[171], isr171, 0b1110);
    write_idt_entry(&mut entries[172], isr172, 0b1110);
    write_idt_entry(&mut entries[173], isr173, 0b1110);
    write_idt_entry(&mut entries[174], isr174, 0b1110);
    write_idt_entry(&mut entries[175], isr175, 0b1110);
    write_idt_entry(&mut entries[176], isr176, 0b1110);
    write_idt_entry(&mut entries[177], isr177, 0b1110);
    write_idt_entry(&mut entries[178], isr178, 0b1110);
    write_idt_entry(&mut entries[179], isr179, 0b1110);
    write_idt_entry(&mut entries[180], isr180, 0b1110);
    write_idt_entry(&mut entries[181], isr181, 0b1110);
    write_idt_entry(&mut entries[182], isr182, 0b1110);
    write_idt_entry(&mut entries[183], isr183, 0b1110);
    write_idt_entry(&mut entries[184], isr184, 0b1110);
    write_idt_entry(&mut entries[185], isr185, 0b1110);
    write_idt_entry(&mut entries[186], isr186, 0b1110);
    write_idt_entry(&mut entries[187], isr187, 0b1110);
    write_idt_entry(&mut entries[188], isr188, 0b1110);
    write_idt_entry(&mut entries[189], isr189, 0b1110);
    write_idt_entry(&mut entries[190], isr190, 0b1110);
    write_idt_entry(&mut entries[191], isr191, 0b1110);
    write_idt_entry(&mut entries[192], isr192, 0b1110);
    write_idt_entry(&mut entries[193], isr193, 0b1110);
    write_idt_entry(&mut entries[194], isr194, 0b1110);
    write_idt_entry(&mut entries[195], isr195, 0b1110);
    write_idt_entry(&mut entries[196], isr196, 0b1110);
    write_idt_entry(&mut entries[197], isr197, 0b1110);
    write_idt_entry(&mut entries[198], isr198, 0b1110);
    write_idt_entry(&mut entries[199], isr199, 0b1110);
    write_idt_entry(&mut entries[200], isr200, 0b1110);
    write_idt_entry(&mut entries[201], isr201, 0b1110);
    write_idt_entry(&mut entries[202], isr202, 0b1110);
    write_idt_entry(&mut entries[203], isr203, 0b1110);
    write_idt_entry(&mut entries[204], isr204, 0b1110);
    write_idt_entry(&mut entries[205], isr205, 0b1110);
    write_idt_entry(&mut entries[206], isr206, 0b1110);
    write_idt_entry(&mut entries[207], isr207, 0b1110);
    write_idt_entry(&mut entries[208], isr208, 0b1110);
    write_idt_entry(&mut entries[209], isr209, 0b1110);
    write_idt_entry(&mut entries[210], isr210, 0b1110);
    write_idt_entry(&mut entries[211], isr211, 0b1110);
    write_idt_entry(&mut entries[212], isr212, 0b1110);
    write_idt_entry(&mut entries[213], isr213, 0b1110);
    write_idt_entry(&mut entries[214], isr214, 0b1110);
    write_idt_entry(&mut entries[215], isr215, 0b1110);
    write_idt_entry(&mut entries[216], isr216, 0b1110);
    write_idt_entry(&mut entries[217], isr217, 0b1110);
    write_idt_entry(&mut entries[218], isr218, 0b1110);
    write_idt_entry(&mut entries[219], isr219, 0b1110);
    write_idt_entry(&mut entries[220], isr220, 0b1110);
    write_idt_entry(&mut entries[221], isr221, 0b1110);
    write_idt_entry(&mut entries[222], isr222, 0b1110);
    write_idt_entry(&mut entries[223], isr223, 0b1110);
    write_idt_entry(&mut entries[224], isr224, 0b1110);
    write_idt_entry(&mut entries[225], isr225, 0b1110);
    write_idt_entry(&mut entries[226], isr226, 0b1110);
    write_idt_entry(&mut entries[227], isr227, 0b1110);
    write_idt_entry(&mut entries[228], isr228, 0b1110);
    write_idt_entry(&mut entries[229], isr229, 0b1110);
    write_idt_entry(&mut entries[230], isr230, 0b1110);
    write_idt_entry(&mut entries[231], isr231, 0b1110);
    write_idt_entry(&mut entries[232], isr232, 0b1110);
    write_idt_entry(&mut entries[233], isr233, 0b1110);
    write_idt_entry(&mut entries[234], isr234, 0b1110);
    write_idt_entry(&mut entries[235], isr235, 0b1110);
    write_idt_entry(&mut entries[236], isr236, 0b1110);
    write_idt_entry(&mut entries[237], isr237, 0b1110);
    write_idt_entry(&mut entries[238], isr238, 0b1110);
    write_idt_entry(&mut entries[239], isr239, 0b1110);
    write_idt_entry(&mut entries[240], isr240, 0b1110);
    write_idt_entry(&mut entries[241], isr241, 0b1110);
    write_idt_entry(&mut entries[242], isr242, 0b1110);
    write_idt_entry(&mut entries[243], isr243, 0b1110);
    write_idt_entry(&mut entries[244], isr244, 0b1110);
    write_idt_entry(&mut entries[245], isr245, 0b1110);
    write_idt_entry(&mut entries[246], isr246, 0b1110);
    write_idt_entry(&mut entries[247], isr247, 0b1110);
    write_idt_entry(&mut entries[248], isr248, 0b1110);
    write_idt_entry(&mut entries[249], isr249, 0b1110);
    write_idt_entry(&mut entries[250], isr250, 0b1110);
    write_idt_entry(&mut entries[251], isr251, 0b1110);
    write_idt_entry(&mut entries[252], isr252, 0b1110);
    write_idt_entry(&mut entries[253], isr253, 0b1110);
    write_idt_entry(&mut entries[254], isr254, 0b1110);
    write_idt_entry(&mut entries[255], isr255, 0b1110);
}

pub fn initialize_host_core_idt(allocator: &mut Allocator) {
    InterruptDescriptorTable::new_and_load(
        allocator,
        &raw mut HOST_CORE_IDT_DESCRIPTOR,
        HOST_CORE_ACTIVATION_ID,
    );
}

pub struct InterruptDescriptorTable {
    table: *mut IdtDescriptor,
    entries: &'static [IdtEntry],
}

impl InterruptDescriptorTable {
    pub fn new_and_load(
        allocator: &mut Allocator,
        table: *mut IdtDescriptor,
        core_activation_ident: u8,
    ) -> InterruptDescriptorTable {
        let mut module = Module::new("IDT");
        let entries: &'static mut [IdtEntry] = match allocator.alloc_zero(1) {
            Ok(mb) => unsafe { slice::from_raw_parts_mut(mb.as_mut_ptr(), 256) },
            Err(_e) => {
                simple_kernel_panic(
                    "InterruptDescriptorTable/new_and_load",
                    "Could not allocate memory for entries\n",
                );
            }
        };
        unsafe {
            write_idt_entries(entries);
            (*table).size = (16 * 256) - 1;
            (*table).offset = entries.as_ptr() as u64;
            load_idt(table);
        }
        success!(module, "Initialized on Core {}\n", core_activation_ident);
        return Self { table, entries };
    }
}

pub fn idt_switch_address(
    physical_allocator: &mut Allocator,
    virt_allocator: &mut VirtualAllocator,
) {
    unsafe {
        #[allow(static_mut_refs)]
        let mut descriptor = &mut HOST_CORE_IDT_DESCRIPTOR;
        let mb = virt_allocator.allocator.alloc(1).unwrap();

        memcpy(mb.as_mut_ptr(), descriptor.offset as *const c_void, 0x1000);

        physical_allocator
            .free(&MemoryBlock::new(0x1000, descriptor.offset))
            .unwrap();

        HOST_CORE_IDT_DESCRIPTOR.offset = mb.base;
        load_idt(&raw const HOST_CORE_IDT_DESCRIPTOR);
    }
}
