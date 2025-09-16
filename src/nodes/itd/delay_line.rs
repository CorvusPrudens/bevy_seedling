#[derive(Debug)]
pub struct DelayLine {
    buffer: Vec<f32>,
    write_head: usize,

    /// The read head is a fractional offset from the write head.
    ///
    /// The larger this value, the further back in time we read.
    read_head: f32,
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

    /// Set the sample offset for the read head.
    ///
    /// The larger this value, the further back in time we read.
    /// `delay` is epxressed as a ratio in the range [0, 1].
    pub fn set_read_head(&mut self, delay: f32) {
        let max = self.len().saturating_sub(1) as f32;
        self.read_head = delay.clamp(0.0, 1.0) * max;
    }

    /// Read from the buffer, performing linear interpolation.
    pub fn read(&self) -> f32 {
        let read_position = self.write_head as f32 - 1.0 - self.read_head;

        let wrapped_position = read_position.rem_euclid(self.buffer.len() as f32);

        let index_a = wrapped_position.floor() as usize;
        let index_b = (index_a + 1) % self.buffer.len();

        let fract = wrapped_position.fract();

        let a = self.buffer[index_a];
        let b = self.buffer[index_b];

        a + fract * (b - a)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_oob() {
        let mut delay = DelayLine::new(31);
        delay.set_read_head(1.0);

        for _ in 0..64 {
            delay.write(0.5);
            delay.read();
        }
    }
}
