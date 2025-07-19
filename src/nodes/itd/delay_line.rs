#[derive(Debug)]
pub struct DelayLine {
    buffer: Vec<f32>,
    pub write_head: usize,

    /// The read head is a fractional offset from the write head.
    ///
    /// The larger this value, the further back in time we read.
    pub read_head: f32,
}

impl DelayLine {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            write_head: 0,
            read_head: 0.0,
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn resize(&mut self, new_size: usize) {
        self.buffer.resize(new_size, 0.0);
    }

    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_head] = sample;
        self.write_head = (self.write_head + 1) % self.buffer.len();
    }

    /// Read from the buffer, performing linear interpolation.
    pub fn read(&self) -> f32 {
        let write_head = self.write_head as f32;

        let index = write_head - (self.read_head + 0.99);
        let index = index.rem_euclid(self.buffer.len() as f32);

        let fract = index.fract();
        let index_a = index.floor() as usize;
        let index_b = index.ceil() as usize % self.buffer.len();

        let a = self.buffer[index_a];
        let b = self.buffer[index_b];

        a + fract * (b - a)
    }
}
