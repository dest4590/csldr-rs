use crate::audio::acoustics::AcousticEnvironment;
use crate::hooks::sound::ListenerState;
use std::f32::consts::PI;
use std::sync::Mutex;

const HEAD_RADIUS: f32 = 0.0875;
const SPEED_OF_SOUND: f32 = 343.0;

#[derive(Clone, Copy, Debug)]
pub struct SpatialParams {
    pub azimuth: f32,
    pub distance: f32,
    pub gain_l: f32,
    pub gain_r: f32,
    pub itd_samples_l: f32,
    pub itd_samples_r: f32,
    pub occlusion: f32,
    pub reverb_send: f32,
    pub is_diffracted: bool,
    pub diffraction_angle_deg: f32,
}

impl Default for SpatialParams {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            distance: 1.0,
            gain_l: 1.0,
            gain_r: 1.0,
            itd_samples_l: 0.0,
            itd_samples_r: 0.0,
            occlusion: 0.0,
            reverb_send: 0.0,
            is_diffracted: false,
            diffraction_angle_deg: 0.0,
        }
    }
}

pub struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
    current_delay: f32,
}

impl DelayLine {
    pub fn new(max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples.max(1) + 2],
            write_pos: 0,
            current_delay: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32, target_delay_samples: f32) -> f32 {
        if self.buffer.is_empty() {
            return sample;
        }

        let len = self.buffer.len();
        self.buffer[self.write_pos] = sample;

        self.current_delay += (target_delay_samples - self.current_delay) * 0.005;

        let delay = self.current_delay.clamp(0.0, (len - 2) as f32);
        let read_pos = (self.write_pos as f32 - delay).rem_euclid(len as f32);
        let idx_a = read_pos.floor() as usize % len;
        let idx_b = (idx_a + 1) % len;
        let frac = read_pos - read_pos.floor();
        let out = self.buffer[idx_a] * (1.0 - frac) + self.buffer[idx_b] * frac;

        self.write_pos = (self.write_pos + 1) % len;
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LowPassFilter {
    pub alpha: f32,
    pub target_alpha: f32,
    pub prev_out_l: f32,
    pub prev_out_r: f32,
}

impl LowPassFilter {
    pub fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate: f32) {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * cutoff_hz.clamp(800.0, 20000.0));
        self.target_alpha = dt / (rc + dt);
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.alpha += (self.target_alpha - self.alpha) * 0.01;
        let out_l = self.prev_out_l + self.alpha * (in_l - self.prev_out_l);
        let out_r = self.prev_out_r + self.alpha * (in_r - self.prev_out_r);
        self.prev_out_l = out_l;
        self.prev_out_r = out_r;
        (out_l, out_r)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ElevationNotchFilter {
    pub prev_l: f32,
    pub prev_r: f32,
    pub band_l: f32,
    pub band_r: f32,
}

impl ElevationNotchFilter {
    pub fn process(&mut self, in_l: f32, in_r: f32, notch_depth: f32) -> (f32, f32) {
        if notch_depth <= 0.01 {
            return (in_l, in_r);
        }
        let diff_l = in_l - self.prev_l;
        let diff_r = in_r - self.prev_r;
        self.band_l += 0.45 * (diff_l - self.band_l);
        self.band_r += 0.45 * (diff_r - self.band_r);
        self.prev_l = in_l;
        self.prev_r = in_r;

        let out_l = in_l - self.band_l * (notch_depth * 0.65);
        let out_r = in_r - self.band_r * (notch_depth * 0.65);
        (out_l, out_r)
    }
}

pub struct FdnReverb {
    delays_l: [Vec<f32>; 4],
    delays_r: [Vec<f32>; 4],
    write_pos: [usize; 4],
    write_pos_r: [usize; 4],
    delay_lengths: [usize; 4],
    delay_lengths_r: [usize; 4],
    feedback_gains: [f32; 4],
    feedback_gains_r: [f32; 4],
    sample_rate: f32,
}

impl FdnReverb {
    pub fn new(sample_rate: f32) -> Self {
        let delay_ms_l = [27.0, 35.0, 41.0, 49.0];
        let delay_ms_r = [29.0, 33.0, 44.0, 47.0];
        let mut delay_lengths = [0usize; 4];
        let mut delay_lengths_r = [0usize; 4];
        let mut delays_l = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut delays_r = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

        for i in 0..4 {
            let len_l = (delay_ms_l[i] * 0.001 * sample_rate) as usize;
            let len_r = (delay_ms_r[i] * 0.001 * sample_rate) as usize;
            delay_lengths[i] = len_l.max(1);
            delay_lengths_r[i] = len_r.max(1);
            delays_l[i] = vec![0.0; len_l.max(1)];
            delays_r[i] = vec![0.0; len_r.max(1)];
        }

        Self {
            delays_l,
            delays_r,
            write_pos: [0; 4],
            write_pos_r: [0; 4],
            delay_lengths,
            delay_lengths_r,
            feedback_gains: [0.4; 4],
            feedback_gains_r: [0.4; 4],
            sample_rate,
        }
    }

    pub fn update_acoustics(&mut self, acoustics: &AcousticEnvironment) {
        let t60 = acoustics.t60_reverb_time.max(0.05);
        let damping = acoustics.damping.clamp(0.05, 0.85);

        for i in 0..4 {
            let delay_sec_l = self.delay_lengths[i] as f32 / self.sample_rate;
            let delay_sec_r = self.delay_lengths_r[i] as f32 / self.sample_rate;
            let base_gain_l = 10.0f32.powf(-3.0 * delay_sec_l / t60).clamp(0.0, 0.88);
            let base_gain_r = 10.0f32.powf(-3.0 * delay_sec_r / t60).clamp(0.0, 0.88);

            let target_gain_l = base_gain_l * (1.0 - damping * 0.20);
            let target_gain_r = base_gain_r * (1.0 - damping * 0.20);

            self.feedback_gains[i] += (target_gain_l - self.feedback_gains[i]) * 0.05;
            self.feedback_gains_r[i] += (target_gain_r - self.feedback_gains_r[i]) * 0.05;
        }
    }

    pub fn process(&mut self, in_l: f32, in_r: f32, mix: f32, send: f32) -> (f32, f32) {
        if mix <= 0.001 {
            return (in_l, in_r);
        }

        const CROSS_FEED: f32 = 0.15;
        let effective_send = send.clamp(0.0, 1.0);
        let feed_l = (in_l + in_r * CROSS_FEED) * effective_send;
        let feed_r = (in_r + in_l * CROSS_FEED) * effective_send;

        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;

        for i in 0..4 {
            let pos_l = self.write_pos[i];
            let pos_r = self.write_pos_r[i];
            let len_l = self.delay_lengths[i];
            let len_r = self.delay_lengths_r[i];
            let read_l = self.delays_l[i][pos_l];
            let read_r = self.delays_r[i][pos_r];

            out_l += read_l;
            out_r += read_r;

            let fb_l = feed_l + read_l * self.feedback_gains[i];
            let fb_r = feed_r + read_r * self.feedback_gains_r[i];

            self.delays_l[i][pos_l] = fb_l;
            self.delays_r[i][pos_r] = fb_r;
            self.write_pos[i] = (pos_l + 1) % len_l;
            self.write_pos_r[i] = (pos_r + 1) % len_r;
        }

        let rev_l = (out_l * 0.25) * mix;
        let rev_r = (out_r * 0.25) * mix;

        (
            in_l * (1.0 - mix * 0.3) + rev_l,
            in_r * (1.0 - mix * 0.3) + rev_r,
        )
    }
}

pub struct TunnelResonator {
    comb_l1: DelayLine,
    comb_l2: DelayLine,
    comb_r1: DelayLine,
    comb_r2: DelayLine,
    sub_lpf_l: f32,
    sub_lpf_r: f32,
    sub_alpha: f32,
    sample_rate: f32,
}

impl TunnelResonator {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = (0.030 * sample_rate) as usize;
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * 160.0);
        let sub_alpha = dt / (rc + dt);
        Self {
            comb_l1: DelayLine::new(max_samples),
            comb_l2: DelayLine::new(max_samples),
            comb_r1: DelayLine::new(max_samples),
            comb_r2: DelayLine::new(max_samples),
            sub_lpf_l: 0.0,
            sub_lpf_r: 0.0,
            sub_alpha,
            sample_rate,
        }
    }

    pub fn process(&mut self, in_l: f32, in_r: f32, intensity: f32) -> (f32, f32) {
        if intensity <= 0.001 {
            return (in_l, in_r);
        }

        let delay_1_samples = 0.0078 * self.sample_rate;
        let delay_2_samples = 0.0125 * self.sample_rate;

        let read_l1 = self.comb_l1.process(in_l, delay_1_samples);
        let read_l2 = self.comb_l2.process(in_l, delay_2_samples);
        let read_r1 = self.comb_r1.process(in_r, delay_1_samples);
        let read_r2 = self.comb_r2.process(in_r, delay_2_samples);

        let flutter_l = (read_l1 * 0.30 + read_l2 * 0.20) * intensity;
        let flutter_r = (read_r1 * 0.30 + read_r2 * 0.20) * intensity;

        self.sub_lpf_l += self.sub_alpha * (in_l - self.sub_lpf_l);
        self.sub_lpf_r += self.sub_alpha * (in_r - self.sub_lpf_r);

        let sub_boost_l = self.sub_lpf_l * 0.25 * intensity;
        let sub_boost_r = self.sub_lpf_r * 0.25 * intensity;

        let out_l = in_l + flutter_l + sub_boost_l;
        let out_r = in_r + flutter_r + sub_boost_r;

        let sat_l = out_l / (1.0 + out_l.abs());
        let sat_r = out_r / (1.0 + out_r.abs());

        (sat_l, sat_r)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConcussionState {
    pub start_time: f32,
    pub active_until: f32,
    pub duration: f32,
    pub intensity: f32,
}

pub static G_CONCUSSION: Mutex<ConcussionState> = Mutex::new(ConcussionState {
    start_time: 0.0,
    active_until: 0.0,
    duration: 0.0,
    intensity: 0.0,
});

pub fn trigger_grenade_concussion(intensity: f32, duration: f32) {
    let now = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map(|t| *t)
        .unwrap_or(0.0);
    let real_now = if now > 0.0 {
        now
    } else {
        // Fallback if client time is not ticked yet
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f32())
            .unwrap_or(0.0)
    };
    if let Ok(mut c) = G_CONCUSSION.lock() {
        c.start_time = real_now;
        c.active_until = real_now + duration;
        c.duration = duration;
        c.intensity = intensity.clamp(0.0, 1.0);
    }
}

