#[derive(Debug)]
pub struct Page {
    pub size: u64,
    pub num: u64,
}
impl Page {
    pub fn new(size: u64, num: u64) -> Self {
        Self { size, num }
    }

    pub fn offset(&self) -> u64 {
        self.size * self.num
    }
}
