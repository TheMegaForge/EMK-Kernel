use core::{ffi::c_void, marker::PhantomData, ptr::null_mut, slice};

use crate::{
    fixed_vaddrs::{FIXED_PROCESSOR_VIRTUAL_ADDRESS, ref_processor_mut},
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::{PAGER_PRESENT, PAGER_RW},
        },
        print::simple_kernel_panic,
    },
    multithreading::processors::Processor,
    utils::{
        list::List,
        memory::{MemoryResult, memcpy, memcpy_qword},
        traits::Region,
    },
};

pub struct PageAllocator<T> {
    memory: MemoryBlock,
    ptr: *mut T,
    allocated: u32,
    capacity: u32,
}

impl<T> PageAllocator<T> {
    pub const fn empty() -> Self {
        return Self {
            memory: MemoryBlock::empty(),
            ptr: null_mut(),
            allocated: 0,
            capacity: 0,
        };
    }
}

impl<T> Default for PageAllocator<T> {
    fn default() -> PageAllocator<T> {
        return Self {
            memory: MemoryBlock::default(),
            ptr: null_mut(),
            allocated: 0,
            capacity: 0,
        };
    }
}

impl<T> PageAllocator<T> {
    pub fn free(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        self.capacity = 0;
        self.allocated = 0;
        self.ptr = null_mut();
        if self.memory.get_base() == 0 {
            return Option::None;
        }
        match allocator.free(&self.memory) {
            Ok(_) => {}
            Err(e) => return Option::Some(e),
        }
        self.memory.base = 0;
        self.memory.length = 0;
        return Option::None;
    }

    pub fn initialize(&mut self, allocator: &mut Allocator, mut capacity: u32) {
        let size = size_of::<T>() as u32;
        let bytes = capacity * size;
        let mut pages = 0u16;
        if bytes % 0x1000 != 0 {
            pages += 1;
        }
        pages += (bytes / 0x1000) as u16;
        capacity = (pages as u32) * 0x1000 / size;
        self.memory = match allocator.alloc_zero(pages) {
            Ok(mb) => mb,
            Err(_e) => {
                simple_kernel_panic("PageAllocator/new", "allocating failed");
            }
        };
        self.ptr = self.memory.as_mut_ptr();
        self.allocated = 0;
        self.capacity = capacity;
    }

    //operand returns false, if 'for_each' should break
    pub fn for_each(&self, mut operand: impl FnMut(u32, *const T) -> bool) {
        for i in 0..self.allocated {
            if !(operand)(i, unsafe { self.ptr.add(i as usize) }) {
                break;
            }
        }
    }

    pub fn push_back(&mut self, data: T) -> bool {
        if self.allocated + 1 > self.capacity {
            return false;
        }
        unsafe {
            *self.ptr.add(self.allocated as usize) = data;
        }
        self.allocated += 1;
        return true;
    }

    pub fn as_ref(&self, index: u32) -> Option<&T> {
        if index > self.allocated {
            return Option::None;
        }
        return unsafe { self.ptr.add(index as usize).as_ref() };
    }

    pub fn as_mut(&mut self, index: u32) -> Option<&mut T> {
        if index > self.allocated {
            return Option::None;
        }
        return unsafe { self.ptr.add(index as usize).as_mut() };
    }

    pub fn as_ref_ptr(&self, index: u32) -> Option<*const T> {
        if index > self.allocated {
            return Option::None;
        }
        return Option::Some(unsafe { self.ptr.add(index as usize) });
    }

    pub fn as_mut_ptr(&mut self, index: u32) -> Option<*mut T> {
        if index > self.allocated {
            return Option::None;
        }
        return Option::Some(unsafe { self.ptr.add(index as usize) });
    }

    pub fn size(&self) -> u32 {
        return self.allocated;
    }

    pub fn new(allocator: &mut Allocator, capacity: u32) -> PageAllocator<T> {
        let mut ret: PageAllocator<T> = PageAllocator::default();
        ret.initialize(allocator, capacity);
        return ret;
    }
}

pub struct DynamicAllocator<T> {
    memory: MemoryBlock,
    ptr: *mut T,
    allocated: u32,
    capacity: u32,
}

impl<T> Default for DynamicAllocator<T> {
    fn default() -> Self {
        return Self {
            memory: MemoryBlock::default(),
            ptr: null_mut(),
            allocated: 0,
            capacity: 0,
        };
    }
}

