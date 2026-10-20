/// Preconverted unmultiplied RGBA pixels; workers never touch UI textures.
pub struct Pixels {
    pub size: [usize; 2],
    pub rgba: Vec<[u8; 4]>,
}
