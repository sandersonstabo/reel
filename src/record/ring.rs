use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub time: i64,
    pub duration: i64,
    pub keyframe: bool,
    pub data: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub video: Vec<Packet>,
    pub audio: Vec<Packet>,
}

#[derive(Debug)]
pub struct Ring {
    span: i64,
    video: VecDeque<Packet>,
    audio: VecDeque<Packet>,
}

fn rebase(packets: impl Iterator<Item = Packet>, origin: i64) -> Vec<Packet> {
    packets
        .map(|packet| Packet {
            time: packet.time - origin,
            ..packet
        })
        .collect()
}

impl Ring {
    pub fn new(span: i64) -> Self {
        Self {
            span,
            video: VecDeque::new(),
            audio: VecDeque::new(),
        }
    }

    fn newest(&self) -> Option<i64> {
        self.video.back().map(|packet| packet.time)
    }

    fn trim(&mut self) {
        let Some(newest) = self.newest() else {
            return;
        };
        let cut = self
            .video
            .iter()
            .rposition(|packet| packet.keyframe && newest - packet.time >= self.span)
            .unwrap_or(0);
        self.video.drain(..cut);
        let oldest = self.video.front().map_or(newest, |packet| packet.time);
        while self.audio.front().is_some_and(|packet| packet.time + packet.duration <= oldest) {
            self.audio.pop_front();
        }
    }

    pub fn push_video(&mut self, packet: Packet) {
        self.video.push_back(packet);
        self.trim();
    }

    pub fn push_audio(&mut self, packet: Packet) {
        self.audio.push_back(packet);
        self.trim();
    }

    pub fn clip(&self, length: i64) -> Option<Clip> {
        let newest = self.newest()?;
        let start = self
            .video
            .iter()
            .rposition(|packet| packet.keyframe && newest - packet.time >= length)
            .or_else(|| self.video.iter().position(|packet| packet.keyframe))?;
        let origin = self.video[start].time;
        let end = newest + self.video.back().map_or(0, |packet| packet.duration);
        Some(Clip {
            video: rebase(self.video.iter().skip(start).cloned(), origin),
            audio: rebase(
                self.audio
                    .iter()
                    .filter(|packet| packet.time >= origin && packet.time < end)
                    .cloned(),
                origin,
            ),
        })
    }
}
