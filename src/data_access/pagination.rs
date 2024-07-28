#[derive(Debug)]
pub struct Page {
    pub size: u32,
    pub num: u32,
}
impl Page {
    pub fn new(size: u32, num: u32) -> Self {
        Self { size, num }
    }

    pub fn offset(&self) -> u32 {
        self.size * self.num
    }
}