pub struct NativeHrtfEngine {
    pub sample_rate: f32,
    pub lpf: LowPassFilter,
    pub far_ear_lpf: LowPassFilter,
    pub air_lpf: LowPassFilter,
    pub concussion_lpf: LowPassFilter,
    itd_delay_l: DelayLine,
    itd_delay_r: DelayLine,
    reverb_a: FdnReverb,
    current_room: Option<crate::audio::acoustics::RoomType>,
    pub last_spatial: SpatialParams,
    tunnel_effect: TunnelResonator,
    tunnel_blend: f32,
    tinnitus_phase: f32,
}

impl NativeHrtfEngine {
    const POSTMIX_FILTER_BASE: f32 = 0.01;
    const POSTMIX_FILTER_OCCLUSION: f32 = 0.65;
    const POSTMIX_FILTER_AZIMUTH: f32 = 0.10;
    const POSTMIX_ILD_STRENGTH: f32 = 0.20;

    pub fn new(sample_rate: f32) -> Self {
        let mut lpf = LowPassFilter::default();
        lpf.set_cutoff(20000.0, sample_rate);

        let mut far_ear_lpf = LowPassFilter::default();
        far_ear_lpf.set_cutoff(20000.0, sample_rate);

        let mut air_lpf = LowPassFilter::default();
        air_lpf.set_cutoff(20000.0, sample_rate);

        let mut concussion_lpf = LowPassFilter::default();
        concussion_lpf.set_cutoff(20000.0, sample_rate);

        let max_itd_samples = ((HEAD_RADIUS / SPEED_OF_SOUND) * 4.0 * sample_rate).ceil() as usize;

        Self {
            sample_rate,
            lpf,
            far_ear_lpf,
            air_lpf,
            concussion_lpf,
            itd_delay_l: DelayLine::new(max_itd_samples),
            itd_delay_r: DelayLine::new(max_itd_samples),
            reverb_a: FdnReverb::new(sample_rate),
            current_room: None,
            last_spatial: SpatialParams::default(),
            tunnel_effect: TunnelResonator::new(sample_rate),
            tunnel_blend: 0.0,
            tinnitus_phase: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if sample_rate.is_finite()
            && sample_rate > 1.0
            && (self.sample_rate - sample_rate).abs() > 1.0
        {
            *self = Self::new(sample_rate);
        }
    }

    pub fn update_acoustics(&mut self, acoustics: &AcousticEnvironment) {
        self.current_room = Some(acoustics.room_type);
        self.reverb_a.update_acoustics(acoustics);
    }

    pub fn process_reverb(&mut self, in_l: f32, in_r: f32, mix: f32, send: f32) -> (f32, f32) {
        self.reverb_a.process(in_l, in_r, mix, send)
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32, mix: f32) -> (f32, f32) {
        let azimuth = self.last_spatial.azimuth;
        let (dry_l, dry_r) = self.apply_itd(in_l, in_r);
        let (flt_l, flt_r) = self.apply_filters(dry_l, dry_r, azimuth);

        let filter_mix = self.postmix_filter_mix();
        let shaped_l = dry_l + (flt_l - dry_l) * filter_mix;
        let shaped_r = dry_r + (flt_r - dry_r) * filter_mix;

        let (gain_l, gain_r) = self.postmix_gains();
        let direct_l = shaped_l * gain_l;
        let direct_r = shaped_r * gain_r;

        let (rev_l, rev_r) =
            self.process_reverb(direct_l, direct_r, mix, self.last_spatial.reverb_send);

        let target_blend = if self.current_room == Some(crate::audio::acoustics::RoomType::Tunnel) {
            (mix / 0.60).clamp(0.0, 1.0) * 0.70
        } else {
            0.0
        };
        self.tunnel_blend += (target_blend - self.tunnel_blend) * 0.001;

        self.tunnel_effect.process(rev_l, rev_r, self.tunnel_blend)
    }

    pub fn process_buffer_interleaved_i16(&mut self, buffer: &mut [i16], mix: f32) {
        let now = crate::engine::cdll_int::CLIENT_TIME
            .lock()
            .map(|t| *t)
            .unwrap_or(0.0);
        let real_now = if now > 0.0 {
            now
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f32())
                .unwrap_or(0.0)
        };
        let concussion = G_CONCUSSION.lock().map(|c| *c).unwrap_or_default();
        let is_concussed = real_now < concussion.active_until && concussion.duration > 0.0;
        let (conc_factor, attack_factor) = if is_concussed {
            let elapsed = (real_now - concussion.start_time).max(0.0);
            let raw_attack = (elapsed / 0.60).clamp(0.0, 1.0);
            let attack = raw_attack * raw_attack * (3.0 - 2.0 * raw_attack);

            let remain = (concussion.active_until - real_now) / concussion.duration;
            let decay = remain.clamp(0.0, 1.0).powf(1.4) * concussion.intensity;
            (decay * attack, attack)
        } else {
            (0.0, 0.0)
        };

        if conc_factor > 0.001 {
            let cutoff = 180.0 + (20000.0 - 180.0) * (1.0 - conc_factor).powi(3);
            self.concussion_lpf.set_cutoff(cutoff, self.sample_rate);
        }

        let base_tone = 3600.0;
        let dt = 1.0 / self.sample_rate;

        for chunk in buffer.chunks_exact_mut(2) {
            let in_l = chunk[0] as f32 / 32768.0;
            let in_r = chunk[1] as f32 / 32768.0;
            let (mut out_l, mut out_r) = self.process_sample(in_l, in_r, mix);

            if conc_factor > 0.001 {
                let (proc_l, proc_r) = self.concussion_lpf.process(out_l, out_r);
                out_l = proc_l * (1.0 - conc_factor * 0.85);
                out_r = proc_r * (1.0 - conc_factor * 0.85);

                let primary_tone = self.tinnitus_phase.sin();
                let sub_harmonic = (self.tinnitus_phase * 0.5).sin() * 0.30;
                let overtone = (self.tinnitus_phase * 1.5).sin() * 0.15;
                let combined_tone =
                    (primary_tone + sub_harmonic + overtone) * conc_factor * attack_factor * 0.14;

                let phase_inc = 2.0 * PI * (base_tone + conc_factor * 200.0) * dt;
                self.tinnitus_phase = (self.tinnitus_phase + phase_inc) % (2.0 * PI);

                out_l += combined_tone * 0.95;
                out_r += combined_tone * 1.05;
            }

            chunk[0] = (out_l.clamp(-1.0, 1.0) * 32767.0) as i16;
            chunk[1] = (out_r.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
    }

    pub fn process_buffer_interleaved_f32(&mut self, buffer: &mut [f32], mix: f32) {
        let now = crate::engine::cdll_int::CLIENT_TIME
            .lock()
            .map(|t| *t)
            .unwrap_or(0.0);
        let real_now = if now > 0.0 {
            now
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f32())
                .unwrap_or(0.0)
        };
        let concussion = G_CONCUSSION.lock().map(|c| *c).unwrap_or_default();
        let is_concussed = real_now < concussion.active_until && concussion.duration > 0.0;
        let (conc_factor, attack_factor) = if is_concussed {
            let elapsed = (real_now - concussion.start_time).max(0.0);
            // Smooth S-curve attack over 600ms so tinnitus doesn't start abruptly
            let raw_attack = (elapsed / 0.60).clamp(0.0, 1.0);
            let attack = raw_attack * raw_attack * (3.0 - 2.0 * raw_attack);

            let remain = (concussion.active_until - real_now) / concussion.duration;
            let decay = remain.clamp(0.0, 1.0).powf(1.4) * concussion.intensity;
            (decay * attack, attack)
        } else {
            (0.0, 0.0)
        };

        if conc_factor > 0.001 {
            // Aquarium low-pass down to 180 Hz
            let cutoff = 180.0 + (20000.0 - 180.0) * (1.0 - conc_factor).powi(3);
            self.concussion_lpf.set_cutoff(cutoff, self.sample_rate);
        }

        let base_tone = 3600.0;
        let dt = 1.0 / self.sample_rate;

        for chunk in buffer.chunks_exact_mut(2) {
            let (mut out_l, mut out_r) = self.process_sample(chunk[0], chunk[1], mix);

            if conc_factor > 0.001 {
                let (proc_l, proc_r) = self.concussion_lpf.process(out_l, out_r);
                out_l = proc_l * (1.0 - conc_factor * 0.85);
                out_r = proc_r * (1.0 - conc_factor * 0.85);

                // Multi-harmonic rich ringing with smooth fade-in
                let primary_tone = self.tinnitus_phase.sin();
                let sub_harmonic = (self.tinnitus_phase * 0.5).sin() * 0.30;
                let overtone = (self.tinnitus_phase * 1.5).sin() * 0.15;
                let combined_tone =
                    (primary_tone + sub_harmonic + overtone) * conc_factor * attack_factor * 0.14;

                let phase_inc = 2.0 * PI * (base_tone + conc_factor * 200.0) * dt;
                self.tinnitus_phase = (self.tinnitus_phase + phase_inc) % (2.0 * PI);

                out_l += combined_tone * 0.95;
                out_r += combined_tone * 1.05;
            }

            chunk[0] = out_l.clamp(-1.0, 1.0);
            chunk[1] = out_r.clamp(-1.0, 1.0);
        }
    }

    pub fn compute_spatial_params(
        &mut self,
        source_origin: [f32; 3],
        listener: &ListenerState,
        occlusion: f32,
        reverb_send: f32,
        is_diffracted: bool,
        diffraction_angle_deg: f32,
    ) -> SpatialParams {
        let dx = source_origin[0] - listener.origin[0];
        let dy = source_origin[1] - listener.origin[1];
        let dz = source_origin[2] - listener.origin[2];

        let distance = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0);

        // Horizontal view yaw projection (independent of looking pitch up/down)
        let fwd_h = [listener.forward[0], listener.forward[1]];
        let fwd_len = (fwd_h[0] * fwd_h[0] + fwd_h[1] * fwd_h[1])
            .sqrt()
            .max(0.001);
        let fwd_norm = [fwd_h[0] / fwd_len, fwd_h[1] / fwd_len];
        let right_norm = [fwd_norm[1], -fwd_norm[0]]; // 90 deg clockwise in 2D

        // Pure horizontal azimuth calculation
        let local_fwd = dx * fwd_norm[0] + dy * fwd_norm[1];
        let local_right = dx * right_norm[0] + dy * right_norm[1];
        let azimuth = local_right.atan2(local_fwd);

        let sin_az = azimuth.sin();
        let cos_az = azimuth.cos();
        let itd_sec = (HEAD_RADIUS / SPEED_OF_SOUND) * (sin_az + azimuth * sin_az.abs());
        let itd_samples = itd_sec * self.sample_rate;

        const CLOSE_RANGE_MIN: f32 = 45.0;
        const CLOSE_RANGE_MAX: f32 = 120.0;
        let spatial_blend =
            ((distance - CLOSE_RANGE_MIN) / (CLOSE_RANGE_MAX - CLOSE_RANGE_MIN)).clamp(0.0, 1.0);

        let raw_itd_l = if itd_samples > 0.0 { itd_samples } else { 0.0 };
        let raw_itd_r = if itd_samples < 0.0 { -itd_samples } else { 0.0 };
        let itd_l = raw_itd_l * spatial_blend;
        let itd_r = raw_itd_r * spatial_blend;

        let near_ear = 1.0;
        let far_ear = (0.75 + 0.25 * cos_az).clamp(0.60, 1.0);
        let blended_far_ear = 1.0 + (far_ear - 1.0) * spatial_blend;

        let (target_gain_l, target_gain_r) = if azimuth >= 0.0 {
            (blended_far_ear, near_ear)
        } else {
            (near_ear, blended_far_ear)
        };

        let lerp_gain = 0.25;
        let gain_l = (self.last_spatial.gain_l
            + (target_gain_l - self.last_spatial.gain_l) * lerp_gain)
            .max(0.2);
        let gain_r = (self.last_spatial.gain_r
            + (target_gain_r - self.last_spatial.gain_r) * lerp_gain)
            .max(0.2);

        let smooth_occl =
            self.last_spatial.occlusion + (occlusion - self.last_spatial.occlusion) * 0.20;

        let min_cutoff = 2400.0;
        let cutoff = min_cutoff + (20000.0 - min_cutoff) * (1.0 - smooth_occl).powi(2);
        self.lpf.set_cutoff(cutoff, self.sample_rate);

        let shadow_amount = sin_az.abs();
        let far_ear_cutoff = 17500.0 - shadow_amount * 5500.0;
        self.far_ear_lpf
            .set_cutoff(far_ear_cutoff, self.sample_rate);

        const REFERENCE_DIST: f32 = 39.37;
        let air_cutoff = (20000.0 - (distance / REFERENCE_DIST) * 180.0).clamp(6500.0, 20000.0);
        self.air_lpf.set_cutoff(air_cutoff, self.sample_rate);

        let params = SpatialParams {
            azimuth,
            distance,
            itd_samples_l: itd_l,
            itd_samples_r: itd_r,
            gain_l,
            gain_r,
            occlusion: smooth_occl,
            reverb_send: reverb_send.clamp(0.0, 1.0),
            is_diffracted,
            diffraction_angle_deg,
        };

        self.last_spatial = params;
        params
    }

