//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>
//! - The first of the two official SNES documentation books

use crate::{
    backend::AudioBackend,
    timing::{Cycles, APU_CPU_TIMING_PROPORTION_NTSC, APU_CPU_TIMING_PROPORTION_PAL},
};
use core::{cell::Cell, iter::once, mem::replace};
use save_state::{SaveStateDeserializer, SaveStateSerializer};
use save_state_macro::*;

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

const GAUSS_INTERPOLATION_POINTS: [i32; 16 * 32] = [
    0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001,
    0x001, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x003, 0x003, 0x003, 0x003, 0x003,
    0x004, 0x004, 0x004, 0x004, 0x004, 0x005, 0x005, 0x005, 0x005, 0x006, 0x006, 0x006, 0x006,
    0x007, 0x007, 0x007, 0x008, 0x008, 0x008, 0x009, 0x009, 0x009, 0x00A, 0x00A, 0x00A, 0x00B,
    0x00B, 0x00B, 0x00C, 0x00C, 0x00D, 0x00D, 0x00E, 0x00E, 0x00F, 0x00F, 0x00F, 0x010, 0x010,
    0x011, 0x011, 0x012, 0x013, 0x013, 0x014, 0x014, 0x015, 0x015, 0x016, 0x017, 0x017, 0x018,
    0x018, 0x019, 0x01A, 0x01B, 0x01B, 0x01C, 0x01D, 0x01D, 0x01E, 0x01F, 0x020, 0x020, 0x021,
    0x022, 0x023, 0x024, 0x024, 0x025, 0x026, 0x027, 0x028, 0x029, 0x02A, 0x02B, 0x02C, 0x02D,
    0x02E, 0x02F, 0x030, 0x031, 0x032, 0x033, 0x034, 0x035, 0x036, 0x037, 0x038, 0x03A, 0x03B,
    0x03C, 0x03D, 0x03E, 0x040, 0x041, 0x042, 0x043, 0x045, 0x046, 0x047, 0x049, 0x04A, 0x04C,
    0x04D, 0x04E, 0x050, 0x051, 0x053, 0x054, 0x056, 0x057, 0x059, 0x05A, 0x05C, 0x05E, 0x05F,
    0x061, 0x063, 0x064, 0x066, 0x068, 0x06A, 0x06B, 0x06D, 0x06F, 0x071, 0x073, 0x075, 0x076,
    0x078, 0x07A, 0x07C, 0x07E, 0x080, 0x082, 0x084, 0x086, 0x089, 0x08B, 0x08D, 0x08F, 0x091,
    0x093, 0x096, 0x098, 0x09A, 0x09C, 0x09F, 0x0A1, 0x0A3, 0x0A6, 0x0A8, 0x0AB, 0x0AD, 0x0AF,
    0x0B2, 0x0B4, 0x0B7, 0x0BA, 0x0BC, 0x0BF, 0x0C1, 0x0C4, 0x0C7, 0x0C9, 0x0CC, 0x0CF, 0x0D2,
    0x0D4, 0x0D7, 0x0DA, 0x0DD, 0x0E0, 0x0E3, 0x0E6, 0x0E9, 0x0EC, 0x0EF, 0x0F2, 0x0F5, 0x0F8,
    0x0FB, 0x0FE, 0x101, 0x104, 0x107, 0x10B, 0x10E, 0x111, 0x114, 0x118, 0x11B, 0x11E, 0x122,
    0x125, 0x129, 0x12C, 0x130, 0x133, 0x137, 0x13A, 0x13E, 0x141, 0x145, 0x148, 0x14C, 0x150,
    0x153, 0x157, 0x15B, 0x15F, 0x162, 0x166, 0x16A, 0x16E, 0x172, 0x176, 0x17A, 0x17D, 0x181,
    0x185, 0x189, 0x18D, 0x191, 0x195, 0x19A, 0x19E, 0x1A2, 0x1A6, 0x1AA, 0x1AE, 0x1B2, 0x1B7,
    0x1BB, 0x1BF, 0x1C3, 0x1C8, 0x1CC, 0x1D0, 0x1D5, 0x1D9, 0x1DD, 0x1E2, 0x1E6, 0x1EB, 0x1EF,
    0x1F3, 0x1F8, 0x1FC, 0x201, 0x205, 0x20A, 0x20F, 0x213, 0x218, 0x21C, 0x221, 0x226, 0x22A,
    0x22F, 0x233, 0x238, 0x23D, 0x241, 0x246, 0x24B, 0x250, 0x254, 0x259, 0x25E, 0x263, 0x267,
    0x26C, 0x271, 0x276, 0x27B, 0x280, 0x284, 0x289, 0x28E, 0x293, 0x298, 0x29D, 0x2A2, 0x2A6,
    0x2AB, 0x2B0, 0x2B5, 0x2BA, 0x2BF, 0x2C4, 0x2C9, 0x2CE, 0x2D3, 0x2D8, 0x2DC, 0x2E1, 0x2E6,
    0x2EB, 0x2F0, 0x2F5, 0x2FA, 0x2FF, 0x304, 0x309, 0x30E, 0x313, 0x318, 0x31D, 0x322, 0x326,
    0x32B, 0x330, 0x335, 0x33A, 0x33F, 0x344, 0x349, 0x34E, 0x353, 0x357, 0x35C, 0x361, 0x366,
    0x36B, 0x370, 0x374, 0x379, 0x37E, 0x383, 0x388, 0x38C, 0x391, 0x396, 0x39B, 0x39F, 0x3A4,
    0x3A9, 0x3AD, 0x3B2, 0x3B7, 0x3BB, 0x3C0, 0x3C5, 0x3C9, 0x3CE, 0x3D2, 0x3D7, 0x3DC, 0x3E0,
    0x3E5, 0x3E9, 0x3ED, 0x3F2, 0x3F6, 0x3FB, 0x3FF, 0x403, 0x408, 0x40C, 0x410, 0x415, 0x419,
    0x41D, 0x421, 0x425, 0x42A, 0x42E, 0x432, 0x436, 0x43A, 0x43E, 0x442, 0x446, 0x44A, 0x44E,
    0x452, 0x455, 0x459, 0x45D, 0x461, 0x465, 0x468, 0x46C, 0x470, 0x473, 0x477, 0x47A, 0x47E,
    0x481, 0x485, 0x488, 0x48C, 0x48F, 0x492, 0x496, 0x499, 0x49C, 0x49F, 0x4A2, 0x4A6, 0x4A9,
    0x4AC, 0x4AF, 0x4B2, 0x4B5, 0x4B7, 0x4BA, 0x4BD, 0x4C0, 0x4C3, 0x4C5, 0x4C8, 0x4CB, 0x4CD,
    0x4D0, 0x4D2, 0x4D5, 0x4D7, 0x4D9, 0x4DC, 0x4DE, 0x4E0, 0x4E3, 0x4E5, 0x4E7, 0x4E9, 0x4EB,
    0x4ED, 0x4EF, 0x4F1, 0x4F3, 0x4F5, 0x4F6, 0x4F8, 0x4FA, 0x4FB, 0x4FD, 0x4FF, 0x500, 0x502,
    0x503, 0x504, 0x506, 0x507, 0x508, 0x50A, 0x50B, 0x50C, 0x50D, 0x50E, 0x50F, 0x510, 0x511,
    0x511, 0x512, 0x513, 0x514, 0x514, 0x515, 0x516, 0x516, 0x517, 0x517, 0x517, 0x518, 0x518,
    0x518, 0x518, 0x518, 0x519, 0x519,
];

const fn calculate_gain_noise_rates() -> [u16; 32] {
    let mut rates = [0; 32];
    macro_rules! gen_rates {
        (t0, $n:expr) => {
            rates[$n] = if $n < 0x1a {
                let inv = 0x22 - $n;
                let x = inv / 3;
                let y = inv % 3;
                (1 << (x - 2)) * y + (1 << x)
            } else {
                0x20 - $n
            }
        };
        (t1, $off:expr) => {
            gen_rates!(t0, $off);
            gen_rates!(t0, $off + 1);
        };
        (t2, $off:expr) => {
            gen_rates!(t1, $off);
            gen_rates!(t1, $off + 2);
            gen_rates!(t1, $off + 4);
            gen_rates!(t1, $off + 6);
        };
        (t3, $off:expr) => {
            gen_rates!(t2, $off);
            gen_rates!(t2, $off + 8);
            gen_rates!(t2, $off + 16);
            gen_rates!(t2, $off + 24);
        };
    }
    gen_rates!(t3, 0);
    rates
}

