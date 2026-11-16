#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pacer {
    interval: i64,
    due: Option<i64>,
}

impl Pacer {
    pub fn new(ticks_per_second: i64, fps: u32) -> Self {
        Self {
            interval: ticks_per_second / i64::from(fps),
            due: None,
        }
    }

    pub fn interval(&self) -> i64 {
        self.interval
    }

    pub fn admit(&mut self, time: i64) -> bool {
        let tolerance = self.interval / 2;
        match (time >= 0, self.due) {
            (false, _) => false,
            (true, Some(due)) if time < due - tolerance => false,
            (true, due) => {
                let next = due.map_or(time + self.interval, |due| {
                    (due + self.interval).max(time + self.interval - tolerance)
                });
                self.due = Some(next);
                true
            }
        }
    }
}