impl<T> DynamicAllocator<T> {
    pub fn to_list(self) -> List<T> {
        return List::new(self.memory, self.allocated);
    }

    pub fn initialize(&mut self, allocator: &mut Allocator, mut capacity: u32) {
        let size = size_of::<T>() as u32;
        let bytes = capacity * size;
        let mut pages = 0u16;
        if bytes % 0x1000 != 0 {
            pages += 1;
        }
        pages += (bytes / 0x1000) as u16;
        capacity = (pages as u32) * 0x1000 / size;
        self.memory = match allocator.alloc_zero(pages) {
            Ok(block) => block,
            Err(_e) => {
                simple_kernel_panic("DynamicAllocator/initialize", "Could not allocate\n");
            }
        };
        self.ptr = self.memory.as_mut_ptr();
        self.allocated = 0;
        self.capacity = capacity;
    }

    pub fn push_back(&mut self, allocator: &mut Allocator, data: T) -> Option<MemoryResult> {
        if self.allocated + 1 > self.capacity {
            let bytes = self.capacity * size_of::<T>() as u32;
            let mut pages = 0;
            if bytes % 0x1000 != 0 {
                pages += 1;
            }
            pages += bytes / 0x1000;
            pages *= 2;
            let new_memory = match allocator.alloc_zero(pages as u16) {
                Ok(new_mem) => new_mem,
                Err(e) => return Option::Some(e),
            };
            unsafe {
                memcpy_qword(
                    new_memory.as_mut_ptr(),
                    self.memory.as_mut_ptr(),
                    (self.memory.get_length() / 8) as u32,
                );
            }
            match allocator.free(&self.memory) {
                Ok(_) => {}
                Err(e) => {
                    let _ = allocator.free(&new_memory);
                    return Option::Some(e);
                }
            }
            self.memory = new_memory;
            self.capacity *= 2;
            self.ptr = self.memory.as_mut_ptr();
        }
        unsafe {
            *self.ptr.add(self.allocated as usize) = data;
        };
        self.allocated += 1;
        return Option::None;
    }

    pub fn ref_const(&self, index: u32) -> Option<*const T> {
        if index > self.allocated {
            return Option::None;
        }
        return Option::Some(unsafe { self.ptr.add(index as usize) } as *const T);
    }

    pub fn size(&self) -> u32 {
        return self.allocated;
    }

    pub fn new(allocator: &mut Allocator, capacity: u32) -> Self {
        let mut ret = Self::default();
        ret.initialize(allocator, capacity);
        return ret;
    }
}

#[derive(Clone, Copy)]
pub struct Node {
    length: u64,
    base: u64,
    reserving: bool,
}

impl Node {
    pub fn new(reserving: bool, length: u64, base: u64) -> Self {
        return Self {
            length,
            base,
            reserving,
        };
    }
    #[inline]
    pub fn change_base(&mut self, new_base: u64) {
        self.base = new_base;
    }
    #[inline]
    pub fn change_length(&mut self, new_length: u64) {
        self.length = new_length;
    }
    #[inline]
    pub fn get_base(&self) -> u64 {
        return self.base;
    }
    #[inline]
    pub fn get_length(&self) -> u64 {
        return self.length;
    }

    pub fn end_address<T>(&self) -> u64 {
        self.base + self.length * size_of::<T>() as u64
    }

    pub fn is_freeing(&self) -> bool {
        return !self.reserving;
    }

    pub fn is_reserving(&self) -> bool {
        return self.reserving;
    }

    pub fn base_as_offset<T>(&self, base: u64) -> u32 {
        return ((self.base - base) / size_of::<T>() as u64) as u32;
    }

    pub fn end_as_offset<T>(&self, base: u64) -> u32 {
        return ((self.end_address::<T>() - base) / size_of::<T>() as u64) as u32;
    }
}

pub struct NodeAllocator<'a, T> {
    memory: [MemoryBlock; 2],

    nodes: &'a mut [Node],
    current_node: u32,
    num_used_nodes: u32,
    phantom_data: PhantomData<T>,
    fake: bool,
}

