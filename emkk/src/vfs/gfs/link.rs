use crate::vfs::gfs::GfsType;
#[derive(Clone, Copy)]
pub struct GfsLink {
    link: u64,
}

impl GfsLink {
    pub fn new(index: u32, link_type: GfsType) -> Self {
        return Self {
            link: index as u64 | link_type.to_u64() << 32,
        };
    }
    pub fn index(&self) -> u32 {
        return (self.link & 0xFFFFFFFF) as u32;
    }
    pub fn link_type(&self) -> GfsType {
        GfsType::from_u64(self.link >> 32)
    }
}
