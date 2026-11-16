#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Contiguous,
    Silence(u64),
    Skip(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    rate: u32,
    ticks_per_second: i64,
    tolerance: i64,
    written: u64,
}

impl Timeline {
    pub fn new(rate: u32, ticks_per_second: i64, tolerance: i64) -> Self {
        Self {
            rate,
            ticks_per_second,
            tolerance,
            written: 0,
        }
    }

    pub fn position(&self) -> i64 {
        (i128::from(self.written) * i128::from(self.ticks_per_second) / i128::from(self.rate)) as i64
    }

    pub fn duration(&self, frames: u64) -> i64 {
        (i128::from(frames) * i128::from(self.ticks_per_second) / i128::from(self.rate)) as i64
    }

    fn frames_in(&self, ticks: i64) -> u64 {
        (i128::from(ticks) * i128::from(self.rate) / i128::from(self.ticks_per_second)) as u64
    }

    pub fn align(&self, packet_time: i64) -> Alignment {
        let drift = packet_time - self.position();
        match drift {
            d if d > self.tolerance => Alignment::Silence(self.frames_in(d)),
            d if d < -self.tolerance => Alignment::Skip(self.frames_in(-d)),
            _ => Alignment::Contiguous,
        }
    }

    pub fn silence_until(&self, time: i64) -> u64 {
        match time - self.position() {
            gap if gap > self.tolerance => self.frames_in(gap),
            _ => 0,
        }
    }

    pub fn advance(&mut self, frames: u64) {
        self.written += frames;
    }
}