const ADSR_GAIN_NOISE_RATES: [u16; 32] = calculate_gain_noise_rates();

const DECODE_BUFFER_SIZE: usize = 3 + 16;

// 0x2f BRA: the 2 instead of 4 cycles are on purpose.
//           `branch_rel` will increment the cycle count
#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       2, 0, 4, 5, 3, 4, 3, 6,   2, 6, 5, 4, 5, 4, 6, 0,  // 0^
       2, 0, 4, 5, 4, 5, 5, 6,   5, 5, 6, 0, 2, 2, 0, 6,  // 1^
       2, 0, 4, 5, 3, 4, 3, 0,   2, 6, 5, 4, 0, 4, 5, 2,  // 2^
       2, 0, 4, 5, 4, 5, 5, 0,   5, 0, 6, 0, 2, 2, 3, 8,  // 3^
       2, 0, 4, 5, 3, 4, 0, 0,   2, 0, 0, 4, 5, 4, 6, 0,  // 4^
       0, 0, 4, 5, 4, 5, 5, 0,   5, 0, 4, 5, 2, 2, 4, 3,  // 5^
       2, 0, 4, 5, 3, 4, 3, 2,   2, 6, 0, 4, 0, 4, 5, 5,  // 6^
       0, 0, 4, 5, 4, 5, 5, 0,   5, 0, 5, 0, 2, 2, 3, 0,  // 7^
       2, 0, 4, 5, 3, 4, 0, 6,   2, 6, 5, 4, 5, 2, 4, 5,  // 8^
       2, 0, 4, 5, 4, 5, 5, 6,   5, 0, 5, 5, 2, 2,12, 5,  // 9^
       3, 0, 4, 5, 3, 4, 0, 0,   2, 0, 4, 4, 5, 2, 4, 4,  // a^
       2, 0, 4, 5, 4, 5, 5, 0,   0, 0, 5, 5, 2, 2, 0, 4,  // b^
       3, 0, 4, 5, 4, 5, 4, 7,   2, 5, 0, 4, 5, 2, 4, 9,  // c^
       2, 0, 4, 5, 5, 6, 6, 7,   4, 0, 5, 5, 2, 2, 6, 0,  // d^
       2, 0, 4, 5, 3, 4, 3, 6,   2, 4, 5, 3, 4, 3, 4, 0,  // e^
       2, 0, 4, 5, 4, 5, 5, 6,   3, 4, 5, 4, 2, 2, 4, 0,  // f^
];

const F0_RESET: u8 = 0x80;

/// Flags
pub mod flags {
    pub const CARRY: u8 = 0x01;
    pub const ZERO: u8 = 0x02;
    pub const INTERRUPT_ENABLE: u8 = 0x04;
    pub const HALF_CARRY: u8 = 0x08;
    pub const BREAK: u8 = 0x10;
    /// 0 means zero page is at 0x00xx,
    /// 1 means zero page is at 0x01xx
    pub const ZERO_PAGE: u8 = 0x20;
    pub const OVERFLOW: u8 = 0x40;
    pub const SIGN: u8 = 0x80;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum AdsrPeriod {
    Attack = 0,
    Decay = 1,
    Sustain = 2,
    Gain = 3,
    Release = 4,
}

impl save_state::InSaveState for AdsrPeriod {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as usize as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::Attack,
            1 => Self::Decay,
            2 => Self::Sustain,
            3 => Self::Gain,
            4 => Self::Release,
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct StereoSample<T: save_state::InSaveState> {
    pub l: T,
    pub r: T,
}

macro_rules! impl_new_for_stereo_sample {
    ($t:ty) => {
        impl StereoSample<$t> {
            pub const fn new(val: $t) -> Self {
                Self { l: val, r: val }
            }
            pub const fn new2(l: $t, r: $t) -> Self {
                Self { l, r }
            }
        }
    };
    ($t1:ty $(, $t:ty)*) => {
        impl_new_for_stereo_sample!($t1);
        impl_new_for_stereo_sample!($($t),*);
    };
}

impl_new_for_stereo_sample! { u8, i8, u16, i16, u32, i32 }

impl StereoSample<i16> {
    pub fn saturating_add32(self, val: StereoSample<i32>) -> Self {
        let clamped = val.clamp16();
        Self {
            l: self.l.saturating_add(clamped.l),
            r: self.r.saturating_add(clamped.r),
        }
    }
}

impl StereoSample<i32> {
    pub fn clamp16(self) -> StereoSample<i16> {
        StereoSample {
            l: self.l.clamp(-0x8000, 0x7fff) as i16,
            r: self.r.clamp(-0x8000, 0x7fff) as i16,
        }
    }

    pub fn clip16(self) -> StereoSample<i16> {
        StereoSample {
            l: (self.l as u32 & 0xffff) as i16,
            r: (self.r as u32 & 0xffff) as i16,
        }
    }
}

impl<T: Into<i32> + save_state::InSaveState> StereoSample<T> {
    pub fn to_i32(self) -> StereoSample<i32> {
        StereoSample {
            l: self.l.into(),
            r: self.r.into(),
        }
    }
}

impl core::ops::Mul for StereoSample<i32> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            l: self.l * other.l,
            r: self.r * other.r,
        }
    }
}

impl<T2: Copy, T1: core::ops::Mul<T2> + save_state::InSaveState> core::ops::Mul<T2>
    for StereoSample<T1>
where
    <T1 as core::ops::Mul<T2>>::Output: save_state::InSaveState,
{
    type Output = StereoSample<<T1 as core::ops::Mul<T2>>::Output>;
    fn mul(self, other: T2) -> Self::Output {
        Self::Output {
            l: self.l * other,
            r: self.r * other,
        }
    }
}

impl<R: Copy, T: core::ops::Shr<R> + save_state::InSaveState> core::ops::Shr<R> for StereoSample<T>
where
    T::Output: save_state::InSaveState,
{
    type Output = StereoSample<T::Output>;
    fn shr(self, rhs: R) -> Self::Output {
        StereoSample {
            l: self.l >> rhs,
            r: self.r >> rhs,
        }
    }
}

impl<T2: save_state::InSaveState, T1: core::ops::AddAssign<T2> + save_state::InSaveState>
    core::ops::AddAssign<StereoSample<T2>> for StereoSample<T1>
{
    fn add_assign(&mut self, rhs: StereoSample<T2>) {
        self.l += rhs.l;
        self.r += rhs.r;
    }
}

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Channel {
    volume: StereoSample<i8>,
    // pitch (corresponds to `pitch * 125/8 Hz`)
    pitch: u16,
    source_number: u8,
    dir_addr: u16,
    data_addr: u16,
    adsr: [u8; 2],
    gain_mode: u8,
    gain: u16,
    vx_env: u8,
    vx_out: u8,
    sustain: u16,
    unused: [u8; 3],
    fir_coefficient: i8,

    decode_buffer: [i16; DECODE_BUFFER_SIZE],
    pitch_counter: u16,
    period: AdsrPeriod,
    period_rate_map: [u16; 4],
    rate_index: u16,
    end_bit: bool,
    loop_bit: bool,
    last_sample: i16,
}

impl Channel {
    pub const fn new() -> Self {
        Self {
            volume: StereoSample::<i8>::new(0),
            pitch: 0,
            source_number: 0,
            dir_addr: 0,
            data_addr: 0,
            adsr: [0; 2],
            gain_mode: 0,
            gain: 0,
            vx_env: 0,
            vx_out: 0,
            sustain: 0,
            unused: [0; 3],
            fir_coefficient: 0,
            decode_buffer: [0; DECODE_BUFFER_SIZE],
            pitch_counter: 0,
            period: AdsrPeriod::Attack,
            period_rate_map: [0; 4],
            rate_index: 0,
            end_bit: false,
            loop_bit: false,
            last_sample: 0,
        }
    }