    pub fn apply_itd(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let delayed_l = self
            .itd_delay_l
            .process(in_l, self.last_spatial.itd_samples_l.max(0.0));
        let delayed_r = self
            .itd_delay_r
            .process(in_r, self.last_spatial.itd_samples_r.max(0.0));
        (delayed_l, delayed_r)
    }

    fn postmix_filter_mix(&self) -> f32 {
        let azimuth_mix = self.last_spatial.azimuth.sin().abs();
        const REFERENCE_DIST: f32 = 39.37;
        let dist_excess = (self.last_spatial.distance - 80.0).max(0.0);
        let dist_factor = ((dist_excess / REFERENCE_DIST) * 0.02).clamp(0.0, 0.20);
        (Self::POSTMIX_FILTER_BASE
            + self.last_spatial.occlusion * Self::POSTMIX_FILTER_OCCLUSION
            + azimuth_mix * Self::POSTMIX_FILTER_AZIMUTH
            + dist_factor)
            .clamp(0.01, 0.90)
    }

    fn postmix_gains(&self) -> (f32, f32) {
        let balance = (self.last_spatial.gain_r - self.last_spatial.gain_l).clamp(-0.40, 0.40);
        let gain_l = (1.0 - balance * Self::POSTMIX_ILD_STRENGTH).clamp(0.85, 1.15);
        let gain_r = (1.0 + balance * Self::POSTMIX_ILD_STRENGTH).clamp(0.85, 1.15);
        (gain_l, gain_r)
    }

    pub fn apply_filters(&mut self, in_l: f32, in_r: f32, azimuth: f32) -> (f32, f32) {
        let (occl_l, occl_r) = self.lpf.process(in_l, in_r);
        let (shadow_l, shadow_r) = self.far_ear_lpf.process(occl_l, occl_r);
        let (out_l, out_r) = if azimuth >= 0.0 {
            (shadow_l, occl_r)
        } else {
            (occl_l, shadow_r)
        };
        self.air_lpf.process(out_l, out_r)
    }
}

pub static NATIVE_HRTF_ENGINE: Mutex<Option<NativeHrtfEngine>> = Mutex::new(None);

pub fn init_native_hrtf_engine() {
    let engine = NativeHrtfEngine::new(44100.0);
    if let Ok(mut guard) = NATIVE_HRTF_ENGINE.lock() {
        *guard = Some(engine);
    }
}
