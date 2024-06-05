use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub enum TargetFrameRate {
    Limited(u32),
    UnlimitedOrVsync,
}

#[derive(Debug, Clone)]
pub struct Time {
    /// Target frame rate
    target_frame_rate: TargetFrameRate,
    /// Instant of the first frame
    first_frame_instant: Instant,
    /// Instant at which begin_frame() was last called
    last_frame_instant: Instant,
    /// Instant of the last second
    last_second_instant: Instant,
    /// Duration of the previous frame
    delta: Duration,
    /// Number of frames so far in this second
    frames_this_second: u32,
    /// Number of frames in the last second
    frames_last_second: u32,
}

impl Time {
    pub fn new(target_frame_rate: TargetFrameRate) -> Self {
        Self {
            target_frame_rate,
            first_frame_instant: Instant::now(),
            last_frame_instant: Instant::now(),
            last_second_instant: Instant::now(),
            delta: Duration::ZERO,
            frames_this_second: 0,
            frames_last_second: 0,
        }
    }

    /// This function is called at the beginning of each frame
    pub fn begin_frame(&mut self) {
        // update delta
        let now = Instant::now();
        self.delta = now - self.last_frame_instant;

        // update last frame instant
        self.last_frame_instant = now;
    }

    /// This function is called at the end of each frame to sleep for the
    /// duration required to achieve the target frame rate
    pub fn wait_for_next_frame(&mut self) {
        match self.target_frame_rate {
            TargetFrameRate::Limited(frame_count) => {
                let target_frame_duration = Duration::from_secs_f64((frame_count as f64).recip());
                let current_frame_duration = Instant::now() - self.last_frame_instant;
                if target_frame_duration > current_frame_duration {
                    std::thread::sleep(target_frame_duration - current_frame_duration);
                }
            }
            TargetFrameRate::UnlimitedOrVsync => (),
        }
    }

    /// This function is called at the end of each frame to increment the frame counter
    pub fn update_frame_count(&mut self) {
        const ONE_SECOND: Duration = Duration::from_secs(1);

        let now = Instant::now();

        self.frames_this_second += 1;

        // update frames per second/fixed updates per second timers
        if now - self.last_second_instant >= ONE_SECOND {
            self.last_second_instant = now;
            self.frames_last_second = self.frames_this_second;
            self.frames_this_second = 0;
        }
    }

    /// The duration of the previous frame
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// The duration of the previous frame in seconds
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// The duration of the previous frame in seconds
    pub fn delta_seconds_f64(&self) -> f64 {
        self.delta.as_secs_f64()
    }

    /// The duration the program has been running
    pub fn elapsed(&self) -> Duration {
        self.last_frame_instant - self.first_frame_instant
    }

    /// The duration the program has been running in seconds
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    /// The duration the program has been running in seconds
    pub fn elapsed_seconds_f64(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    /// The number of frames in the previous second
    pub fn get_frames_last_second(&self) -> u32 {
        self.frames_last_second
    }
}
