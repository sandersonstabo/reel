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
        let nearest = (time + self.interval / 2).div_euclid(self.interval);
        let slot = self.last_slot.map_or(nearest, |last| nearest.max(last + 1));
        let early = slot * self.interval - time;
        match time >= 0 && early <= self.interval / 2 {
            false => None,
            true => {
                self.last_slot = Some(slot);
                Some(slot * self.interval)
            }
        }
    }
}