    pub fn update_gain(&mut self, rate: u16) {
        match self.period {
            AdsrPeriod::Attack => {
                self.gain = self
                    .gain
                    .saturating_add(if rate == 1 { 1024 } else { 32 })
                    .min(0x7ff);
                if self.gain > 0x7df {
                    self.period = AdsrPeriod::Decay
                }
            }
            AdsrPeriod::Decay | AdsrPeriod::Sustain => {
                self.gain = self
                    .gain
                    .saturating_sub((self.gain.saturating_sub(1) >> 8) + 1);
                if self.period == AdsrPeriod::Decay && self.gain < self.sustain {
                    self.period = AdsrPeriod::Sustain
                }
            }
            AdsrPeriod::Gain => todo!("gain mode"),
            AdsrPeriod::Release => panic!("`update_gain` must not be called in release mode"),
        }
    }

    pub fn reset(&mut self) {
        self.period = AdsrPeriod::Release;
        self.gain = 0;
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Dsp {
    // in milliseconds
    echo_delay: u16,
    echo_index: u16,
    source_dir_addr: u16,
    echo_data_addr: u16,
    channels: [Channel; 8],
    pitch_modulation: u8,
    echo_feedback: i8,
    noise: u8,
    echo: u8,
    fade_in: u8,
    fade_out: u8,
    // FLG register (6c)
    flags: u8,
    master_volume: StereoSample<i8>,
    echo_volume: StereoSample<i8>,
    unused: u8,
    echo_buffer_offset: u16,
    fir_buffer: [StereoSample<i16>; 8],
    fir_buffer_index: u8,
}

impl Dsp {
    pub const fn new() -> Self {
        Self {
            echo_delay: 1,
            echo_index: 1,
            source_dir_addr: 0,
            echo_data_addr: 0,
            channels: [Channel::new(); 8],
            pitch_modulation: 0,
            echo_feedback: 0,
            noise: 0,
            echo: 0,
            fade_in: 0,
            fade_out: 0,
            flags: 0xe0,
            master_volume: StereoSample::<i8>::new(0),
            echo_volume: StereoSample::<i8>::new(0),
            unused: 0,
            echo_buffer_offset: 0,
            fir_buffer: [StereoSample { l: 0, r: 0 }; 8],
            fir_buffer_index: 0,
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Spc700<B: AudioBackend> {
    mem: [u8; MEMORY_SIZE],
    /// data, the main processor sends to us
    pub input: [u8; 4],
    /// data, we send to the main processor
    pub output: [u8; 4],
    dsp: Dsp,
    #[except((|_v, _s| ()), (|_v, _s| ()))]
    pub backend: B,

    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    status: u8,
    pc: u16,

    timer_max: [u8; 3],
    // internal timer ticks ALL in 64kHz
    timers: [u8; 3],
    timer_enable: u8,
    counters: [Cell<u8>; 3],
    dispatch_counter: u16,
    pub(crate) master_cycles: Cycles,
    cycles_ahead: Cycles,
    timing_proportion: (Cycles, Cycles),
}

impl<B: AudioBackend> Spc700<B> {
    pub fn new(backend: B, is_pal: bool) -> Self {
        const fn generate_power_up_memory() -> [u8; MEMORY_SIZE] {
            let mut mem = [0; MEMORY_SIZE];
            mem[0xf0] = F0_RESET;
            mem
        }
        const POWER_UP_MEMORY: [u8; MEMORY_SIZE] = generate_power_up_memory();
        Self {
            mem: POWER_UP_MEMORY,
            input: [0; 4],
            output: [0; 4],
            dsp: Dsp::new(),
            backend,
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0xffc0,
            status: 0,

            timer_max: [0; 3],
            timers: [0; 3],
            timer_enable: 0,
            counters: [Cell::new(0), Cell::new(0), Cell::new(0)],
            dispatch_counter: 0,
            master_cycles: 0,
            cycles_ahead: 7,
            timing_proportion: if is_pal {
                APU_CPU_TIMING_PROPORTION_PAL
            } else {
                APU_CPU_TIMING_PROPORTION_NTSC
            },
        }
    }

    pub fn reset(&mut self) {
        self.mem[0xf0] = F0_RESET;
        self.input = [0; 4];
        self.output = [0; 4];
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        // actually self.read16(0xfffe), but this will
        // always result in 0xffc0, because mem[0xf0] = 0x80
        self.pc = 0xffc0;
        self.status = 0;
    }

    pub fn is_rom_mapped(&self) -> bool {
        self.mem[0xf0] & 0x80 > 0
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    fn read16_norom(&self, addr: u16) -> u16 {
        u16::from_le_bytes([
            self.mem[usize::from(addr)],
            self.mem[usize::from(addr.wrapping_add(1))],
        ])
    }

    fn write16_norom(&mut self, addr: u16, val: u16) {
        let [a, b] = val.to_le_bytes();
        self.mem[usize::from(addr)] = a;
        self.mem[usize::from(addr.wrapping_add(1))] = b;
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xf3 => self.read_dsp_register(self.mem[0xf2]),
            0xf4..=0xf7 => self.input[usize::from(addr - 0xf4)],
            0xfd..=0xff => self.counters[usize::from(addr - 0xfd)].take(),
            0xf1 | 0xf8..=0xff => {
                todo!("reading SPC register 0x{:02x}", addr)
            }
            0xffc0..=0xffff if self.is_rom_mapped() => ROM[(addr & 0x3f) as usize],
            addr => self.mem[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xf1 => {
                if val & 0x10 > 0 {
                    self.input[0..2].fill(0)
                }
                if val & 0x20 > 0 {
                    self.input[2..4].fill(0)
                }
                let active = val & !self.timer_enable;
                self.timer_enable = val & 7;
                for i in 0..3 {
                    if active & (1 << i) > 0 {
                        self.counters[i].set(0);
                        self.timers[i] = 0;
                    }
                }
            }
            0xf3 => self.write_dsp_register(self.mem[0xf2], val),
            0xf4..=0xf7 => self.output[(addr - 0xf4) as usize] = val,
            0xfa | 0xfb | 0xfc => self.timer_max[usize::from(!addr & 3) ^ 1] = val,
            0xf8..=0xff => {
                todo!("writing 0x{:02x} to SPC register 0x{:02x}", val, addr)
            }
            addr => self.mem[addr as usize] = val,
        }
    }

    pub fn read_dsp_register(&self, id: u8) -> u8 {
        let rid = id & 0x8f;
        if rid & 0xe != 0xc {
            let channel = &self.dsp.channels[usize::from(id >> 4)];
            match rid {
                0 => channel.volume.l as u8,
                1 => channel.volume.r as u8,
                2 => (channel.pitch & 0xff) as u8,
                3 => (channel.pitch >> 8) as u8,
                4 => channel.source_number,
                5 | 6 => channel.adsr[usize::from(!rid & 1)],
                7 => channel.gain_mode,
                8 => channel.vx_env,
                9 => channel.vx_out,
                10 => channel.unused[0],
                11 => channel.unused[1],
                14 => channel.unused[2],
                15 => channel.fir_coefficient as u8,
                _ => unreachable!(),
            }
        } else {
            match id {
                0x0c => self.dsp.master_volume.l as u8,
                0x1c => self.dsp.master_volume.r as u8,
                0x2c => self.dsp.echo_volume.l as u8,
                0x3c => self.dsp.echo_volume.r as u8,
                0x4c => self.dsp.fade_in,
                0x5c => self.dsp.fade_out,
                0x6c => self.dsp.flags,

                0x0d => self.dsp.echo_feedback as u8,
                0x1d => self.dsp.unused,
                0x2d => self.dsp.pitch_modulation,
                0x3d => self.dsp.noise,
                0x4d => self.dsp.echo,
                0x5d => (self.dsp.source_dir_addr >> 8) as u8,
                0x6d => (self.dsp.echo_data_addr >> 8) as u8,
                0x7d => (self.dsp.echo_delay >> 9) as u8,

                _ => todo!("read dsp register 0x{:02x}", id),
            }
        }
    }

    pub fn write_dsp_register(&mut self, id: u8, val: u8) {
        let rid = id & 0x8f;
        if rid & 0xe != 0xc {
            let channel = &mut self.dsp.channels[usize::from(id >> 4)];
            match rid {
                0 => channel.volume.l = val as i8,
                1 => channel.volume.r = val as i8,
                2 => channel.pitch = (channel.pitch & 0x3f00) | u16::from(val),
                3 => channel.pitch = (channel.pitch & 0xff) | (u16::from(val & 0x3f) << 8),
                4 => {
                    channel.source_number = val;
                    channel.dir_addr = self.dsp.source_dir_addr.wrapping_add(u16::from(val) << 2);
                }
                5 => {
                    channel.adsr[0] = val;
                    channel.period_rate_map[AdsrPeriod::Attack as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(((val & 0xf) << 1) | 1)];
                    channel.period_rate_map[AdsrPeriod::Decay as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(((val & 0x70) >> 3) | 0x10)];
                }
                6 => {
                    channel.adsr[1] = val;
                    channel.period_rate_map[AdsrPeriod::Sustain as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(val & 0x1f)];
                    channel.sustain = (u16::from(val >> 5) + 1) * 0x100;
                }
                7 => channel.gain_mode = val,
                8 => channel.vx_env = val,
                9 => channel.vx_out = val,
                10 => channel.unused[0] = val,
                11 => channel.unused[1] = val,
                14 => channel.unused[2] = val,
                15 => channel.fir_coefficient = val as i8,
                _ => unreachable!(),
            }
        } else {
            match id {
                0x0c => self.dsp.master_volume.l = val as i8,
                0x1c => self.dsp.master_volume.r = val as i8,
                0x2c => self.dsp.echo_volume.l = val as i8,
                0x3c => self.dsp.echo_volume.r = val as i8,
                0x4c => self.dsp.fade_in = val,
                0x5c => self.dsp.fade_out = val,
                0x6c => self.dsp.flags = val,

                0x0d => self.dsp.echo_feedback = val as i8,
                0x1d => self.dsp.unused = val,
                0x2d => self.dsp.pitch_modulation = val & 0xfe,
                0x3d => self.dsp.noise = val,
                0x4d => self.dsp.echo = val,
                0x5d => {
                    self.dsp.source_dir_addr = u16::from(val) << 8;
                    for channel in &mut self.dsp.channels {
                        channel.dir_addr = self
                            .dsp
                            .source_dir_addr
                            .wrapping_add(u16::from(channel.source_number) << 2)
                    }
                }
                0x6d => self.dsp.echo_data_addr = u16::from(val) << 8,
                0x7d => self.dsp.echo_delay = 1.max((val as u16 & 15) << 9),

                _ => todo!("write value 0x{:02x} dsp register 0x{:02x}", val, id),
            }
        }
    }

    pub fn get_small(&self, addr: u8) -> u16 {
        u16::from(addr) | (((self.status & flags::ZERO_PAGE) as u16) << 3)
    }

    pub fn read_small(&self, addr: u8) -> u8 {
        self.read(self.get_small(addr))
    }

    pub fn read16_small(&self, addr: u8) -> u16 {
        u16::from_le_bytes([self.read_small(addr), self.read_small(addr.wrapping_add(1))])
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        let [a, b] = val.to_le_bytes();
        self.write(addr, a);
        self.write(addr.wrapping_add(1), b);
    }

    pub fn write_small(&mut self, addr: u8, val: u8) {
        self.write(self.get_small(addr), val)
    }

    pub fn write16_small(&mut self, addr: u8, val: u16) {
        let [a, b] = val.to_le_bytes();
        self.write_small(addr, a);
        self.write_small(addr.wrapping_add(1), b)
    }

    pub fn push(&mut self, val: u8) {
        self.write(u16::from(self.sp) | 0x100, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    pub fn push16(&mut self, val: u16) {
        let [a, b] = val.to_be_bytes();
        self.push(a);
        self.push(b)
    }

    pub fn pull(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.read(u16::from(self.sp) | 0x100)
    }

    pub fn pull16(&mut self) -> u16 {
        u16::from_le_bytes([self.pull(), self.pull()])
    }

    pub fn load(&mut self) -> u8 {
        let val = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    pub fn load16(&mut self) -> u16 {
        let val = self.read16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        val
    }

    pub fn ya(&self) -> u16 {
        u16::from_le_bytes([self.a, self.y])
    }

    pub fn set_ya(&mut self, val: u16) {
        let [a, y] = val.to_le_bytes();
        self.a = a;
        self.y = y;
    }

    pub fn set_status(&mut self, cond: bool, flag: u8) {
        if cond {
            self.status |= flag
        } else {
            self.status &= !flag
        }
    }

    pub fn sound_cycle(&mut self) {
        let (fade_in, fade_out) = if self.dispatch_counter & 0x3f > 0 {
            let new = self.dsp.fade_in & self.dsp.fade_out;
            (replace(&mut self.dsp.fade_in, new), self.dsp.fade_out)
        } else {
            (0, 0)
        };
        if self.dsp.flags & 0x80 > 0 {
            for channel in self.dsp.channels.iter_mut() {
                channel.reset()
            }
        }
        let mut last_sample = 0;
        let mut result = StereoSample::<i16>::new(0);
        for (i, channel) in self.dsp.channels.iter_mut().enumerate() {
            if fade_out & (1 << i) > 0 {
                channel.period = AdsrPeriod::Release
            } else if fade_in & (1 << i) > 0 {
                channel.data_addr = u16::from_le_bytes([
                    self.mem[usize::from(channel.dir_addr)],
                    self.mem[usize::from(channel.dir_addr.wrapping_add(1))],
                ]);
                channel.loop_bit = false;
                channel.end_bit = false;
                channel.gain = 0;
                channel.decode_buffer.fill(0);
                channel.period = if channel.adsr[0] & 0x80 > 0 {
                    AdsrPeriod::Attack
                } else {
                    AdsrPeriod::Gain
                };
            }
            let step = if self.dsp.pitch_modulation & (1 << i) > 0 && i != 0 {
                let factor = (last_sample >> 4) + 0x400;
                ((i32::from(channel.pitch) * i32::from(factor)) >> 10) as u16
            } else {
                channel.pitch as u16
            };
            let (new_pitch_counter, ov) = channel.pitch_counter.overflowing_add(step);
            channel.pitch_counter = new_pitch_counter;
            if ov {
                if channel.end_bit {
                    channel.data_addr = u16::from_le_bytes([
                        self.mem[usize::from(channel.dir_addr.wrapping_add(2))],
                        self.mem[usize::from(channel.dir_addr.wrapping_add(3))],
                    ]);
                    if !channel.loop_bit {
                        channel.reset()
                    }
                }
                channel
                    .decode_buffer
                    .copy_within(DECODE_BUFFER_SIZE - 3..DECODE_BUFFER_SIZE, 0);
                let header = self.mem[usize::from(channel.data_addr)];
                channel.end_bit = header & 1 > 0;
                channel.loop_bit = header & 2 > 0;
                channel.data_addr = channel.data_addr.wrapping_add(1);
                for byte_id in 0usize..8 {
                    let byte = self.mem[usize::from(channel.data_addr)];
                    channel.data_addr = channel.data_addr.wrapping_add(1);
                    let index = byte_id << 1;
                    for (nibble_id, sample) in once(byte >> 4).chain(once(byte & 0xf)).enumerate() {
                        let index = index | nibble_id;
                        let sample = if sample & 8 > 0 {
                            (sample | 0xf0) as i8
                        } else {
                            sample as i8
                        };
                        let sample = match header >> 4 {
                            0 => i16::from(sample) >> 1,
                            s @ 1..=12 => i16::from(sample) << (s - 1),
                            13..=15 => i16::from(sample >> 3) << 11,
                            _ => unreachable!(),
                        };
                        let older = channel.decode_buffer[index + 1];
                        let old = channel.decode_buffer[index + 2];
                        let sample = (match header & 0b1100 {
                            0 => sample.into(),
                            0b0100 => (i32::from(sample) + i32::from(old) + (-i32::from(old) >> 4)),
                            0b1000 => {
                                i32::from(sample)
                                    + i32::from(old) * 2
                                    + ((-3 * i32::from(old)) >> 5)
                                    - i32::from(older)
                                    + i32::from(older >> 4)
                            }
                            0b1100 => {
                                i32::from(sample)
                                    + i32::from(old) * 2
                                    + ((-13 * i32::from(old)) >> 6)
                                    - i32::from(older)
                                    + ((i32::from(older) * 3) >> 4)
                            }
                            _ => unreachable!(),
                        })
                        .clamp(-0x8000, 0x7fff) as i16;
                        // this behaviour is documented by nocash FullSNES
                        let sample = if sample > 0x3fff {
                            -0x8000 + sample
                        } else if sample < -0x4000 {
                            sample - -0x8000
                        } else {
                            sample
                        };
                        channel.decode_buffer[index + 3] = sample
                    }
                }
            }
            let interpolation_index = (channel.pitch_counter >> 4) & 0xff;
            let brr_index = usize::from(channel.pitch_counter >> 12);
            let sample = (GAUSS_INTERPOLATION_POINTS[usize::from(0xff - interpolation_index)]
                * i32::from(channel.decode_buffer[brr_index]))
                >> 10;
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(0x1ff - interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 1]))
                    >> 10);
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(0x100 + interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 2]))
                    >> 10);
            let sample = i32::from((sample & 0xffff) as i16);
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 3]))
                    >> 10);
            let sample = (sample.clamp(i16::MIN.into(), i16::MAX.into()) as i16) >> 1;

            if let AdsrPeriod::Release = channel.period {
                let (new_gain, ov) = channel.gain.overflowing_sub(8);
                channel.gain = if ov || new_gain > 0x7ff { 0 } else { new_gain };
            } else {
                // `channel.period as usize` will always be < 4
                let rate = channel.period_rate_map[channel.period as usize];
                if channel.gain_mode & 0x80 == 0 && channel.adsr[0] & 0x80 == 0 {
                    channel.gain = (channel.gain_mode & 0x7f).into()
                } else if rate > 0 {
                    channel.rate_index = channel.rate_index.wrapping_add(1);
                    if channel.rate_index >= rate {
                        channel.rate_index = 0;
                        channel.update_gain(rate)
                    }
                }
            }
            debug_assert!(channel.gain < 0x800);
            let sample = ((i32::from(sample) * i32::from(channel.gain)) >> 11) as i16;
            last_sample = sample;
            channel.last_sample = sample;
            channel.vx_env = (channel.gain >> 4) as u8; // TODO: really `>> 4`?
            channel.vx_out = (sample >> 7) as u8;
            result = result.saturating_add32(
                (StereoSample::<i32>::new(sample.into()) * channel.volume.to_i32()) >> 6,
            );
        }
        let sample = ((result.to_i32() * self.dsp.master_volume.to_i32()) >> 7).clamp16();
        let echo_addr = self
            .dsp
            .echo_data_addr
            .wrapping_add(self.dsp.echo_buffer_offset);
        self.dsp.echo_buffer_offset += 4;
        self.dsp.fir_buffer[usize::from(self.dsp.fir_buffer_index)] = StereoSample::<i16>::new2(
            self.read16_norom(echo_addr) as i16,
            self.read16_norom(echo_addr.wrapping_add(2)) as i16,
        ) >> 1;
        let mut result = StereoSample::<i32>::new(0);
        for i in 0..8 {
            let fir_value =
                self.dsp.fir_buffer[usize::from(self.dsp.fir_buffer_index + i + 1) & 7].to_i32();
            result +=
                (fir_value * i32::from(self.dsp.channels[usize::from(i)].fir_coefficient)) >> 6;
            if i == 6 {
                result = result.clip16().to_i32()
            }
        }
        self.dsp.fir_buffer_index = (self.dsp.fir_buffer_index + 1) & 7;
        let result = result.clamp16();
        let sample =
            sample.saturating_add32((result.to_i32() * self.dsp.echo_volume.to_i32()) >> 7);
        if self.dsp.flags & 0x20 == 0 {
            let sample = self
                .dsp
                .channels
                .iter()
                .enumerate()
                .filter(|(i, _)| self.dsp.echo & (1u8 << *i) > 0)
                .map(|(_, c)| (c.volume.to_i32() * c.last_sample as i32) >> 6)
                .fold(StereoSample::<i16>::new(0), StereoSample::saturating_add32);
            let sample =
                sample.saturating_add32((result.to_i32() * i32::from(self.dsp.echo_feedback)) >> 7);
            let sample = StereoSample::<i16>::new2(
                (sample.l as u16 & 0xfffe) as i16,
                (sample.r as u16 & 0xfffe) as i16,
            );
            self.write16_norom(echo_addr, sample.l as u16);
            self.write16_norom(echo_addr.wrapping_add(2), sample.r as u16);
        }
        self.dsp.echo_index -= 1;
        if self.dsp.echo_index == 0 {
            self.dsp.echo_index = self.dsp.echo_delay;
            self.dsp.echo_buffer_offset = 0;
        }
        // TODO: noise
        let sample = if self.dsp.flags & 0x40 > 0 {
            StereoSample::<i16>::new(0)
        } else {
            sample
        };
        self.backend.push_sample(sample)
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let op = self.load();
        let mut cycles = CYCLES[op as usize];
        match op {
            0x00 => (), // NOP
            0x02 | 0x22 | 0x42 | 0x62 | 0x82 | 0xa2 | 0xc2 | 0xe2 => {
                // SET1 - (imm) |= 1 << ?
                let addr = self.load();
                let addr = self.get_small(addr);
                self.write(addr, self.read(addr) | 1 << (op >> 5))
            }
            0x12 | 0x32 | 0x52 | 0x72 | 0x92 | 0xb2 | 0xd2 | 0xf2 => {
                // CLR1 - (imm) &= ~(1 << ?)
                let addr = self.load();
                let addr = self.get_small(addr);
                self.write(addr, self.read(addr) & !(1 << (op >> 5)))
            }
            0x03 | 0x23 | 0x43 | 0x63 | 0x83 | 0xa3 | 0xc3 | 0xe3 | 0x13 | 0x33 | 0x53 | 0x73
            | 0x93 | 0xb3 | 0xd3 | 0xf3 => {
                // Branch if bit set/cleared
                let addr = self.load();
                let val = self.read_small(addr);
                let rel = self.load();
                self.branch_rel(rel, ((val >> (op >> 5)) ^ (op >> 4)) & 1 == 1, &mut cycles);
            }
            0x04 => {
                // OR - A |= (imm)
                let addr = self.load();
                self.a |= self.read_small(addr);
                self.update_nz8(self.a);
            }
            0x05 => {
                // OR - A |= (imm[16-bit])
                let addr = self.load16();
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x06 => {
                // OR - A |= (X)
                self.a |= self.read_small(self.x);
                self.update_nz8(self.a);
            }
            0x07 => {
                // OR - A |= ((imm + X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                self.a |= self.read(self.read16_small(addr));
                self.update_nz8(self.a);
            }
            0x08 => {
                // OR - A |= imm
                self.a |= self.load();
                self.update_nz8(self.a)
            }
            0x09 => {
                // OR - (imm) |= (imm)
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let val = self.read_small(src) | self.read(dst);
                self.write(dst, val);
                self.update_nz8(val);
            }
            0x0a => {
                // OR1 - OR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status |= (val >> (addr >> 13)) & flags::CARRY
            }
            0x0b => {
                // ASL - (imm) <<= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let mut val = self.read(addr);
                self.set_status(val >= 0x80, flags::CARRY);
                val <<= 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x0c => {
                // ASL - (a) <<= 1
                let addr = self.load16();
                let mut val = self.read(addr);
                self.set_status(val >= 0x80, flags::CARRY);
                val <<= 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x0d => {
                // PUSH - status
                self.push(self.status)
            }
            0x0e => {
                // TSET1 - (imm[16-bit]) |= A
                let addr = self.load16();
                let val = self.read(addr);
                self.update_nz8(self.a.wrapping_add(!val).wrapping_add(1));
                self.write(addr, val | self.a)
            }
            0x10 => {
                // BPL/JNS - Branch if SIGN not set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::SIGN == 0, &mut cycles)
            }
            0x14 => {
                // OR - A |= (imm + X)
                let addr = self.load().wrapping_add(self.x);
                self.a |= self.read_small(addr);
                self.update_nz8(self.a);
            }
            0x15 => {
                // OR - A |= (imm[16-bit] + X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x16 => {
                // OR - A |= (imm[16-bit] + Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x17 => {
                // OR - A |= ((imm)[16-bit] + Y)
                let addr = self.load();
                self.a |= self.read(self.read16_small(addr).wrapping_add(self.y.into()));
                self.update_nz8(self.a);
            }
            0x18 => {
                // OR - (imm) |= imm
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let val = src | self.read(dst);
                self.write(dst, val);
                self.update_nz8(val);
            }
            0x19 => {
                // OR - (X) |= (Y)
                let x = self.get_small(self.x);
                let res = self.read(x) | self.read_small(self.y);
                self.write(x, res);
                self.update_nz8(res)
            }
            0x1a => {
                // DECW - (imm)[16-bit]--
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_sub(1);
                self.write16(addr, val);
                self.update_nz16(val)
            }
            0x1c => {
                // ASL - A <<= 1
                self.set_status(self.a >= 0x80, flags::CARRY);
                self.a <<= 1;
                self.update_nz8(self.a)
            }
            0x1d => {
                // DEC - X
                self.x = self.x.wrapping_sub(1);
                self.update_nz8(self.x);
            }
            0x1f => {
                // JMP - PC := (X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.pc = self.read16(addr);
            }
            0x20 => {
                // CLRP - Clear ZERO_PAGE
                self.status &= !flags::ZERO_PAGE
            }
            0x24 => {
                // AND - A &= (imm)
                let addr = self.load();
                self.a &= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x25 => {
                // AND - A &= (imm[16-bit])
                let addr = self.load16();
                self.a &= self.read(addr);
                self.update_nz8(self.a)
            }
            0x26 => {
                // AND - A &= (X)
                self.a &= self.read_small(self.x);
                self.update_nz8(self.a)
            }
            0x28 => {
                // AND - A &= imm
                self.a &= self.load();
                self.update_nz8(self.a)
            }
            0x29 => {
                // AND - (imm) &= (imm)
                let src = self.load();
                let dst = self.load();
                let [src, dst] = [src, dst].map(|v| self.get_small(v));
                let val = self.read(src) & self.read(dst);
                self.write(dst, val);
                self.update_nz8(val)
            }
            0x2a => {
                // OR1 - NOR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = !self.read(addr & 0x1fff);
                self.status |= (val >> (addr >> 13)) & flags::CARRY
            }
            0x2b => {
                // ROL - (imm) <<= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val << 1) | (self.status & flags::CARRY);
                self.set_status(val >= 0x80, flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x2d => {
                // PUSH - A
                self.push(self.a)
            }
            0x2e => {
                // CBNE - Branch if A != (imm)
                let addr = self.load();
                let rel = self.load();
                self.branch_rel(rel, self.read_small(addr) != self.a, &mut cycles)
            }
            0x2f => {
                // BRA - Branch always
                let rel = self.load();
                self.branch_rel(rel, true, &mut cycles)
            }
            0x30 => {
                // BMI - Branch if SIGN is set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::SIGN > 0, &mut cycles)
            }
            0x34 => {
                // AND - A &= (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a &= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x35 => {
                // AND - A &= (imm[16-bit] + X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a &= self.read(addr);
                self.update_nz8(self.a);
            }
            0x36 => {
                // AND - A &= (imm[16-bit] + Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a &= self.read(addr);
                self.update_nz8(self.a);
            }
            0x38 => {
                // AND - (imm) &= imm
                let imm = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr) & imm;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x3a => {
                // INCW - (imm)[16-bit]++
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_add(1);
                self.write16(addr, val);
                self.update_nz16(val)
            }
            0x3c => {
                // ROL - A <<= 1
                let c = self.a & 0x80;
                self.a = (self.a << 1) | (self.status & flags::CARRY);
                self.set_status(c > 0, flags::CARRY);
                self.update_nz8(self.a);
            }
            0x3d => {
                // INC - X
                self.x = self.x.wrapping_add(1);
                self.update_nz8(self.x);
            }
            0x3e => {
                // CMP - X - (imm)
                let addr = self.load();
                let val = self.read_small(addr);
                self.compare(self.x, val)
            }
            0x3f => {
                // CALL - Call a subroutine
                let addr = self.load16();
                self.push16(self.pc);
                self.pc = addr
            }
            0x40 => {
                // SETP - Set ZERO_PAGE
                self.status |= flags::ZERO_PAGE
            }
            0x44 => {
                // EOR - A := A ^ (imm)
                let addr = self.load();
                self.a ^= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x45 => {
                // EOR - A := a ^ (imm[16-bit])
                let addr = self.load16();
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x48 => {
                // EOR - A := A ^ imm
                self.a ^= self.load();
                self.update_nz8(self.a)
            }
            0x4b => {
                // LSR - (imm) >>= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x4c => {
                // LSR - (imm[16-bit]) >>= 1
                let addr = self.load16();
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x4d => {
                // PUSH - X
                self.push(self.x)
            }
            0x4e => {
                // TCLR1
                let addr = self.load16();
                let val = self.read(addr);
                self.update_nz8(self.a.wrapping_add(!val).wrapping_add(1));
                self.write(addr, val & !self.a)
            }
            0x54 => {
                // EOR - A := A ^ (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a ^= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x55 => {
                // EOR - A := A ^ (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x56 => {
                // EOR - A := A ^ (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x58 => {
                // EOR - (imm) ^= imm
                let val = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr) ^ val;
                self.write(addr, val);
                self.update_nz8(val);
            }
            0x5a => {
                // CMPW - YA - (imm)[16-bit]
                let val = self.load();
                let (result, ov1) = self.ya().overflowing_add(!self.read16_small(val));
                let (result, ov2) = result.overflowing_add(1);
                self.set_status(ov1 || ov2, flags::CARRY);
                self.update_nz16(result);
            }
            0x5b => {
                // LSR - (imm+X) >>= 1
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x5c => {
                // LSR - A >>= 1
                self.set_status(self.a & 1 > 0, flags::CARRY);
                self.a >>= 1;
                self.update_nz8(self.a)
            }
            0x5d => {
                // MOV - X := A
                self.x = self.a;
                self.update_nz8(self.x)
            }
            0x5e => {
                // CMP - Y - (imm[16-bit])
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.y, val)
            }
            0x5f => {
                // JMP - PC := imm[16-bit]
                self.pc = self.load16();
            }
            0x60 => {
                // CLRC - Clear CARRY
                self.status &= !flags::CARRY
            }
            0x64 => {
                // CMP - A - (imm)
                let addr = self.load();
                let val = self.read_small(addr);
                self.compare(self.a, val)
            }
            0x65 => {
                // CMP - A - (imm[16-bit])
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x66 => {
                // CMP - A - (X)
                self.compare(self.a, self.read_small(self.x))
            }
            0x68 => {
                // CMP - A - imm
                let val = self.load();
                self.compare(self.a, val)
            }
            0x69 => {
                // CMP - (dp) - (dp)
                let val1 = self.load();
                let val1 = self.read_small(val1);
                let val2 = self.load();
                let val2 = self.read_small(val2);
                self.compare(val2, val1);
            }
            0x6b => {
                // ROR - (imm) >>= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (val & flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x6d => {
                // PUSH - Y
                self.push(self.y)
            }
            0x6e => {
                // DBNZ - (imm)--; JNZ
                let addr = self.load();
                let rel = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.branch_rel(rel, val > 0, &mut cycles)
            }
            0x6f => {
                // RET - Return from subroutine
                self.pc = self.pull16()
            }
            0x74 => {
                // CMP - A - (imm+X)
                let addr = self.load().wrapping_add(self.x);
                let val = self.read_small(addr);
                self.compare(self.a, val)
            }
            0x75 => {
                // CMP - A - (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x76 => {
                // CMP - A - (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x78 => {
                // CMP - (imm) - imm
                let (b, a) = (self.load(), self.load());
                let a = self.read_small(a);
                self.compare(a, b)
            }
            0x7a => {
                // ADDW - YA += (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                let val = self.add16(self.ya(), val);
                self.set_ya(val);
            }
            0x7c => {
                // ROR - A >>= 1
                let new_a = (self.a >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (self.a & flags::CARRY);
                self.a = new_a;
                self.update_nz8(new_a);
            }
            0x7d => {
                // MOV - A := X
                self.a = self.x;
                self.update_nz8(self.a)
            }
            0x7e => {
                // CMP - Y - (imm)
                let addr = self.load();
                self.compare(self.y, self.read_small(addr))
            }
            0x80 => {
                // SETC - Set CARRY
                self.status |= flags::CARRY
            }
            0x84 => {
                // ADC - A += (imm) + CARRY
                let addr = self.load();
                let val = self.read_small(addr);
                self.a = self.adc(self.a, val)
            }
            0x85 => {
                // ADC - A += (imm[16-bit]) + CARRY
                let addr = self.load16();
                let val = self.read(addr);
                self.a = self.adc(self.a, val)
            }
            0x87 => {
                // ADC - A += ((imm+X)[16-bit]) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, self.read(self.read16_small(addr)))
            }
            0x88 => {
                // ADC - A += imm + CARRY
                let val = self.load();
                self.a = self.adc(self.a, val)
            }
            0x89 => {
                // ADC - (imm) += (imm)
                let addr1 = self.load();
                let addr1 = self.get_small(addr1);
                let addr2 = self.load();
                let addr2 = self.get_small(addr2);
                let result = self.adc(self.read(addr2), self.read(addr1));
                self.write(addr2, result);
            }
            0x8a => {
                // EOR1 - XOR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status ^= (val >> (addr >> 13)) & flags::CARRY
            }
            0x8b => {
                // DEC - Decrement (imm)
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x8c => {
                // DEC - (imm[16-bit])--
                let addr = self.load16();
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x8d => {
                // MOV - Y := IMM
                self.y = self.load();
                self.update_nz8(self.y);
            }
            0x8e => {
                // POP - status
                self.status = self.pull()
            }
            0x8f => {
                // MOV - (dp) := IMM
                let (val, addr) = (self.load(), self.load());
                self.write_small(addr, val);
            }
            0x90 => {
                // BCC - Branch if CARRY not set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::CARRY == 0, &mut cycles)
            }
            0x94 => {
                // ADC - A += (imm + X) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, self.read_small(addr));
            }
            0x95 => {
                // ADC - A -= (imm16 + X) + CARRY
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.adc(self.a, self.read(addr));
            }
            0x96 => {
                // ADC - A -= (imm16 + Y) + CARRY
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.adc(self.a, self.read(addr));
            }
            0x97 => {
                // ADC - A += ((imm)[16-bit] + Y) + CARRY
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a = self.adc(self.a, self.read(addr))
            }
            0x98 => {
                // ADC - (imm) += imm + CARRY
                let val = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.adc(self.read(addr), val);
                self.write(addr, val)
            }
            0x9a => {
                // SUBW - YA -= (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                self.status |= flags::CARRY;
                let val = self.adc16(self.ya(), !val);
                self.set_ya(val);
            }
            0x9b => {
                // DEC - (imm+X)[16-bit]--
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val);
            }
            0x9c => {
                // DEC - A
                self.a = self.a.wrapping_sub(1);
                self.update_nz8(self.a);
            }
            0x9d => {
                // MOV - X := SP
                self.x = self.sp;
                self.update_nz8(self.x);
            }
            0x9e => {
                // DIV - Y, A := YA % X, YA / X
                // TODO: no exact reproduction of behaviour (see bsnes impl)
                let (rdiv, rmod) = if self.x == 0 {
                    (0xffff, self.a)
                } else {
                    let ya = self.ya();
                    let x = u16::from(self.x);
                    (ya / x, (ya % x) as u8)
                };
                self.set_status(rdiv > 0xff, flags::OVERFLOW);
                // TODO: understand why this works and what exactly HALF_CARRY does
                // This will probably work, because bsnes does this
                self.set_status((self.x & 15) <= (self.y & 15), flags::HALF_CARRY);
                self.a = (rdiv & 0xff) as u8;
                self.y = rmod;
                self.update_nz8(self.a);
            }
            0x9f => {
                // XCN - A := (A >> 4) | (A << 4)
                self.a = (self.a >> 4) | (self.a << 4);
                self.update_nz8(self.a)
            }
            0xa0 => {
                // EI - Set INTERRUPT_ENABLE
                self.status |= flags::INTERRUPT_ENABLE
            }
            0xa4 => {
                // SBC - A -= (imm) + CARRY
                let addr = self.load();
                self.a = self.adc(self.a, !self.read_small(addr));
            }
            0xa5 => {
                // SBC - A -= (imm[16-bit]) + CARRY
                let addr = self.load16();
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xa8 => {
                // SBC - A -= imm + CARRY
                let val = self.load();
                self.a = self.adc(self.a, !val);
            }
            0xaa => {
                // MOV1 - Set CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status = (self.status & !flags::CARRY) | ((val >> (addr >> 13)) & flags::CARRY)
            }
            0xab => {
                // INC - Increment (imm)
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0xac => {
                // INC - (imm[16-bit])++
                let addr = self.load16();
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0xad => {
                // CMP - Y - IMM
                let val = self.load();
                self.compare(self.y, val)
            }
            0xae => {
                // POP - A
                self.a = self.pull()
            }
            0xaf => {
                // MOV - (X) := A; X++
                self.write_small(self.x, self.a);
                self.x = self.x.wrapping_add(1);
            }
            0xb0 => {
                // BCS - Jump if CARRY set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::CARRY > 0, &mut cycles)
            }
            0xb4 => {
                // SBC - A -= (imm + X) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, !self.read_small(addr));
            }
            0xb5 => {
                // SBC - A -= (imm16 + X) + CARRY
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xb6 => {
                // SBC - A -= (imm16 + Y) + CARRY
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xba => {
                // MOVW - YA := (imm)[16-bit]
                let addr = self.load();
                let value = self.read16_small(addr);
                let [a, y] = value.to_le_bytes();
                self.a = a;
                self.y = y;
                self.update_nz16(value);
            }
            0xbb => {
                // INC - (imm + X)++
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val);
            }
            0xbc => {
                // INC - A
                self.a = self.a.wrapping_add(1);
                self.update_nz8(self.a);
            }
            0xbd => {
                // MOV - SP := X
                self.sp = self.x
            }
            0xbf => {
                // MOV - A := (X++)
                self.a = self.read_small(self.x);
                self.x = self.x.wrapping_add(1);
                self.update_nz8(self.a)
            }
            0xc0 => {
                // DI - Clear INTERRUPT_ENABLE
                self.status &= !flags::INTERRUPT_ENABLE
            }
            0xc4 => {
                // MOV - (db) := A
                let addr = self.load();
                self.write_small(addr, self.a)
            }
            0xc5 => {
                // MOV - (imm[16-bit]) := A
                let addr = self.load16();
                self.write(addr, self.a)
            }
            0xc6 => {
                // MOV - (X) := A
                self.write_small(self.x, self.a)
            }
            0xc7 => {
                // MOV - ((imm+X)[16-bit]) := A
                let addr = self.load().wrapping_add(self.x);
                let addr = self.read16_small(addr);
                self.write(addr, self.a)
            }
            0xc8 => {
                // CMP - X - IMM
                let val = self.load();
                self.compare(self.x, val)
            }
            0xc9 => {
                // MOV - (imm[16-bit]) := X
                let addr = self.load16();
                self.write(addr, self.x)
            }
            0xcb => {
                // MOV - (imm) := Y
                let addr = self.load();
                self.write_small(addr, self.y)
            }
            0xcc => {
                // MOV - (imm[16-bit]) := Y
                let addr = self.load16();
                self.write(addr, self.y)
            }
            0xcd => {
                // MOV - X := IMM
                self.x = self.load();
                self.update_nz8(self.x);
            }
            0xce => {
                // POP - X
                self.x = self.pull()
            }
            0xcf => {
                // MUL - YA := Y * A
                self.set_ya(u16::from(self.y) * u16::from(self.a));
                self.update_nz8(self.y);
            }
            0xd0 => {
                // BNE/JNZ - if not Zero
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::ZERO == 0, &mut cycles)
            }
            0xd4 => {
                // MOV - (imm+X) := A
                let addr = self.load().wrapping_add(self.x);
                self.write_small(addr, self.a)
            }
            0xd5 => {
                // MOV - (imm[16-bit]+X) := A
                let addr = self.load16().wrapping_add(self.x.into());
                self.write(addr, self.a)
            }
            0xd6 => {
                // MOV - (imm[16-bit]+Y) := A
                let addr = self.load16().wrapping_add(self.y.into());
                self.write(addr, self.a)
            }
            0xd7 => {
                // MOV - ((db)[16-bit] + Y) := A
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.write(addr, self.a);
            }
            0xd8 => {
                // MOV - (imm) := X
                let addr = self.load();
                self.write_small(addr, self.x)
            }
            0xda => {
                // MOVW - (imm)[16-bit] := YA
                // TODO: calculate cyles as if only one byte written
                let addr = self.load();
                self.write16_small(addr, u16::from_le_bytes([self.a, self.y]));
            }
            0xdb => {
                // MOV - (imm+X) := Y
                let addr = self.load().wrapping_add(self.x);
                self.write_small(addr, self.y)
            }
            0xdc => {
                // DEC - Y
                self.y = self.y.wrapping_sub(1);
                self.update_nz8(self.y);
            }
            0xdd => {
                // MOV - A := Y
                self.a = self.y;
                self.update_nz8(self.a)
            }
            0xde => {
                // CBNE - Branch if A != (imm+X)
                let addr = self.load().wrapping_add(self.x);
                let val = self.read_small(addr);
                let rel = self.load();
                self.branch_rel(rel, self.a != val, &mut cycles)
            }
            0xe4 => {
                // MOV - A := (imm)
                let addr = self.load();
                self.a = self.read_small(addr);
                self.update_nz8(self.a);
            }
            0xe5 => {
                // MOV - A := (imm[16-bit])
                let addr = self.load16();
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xe8 => {
                // MOV - A := IMM
                self.a = self.load();
                self.update_nz8(self.a);
            }
            0xe9 => {
                // MOV - X := (imm[16-bit])
                let addr = self.load16();
                self.x = self.read(addr);
                self.update_nz8(self.x);
            }
            0xea => {
                // NOT1 - Complement Bit in Memory address
                let imm = self.load16();
                let addr = imm & 0x1fff;
                let val = self.read(addr) ^ (1u8 << (imm >> 13));
                self.write(addr, val)
            }
            0xeb => {
                // MOV - Y := (IMM)
                let addr = self.load();
                self.y = self.read_small(addr);
                self.update_nz8(self.y)
            }
            0xe0 => {
                // CLRV - Clear OVERFLOW and HALF_CARRY
                self.status &= !(flags::OVERFLOW | flags::HALF_CARRY)
            }
            0xe6 => {
                // MOV - A := (X)
                self.a = self.read_small(self.x);
                self.update_nz8(self.a)
            }
            0xe7 => {
                // MOV - A := ((imm[16-bit]+X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                self.a = self.read(self.read16_small(addr));
                self.update_nz8(self.a);
            }
            0xec => {
                // MOV - Y := (imm[16-bit])
                let addr = self.load16();
                self.y = self.read(addr);
                self.update_nz8(self.y);
            }
            0xed => {
                // NOTC - Complement CARRY
                self.status ^= flags::CARRY
            }
            0xee => {
                // POP - Y
                self.y = self.pull()
            }
            0xf0 => {
                // BEQ - Branch if ZERO is set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::ZERO > 0, &mut cycles)
            }
            0xf4 => {
                // MOV - A := (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a = self.read_small(addr);
                self.update_nz8(self.a);
            }
            0xf5 => {
                // MOV - A := (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf6 => {
                // MOV - A := (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf7 => {
                // MOV - A := ((imm)[16-bit]+Y)
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf8 => {
                // MOV - X := (imm)
                let addr = self.load();
                self.x = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xf9 => {
                // MOV - X := (imm+Y)
                let addr = self.load().wrapping_add(self.y);
                self.x = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xfa => {
                // MOV - (dp) := (dp)
                let val1 = self.load();
                let val1 = self.read_small(val1);
                let val2 = self.load();
                self.write_small(val2, val1);
            }
            0xfb => {
                // MOV - Y := (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.y = self.read_small(addr);
                self.update_nz8(self.y);
            }
            0xfc => {
                // INC - Y
                self.y = self.y.wrapping_add(1);
                self.update_nz8(self.y);
            }
            0xfd => {
                // MOV - Y := A
                self.y = self.a;
                self.update_nz8(self.y)
            }
            0xfe => {
                // DBNZ - Y--; JNZ
                self.y = self.y.wrapping_sub(1);
                let rel = self.load();
                self.branch_rel(rel, self.y > 0, &mut cycles)
            }
            _ => todo!("not yet implemented SPC700 instruction 0x{:02x}", op),
        }
        cycles
    }

    pub fn update_nz8(&mut self, val: u8) {
        if val > 0 {
            self.status = (self.status & !(flags::ZERO | flags::SIGN)) | (val & flags::SIGN);
        } else {
            self.status = (self.status & !flags::SIGN) | flags::ZERO
        }
    }

    pub fn update_nz16(&mut self, val: u16) {
        if val > 0 {
            self.status =
                (self.status & !(flags::ZERO | flags::SIGN)) | ((val >> 8) as u8 & flags::SIGN);
        } else {
            self.status = (self.status & !flags::SIGN) | flags::ZERO
        }
    }

    pub fn branch_rel(&mut self, rel: u8, cond: bool, cycles: &mut Cycles) {
        if cond {
            if rel < 0x80 {
                self.pc = self.pc.wrapping_add(rel.into());
            } else {
                self.pc = self.pc.wrapping_sub(0x100 - u16::from(rel));
            }
            *cycles += 2;
        }
    }

    pub fn compare(&mut self, a: u8, b: u8) {
        let res = a as u16 + !b as u16 + 1;
        self.set_status(res > 0xff, flags::CARRY);
        self.update_nz8((res & 0xff) as u8);
    }

    pub fn adc(&mut self, a: u8, b: u8) -> u8 {
        let c = self.status & flags::CARRY;
        let (res, ov1) = a.overflowing_add(b);
        let (res, ov2) = res.overflowing_add(c);
        self.set_status(
            (a & 0x80 == b & 0x80) && (b & 0x80 != res & 0x80),
            flags::OVERFLOW,
        );
        self.set_status(((a & 15) + (b & 15) + c) > 15, flags::HALF_CARRY);
        self.set_status(ov1 || ov2, flags::CARRY);
        self.update_nz8(res);
        res
    }

    pub fn add16(&mut self, a: u16, b: u16) -> u16 {
        let (res, ov) = a.overflowing_add(b);
        self.set_status(
            (a & 0x8000 == b & 0x8000) && (b & 0x8000 != res & 0x8000),
            flags::OVERFLOW,
        );
        self.set_status(((a & 0xfff) + (b & 0xfff)) > 0xffe, flags::HALF_CARRY);
        self.set_status(ov, flags::CARRY);
        self.update_nz16(res);
        res
    }

    pub fn adc16(&mut self, a: u16, b: u16) -> u16 {
        let c = u16::from(self.status & flags::CARRY);
        let (res, ov1) = a.overflowing_add(b);
        let (res, ov2) = res.overflowing_add(c);
        self.set_status(
            (a & 0x8000 == b & 0x8000) && (b & 0x8000 != res & 0x8000),
            flags::OVERFLOW,
        );
        self.set_status(((a & 0xfff) + (b & 0xfff) + c) > 0xfff, flags::HALF_CARRY);
        self.set_status(ov1 || ov2, flags::CARRY);
        self.update_nz16(res);
        res
    }

    /// Tick in main CPU master cycles
    pub fn tick(&mut self, n: u16) {
        self.master_cycles += Cycles::from(n) * self.timing_proportion.1;
    }

    pub fn refresh(&mut self) {
        let cycles = self.master_cycles / self.timing_proportion.0;
        self.master_cycles %= self.timing_proportion.0;
        for _ in 0..cycles {
            self.run_cycle();
        }
    }

    pub fn update_timer(&mut self, i: usize) {
        if self.timer_enable & (1 << i) > 0 {
            self.timers[i] = self.timers[i].wrapping_add(1);
            if self.timers[i] == self.timer_max[i] {
                self.timers[i] = 0;
                self.counters[i].set(self.counters[i].get().wrapping_add(1) & 0xf);
            }
        }
    }

    pub fn run_cycle(&mut self) {
        if self.cycles_ahead == 0 {
            self.cycles_ahead = self.dispatch_instruction().max(1);
        }
        self.cycles_ahead -= 1;
        if self.dispatch_counter & 0xf == 0 {
            if self.dispatch_counter & 0x1f == 0 {
                self.sound_cycle();
                if self.dispatch_counter & 0x7f == 0 {
                    self.update_timer(0);
                    self.update_timer(1);
                }
            }
            self.update_timer(2);
        }
        self.dispatch_counter = self.dispatch_counter.wrapping_add(1);
    }
}
