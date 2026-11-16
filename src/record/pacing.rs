#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pacer {
    interval: i64,
    last_slot: Option<i64>,
}

impl Pacer {
    pub fn new(ticks_per_second: i64, fps: u32) -> Self {
        Self {
            interval: ticks_per_second / i64::from(fps),
            last_slot: None,
        }
    }

    pub fn interval(&self) -> i64 {
        self.interval
    }

    pub fn admit(&mut self, time: i64) -> Option<i64> {
        let slot = time.div_euclid(self.interval);
        match (time >= 0, self.last_slot) {
            (false, _) => None,
            (true, Some(last)) if slot <= last => None,
            (true, _) => {
                self.last_slot = Some(slot);
                Some(slot * self.interval)
            }
        }
    }
}