pub enum FreeingType {
    /* back is reserving memory*/
    Front {
        front_base: u64,
        front_size: u64,
        back_base: u64,
        back_size: u64,
    },
    /* front & back are reserving memory*/
    Middle {
        front_base: u64,
        front_size: u64,
        middle_base: u64,
        middle_size: u64,
        back_base: u64,
        back_size: u64,
    },
    /* front is reserving memory*/
    Back {
        front_base: u64,
        front_size: u64,
        back_base: u64,
        back_size: u64,
    },
}

impl<'a, T> NodeAllocator<'a, T> {
    const NODE_LIST_MEMORY_INDEX: usize = 0;
    const NODE_DATA_MEMORY_INDEX: usize = 1;
    pub const fn empty() -> Self {
        return Self {
            memory: [const { MemoryBlock::empty() }; 2],
            nodes: unsafe { slice::from_raw_parts_mut(align_of::<Node>() as *mut Node, 0) },
            current_node: 0,
            num_used_nodes: 0,
            phantom_data: PhantomData,
            fake: false,
        };
    }
    /**
     * notice: returns FreeingType::Front, wenn freeing the whole node
     */
    fn freeing_type(&self, node: &Node, offset_to_free: u32, size: u32) -> FreeingType {
        let base_offset =
            node.base_as_offset::<T>(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
        let end_offset =
            node.end_as_offset::<T>(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
        if base_offset == offset_to_free || size as u64 == node.length {
            return FreeingType::Front {
                front_base: node.base,
                front_size: size as u64,
                back_base: node.base + size as u64 * size_of::<T>() as u64,
                back_size: node.length - size as u64,
            };
        } else if end_offset - size == offset_to_free {
            return FreeingType::Back {
                front_base: node.base,
                front_size: node.length - size as u64,
                back_base: self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base()
                    + offset_to_free as u64 * size_of::<T>() as u64,
                back_size: size as u64,
            };
        } else {
            let front_size = (offset_to_free - base_offset) as u64;
            let back_size = (end_offset - (offset_to_free + size)) as u64;
            return FreeingType::Middle {
                front_base: node.get_base(),
                front_size,
                middle_base: self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base()
                    + size_of::<T>() as u64 * offset_to_free as u64,
                middle_size: size as u64,
                back_base: self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base()
                    + size_of::<T>() as u64 * (offset_to_free + size) as u64,
                back_size,
            };
        }
    }

    pub fn new_fake(
        allocator: &mut Allocator,
        fake_address: &MemoryBlock,
        num_entries: u16,
    ) -> Self {
        let node_list_memory = allocator.alloc_zero(1).unwrap();
        let reserved_node = unsafe { node_list_memory.as_mut_ptr::<Node>().as_mut().unwrap() };
        let free_node = unsafe {
            node_list_memory
                .as_mut_ptr::<Node>()
                .add(1)
                .as_mut()
                .unwrap()
        };
        *reserved_node = Node::new(true, 0, fake_address.base);
        *free_node = Node::new(
            false,
            fake_address.length / size_of::<T>() as u64,
            fake_address.base,
        );
        return Self {
            nodes: unsafe {
                slice::from_raw_parts_mut(node_list_memory.as_mut_ptr(), 0x1000 / size_of::<Node>())
            },
            current_node: 1,
            num_used_nodes: 2,
            memory: [node_list_memory, fake_address.clone()],
            phantom_data: PhantomData,
            fake: true,
        };
    }

    pub fn change_base(&mut self, new_base: u64) {
        if !self.fake {
            return;
        }
        self.memory[1] = MemoryBlock::new(self.memory[1].get_length(), new_base);
    }

    pub fn to_fake(self, base: u64) -> NodeAllocator<'a, T> {
        let nodes = self.nodes;

        for i in 0..nodes.len() {
            let node = &mut nodes[i as usize];
            node.base = base + (node.base - self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
        }

        return NodeAllocator {
            memory: [
                self.memory[0],
                MemoryBlock::new(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_length(), base),
            ],
            nodes: nodes,
            current_node: self.current_node,
            num_used_nodes: self.num_used_nodes,
            phantom_data: PhantomData,
            fake: true,
        };
    }

    pub fn new(allocator: &mut Allocator, num_entries: u32) -> Self {
        let node_list_memory = allocator.alloc_zero(1).unwrap();
        let memory = size_of::<T>() * num_entries as usize;
        let mut data_pages = memory / 0x1000;
        if memory % 0x1000 != 0 {
            data_pages += 1;
        }
        let data_memory = allocator.alloc_zero(data_pages as u16).unwrap();

        let reserved_node = unsafe { node_list_memory.as_mut_ptr::<Node>().as_mut().unwrap() };
        let free_node = unsafe {
            node_list_memory
                .as_mut_ptr::<Node>()
                .add(1)
                .as_mut()
                .unwrap()
        };
        *reserved_node = Node::new(true, 0, data_memory.base);
        *free_node = Node::new(
            false,
            data_memory.length / size_of::<T>() as u64,
            data_memory.base,
        );
        return Self {
            nodes: unsafe {
                slice::from_raw_parts_mut(node_list_memory.as_mut_ptr(), 0x1000 / size_of::<Node>())
            },
            current_node: 1,
            num_used_nodes: 2,
            memory: [node_list_memory, data_memory],
            phantom_data: PhantomData,
            fake: false,
        };
    }
    pub fn as_mut_slice(&self) -> &'a mut [T] {
        unsafe {
            slice::from_raw_parts_mut(
                self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base() as *mut T,
                self.memory[Self::NODE_DATA_MEMORY_INDEX].get_length() as usize / size_of::<T>(),
            )
        }
    }
    pub fn get_base(&self) -> u64 {
        return self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base();
    }

    pub fn reallocate(&mut self, allocator: &mut Allocator, growth: u8) {
        let data_memory = &self.memory[Self::NODE_DATA_MEMORY_INDEX];
        let new_data;
        let num_current_entries =
            self.memory[Self::NODE_DATA_MEMORY_INDEX].get_length() / size_of::<T>() as u64;
        if self.fake {
            let pages_to_allocate = (data_memory.length / 0x1000) as u16 * growth as u16;
            let more_data = allocator.alloc_zero(pages_to_allocate).unwrap();
            let pager = ref_processor_mut().ref_mut_pager();
            let virt_addr = data_memory.base + data_memory.length;
            for i in 0..pages_to_allocate {
                pager
                    .page_4_kb(
                        virt_addr,
                        more_data.base + i as u64 * 0x1000,
                        PAGER_PRESENT | PAGER_RW,
                        allocator,
                    )
                    .unwrap();
            }
            new_data = MemoryBlock::new(
                data_memory.base,
                data_memory.length + pages_to_allocate as u64 * 0x1000,
            );
        } else {
            new_data = allocator
                .alloc_zero((data_memory.length / 0x1000) as u16 * growth as u16)
                .unwrap();

            unsafe {
                memcpy_qword(
                    new_data.as_mut_ptr(),
                    data_memory.as_mut_ptr(),
                    (data_memory.length / 8) as u32,
                );
            }
        }

        for i in 0..self.num_used_nodes {
            let node = self.ref_mut_node(i);
            node.base = new_data.base + (node.base - data_memory.base);
        }
        let last_node = self.ref_mut_node(self.num_used_nodes - 1);
        let space_left = (new_data.base + new_data.length) - last_node.base;
        last_node.length = space_left / size_of::<T>() as u64;

        if !self.fake {
            allocator.free(data_memory).unwrap();
        }
        self.memory[Self::NODE_DATA_MEMORY_INDEX] = new_data;
    }

    fn ref_node(&self, node_index: u32) -> &Node {
        return &self.nodes[node_index as usize];
    }
    fn ref_mut_node(&self, node_index: u32) -> &mut Node {
        return unsafe {
            (self.nodes.as_ptr().add(node_index as usize) as *mut Node)
                .as_mut()
                .unwrap()
        };
    }

    fn find_new_current(&mut self, size: u32) -> bool {
        for i in 0..self.num_used_nodes {
            if self.ref_node(i).length >= size as u64 {
                self.current_node = i;
                return true;
            }
        }
        return false;
    }

    fn find_reserving_node_for_offset(&self, offset: u32, size: u32) -> Option<u32> {
        for i in 0..self.num_used_nodes {
            let node = self.ref_node(i);
            if node.reserving {
                let start_offset =
                    node.base_as_offset::<T>(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
                let end_offset =
                    node.end_as_offset::<T>(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
                if offset >= start_offset && end_offset > (offset + size) {
                    return Option::Some(i);
                }
            }
        }
        return Option::None;
    }

    fn remove_node(&mut self, node_index: u32, is_current_node: bool) {
        let dst = unsafe { self.nodes.as_ptr().add(node_index as usize) } as *mut c_void;
        let src = &self.nodes[node_index as usize + 1] as *const Node;
        unsafe {
            memcpy(
                dst,
                src as *const c_void,
                self.num_used_nodes - (node_index + 1),
            );
        };
        self.num_used_nodes -= 1;
        if is_current_node {
            for i in 0..self.num_used_nodes {
                let node = self.ref_node(i);
                if node.is_reserving() {
                    self.current_node = i;
                    return;
                }
            }
        }
    }
    fn insert_node(&mut self, inserted_node: &Node, destination_index: u32) {
        for i in self.num_used_nodes - 1..=destination_index {
            *self.ref_mut_node(i + 1) = *self.ref_node(i);
        }
        self.nodes[destination_index as usize] = *inserted_node;

        self.num_used_nodes += 1;
    }
    /**
     * Offset is returned in units of T
     */
    pub fn allocate_as_offset(&mut self, size: u32) -> Option<u32> {
        if size as u64 > self.ref_node(self.current_node).length {
            if !self.find_new_current(size) {
                return Option::None;
            }
        }
        let freeing_node = self.ref_mut_node(self.current_node);

        let offset =
            freeing_node.base_as_offset::<T>(self.memory[Self::NODE_DATA_MEMORY_INDEX].get_base());
        /* increases reserved size*/
        self.ref_mut_node(self.current_node - 1).length += size as u64;
        freeing_node.base += size as u64 * size_of::<T>() as u64;
        freeing_node.length -= size as u64;

        if freeing_node.length == 0 {
            /* Merging the reserved block before and after self.current_node*/
            let next_reserved_length = self.ref_node(self.current_node + 1).length;
            self.remove_node(self.current_node + 1, false);
            self.remove_node(self.current_node, true);
            self.ref_mut_node(self.current_node - 1).length += next_reserved_length;
        }
        return Option::Some(offset as u32);
    }

    /**
     * Offset and size is in units of T
     */
    pub fn free_offset(&mut self, offset: u32, size: u32) -> bool {
        let reserving_node_index;
        if let Some(node_index) = self.find_reserving_node_for_offset(offset, size) {
            reserving_node_index = node_index;
        } else {
            return false;
        }
        // create free and reserving nodes if needed
        {
            let reserving_node = self.ref_mut_node(reserving_node_index);
            match self.freeing_type(reserving_node, offset, size) {
                FreeingType::Front {
                    front_base,
                    front_size,
                    back_base,
                    back_size,
                } => {
                    assert_ne!(reserving_node_index, 0);
                    reserving_node.base = back_base;
                    reserving_node.length = back_size;

                    if reserving_node.length == 0 {
                        /*
                         * combines the free block coming after the reserving_node_index
                         * and the free block coming before reserving_node_index
                         * also deletes the unused reserving block.
                         * */

                        let next_node_length = self.ref_node(reserving_node_index + 1).length;
                        self.remove_node(reserving_node_index + 1, false);
                        self.remove_node(reserving_node_index, false);
                        self.ref_mut_node(reserving_node_index - 1).length += next_node_length;
                    }

                    let prev_node = self.ref_mut_node(reserving_node_index - 1);
                    assert_eq!(prev_node.is_freeing(), true);
                    prev_node.length += front_size;
                }
                FreeingType::Middle {
                    front_base,
                    front_size,
                    middle_base,
                    middle_size,
                    back_base,
                    back_size,
                } => {
                    reserving_node.length = front_size;
                    /* Node always has to be inserted, since it´s in the reserving_node*/
                    self.insert_node(
                        &Node::new(false, middle_size, middle_base),
                        reserving_node_index + 1,
                    );
                    self.insert_node(
                        &Node::new(true, back_size, back_base),
                        reserving_node_index + 2,
                    );
                }
                FreeingType::Back {
                    front_base: _,
                    front_size,
                    back_base,
                    back_size,
                } => {
                    reserving_node.length = front_size;

                    let next_node = self.ref_mut_node(reserving_node_index + 1);
                    assert_eq!(next_node.is_freeing(), true);
                    next_node.base = back_base;
                    next_node.length += back_size;
                }
            }
        }
        return true;
    }
}
