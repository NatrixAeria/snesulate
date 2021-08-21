//! The SNES/Famicom device

use crate::{cartridge::Cartridge, cpu::Cpu, spc700::Spc700};
use core::convert::TryInto;

const RAM_SIZE: usize = 0x20000;

/// The 24-bit address type used
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Addr24 {
    pub bank: u8,
    pub addr: u16,
}

impl Addr24 {
    pub const fn new(bank: u8, addr: u16) -> Self {
        Self { bank, addr }
    }

    pub const fn is_lower_half(&self) -> bool {
        self.addr < 0x8000
    }
}

pub trait Access {
    type Output: std::fmt::Debug + Clone + Copy + OpenBus;
    type Buf: AsRef<[u8]> + AsMut<[u8]> + Default;
    fn access_slice(&self, slice: &mut [u8], index: usize) -> Self::Output;
    fn is_read() -> bool;
}

pub struct ReadAccess<P>(core::marker::PhantomData<P>);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WriteAccess<P>(pub P);

impl<P> ReadAccess<P> {
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

pub trait OpenBus {
    fn from_open_bus(value: u8) -> Self;
    fn to_open_bus(self) -> u8;
}

impl OpenBus for () {
    fn from_open_bus(_value: u8) {}
    fn to_open_bus(self) -> u8 {
        unimplemented!()
    }
}

impl OpenBus for u8 {
    fn from_open_bus(value: u8) -> u8 {
        value
    }
    fn to_open_bus(self) -> u8 {
        self
    }
}

impl OpenBus for u16 {
    fn from_open_bus(value: u8) -> u16 {
        ((value as u16) << 8) | (value as u16)
    }
    fn to_open_bus(self) -> u8 {
        (self >> 8) as u8
    }
}

impl OpenBus for Addr24 {
    fn from_open_bus(value: u8) -> Addr24 {
        Addr24::new(value, ((value as u16) << 8) | (value as u16))
    }
    fn to_open_bus(self) -> u8 {
        (self.addr >> 8) as u8
    }
}

impl Access for ReadAccess<u8> {
    type Output = u8;
    type Buf = [u8; 1];
    fn access_slice(&self, slice: &mut [u8], index: usize) -> u8 {
        slice[index]
    }
    fn is_read() -> bool {
        true
    }
}

impl Access for ReadAccess<u16> {
    type Output = u16;
    type Buf = [u8; 2];
    fn access_slice(&self, slice: &mut [u8], index: usize) -> u16 {
        u16::from_le_bytes(slice[index..index + 2].try_into().unwrap())
    }
    fn is_read() -> bool {
        true
    }
}

impl Access for ReadAccess<Addr24> {
    type Output = Addr24;
    type Buf = [u8; 3];
    fn access_slice(&self, slice: &mut [u8], index: usize) -> Addr24 {
        let [bank, addr @ ..]: [u8; 3] = slice[index..index + 3].try_into().unwrap();
        Addr24::new(bank, u16::from_le_bytes(addr))
    }
    fn is_read() -> bool {
        true
    }
}

impl Access for WriteAccess<u8> {
    type Output = ();
    type Buf = [u8; 1];
    fn access_slice(&self, slice: &mut [u8], index: usize) {
        slice[index] = self.0
    }
    fn is_read() -> bool {
        false
    }
}

impl Access for WriteAccess<u16> {
    type Output = ();
    type Buf = [u8; 2];
    fn access_slice(&self, slice: &mut [u8], index: usize) {
        slice[index..index + 2].copy_from_slice(&self.0.to_le_bytes())
    }
    fn is_read() -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub(crate) cpu: Cpu,
    pub(crate) spc: Spc700,
    cartridge: Option<Cartridge>,
    /// <https://wiki.superfamicom.org/open-bus>
    open_bus: u8,
    ram: [u8; RAM_SIZE],
}

impl Device {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            spc: Spc700::new(),
            cartridge: None,
            open_bus: 0,
            ram: [0; RAM_SIZE],
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
        self.cpu = Cpu::new();
        self.reset_program_counter()
    }

    pub fn reset_program_counter(&mut self) {
        self.cpu.regs.pc = Addr24::new(0, self.read::<u16>(Addr24::new(0, 0xfffc)));
    }

    /// Fetch a value from the program counter memory region
    pub fn load<P>(&mut self) -> <ReadAccess<P> as Access>::Output
    where
        ReadAccess<P>: Access,
    {
        let val = self.read::<P>(self.cpu.regs.pc);
        let len = core::mem::size_of::<P>() as u16;
        // yes, an overflow on addr does not carry the bank
        self.cpu.regs.pc.addr = self.cpu.regs.pc.addr.wrapping_add(len);
        val
    }

    /// Read a value from the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn read<P>(&mut self, addr: Addr24) -> <ReadAccess<P> as Access>::Output
    where
        ReadAccess<P>: Access,
    {
        let val = self.access(ReadAccess::<P>::new(), addr);
        self.open_bus = val.to_open_bus();
        val
    }

    /// Write a value to the mapped memory at the specified address.
    /// This method also updates open bus.
    pub fn write<P: OpenBus + Copy>(
        &mut self,
        addr: Addr24,
        value: P,
    ) -> <WriteAccess<P> as Access>::Output
    where
        WriteAccess<P>: Access,
    {
        self.open_bus = value.to_open_bus();
        self.access(WriteAccess::<P>(value), addr)
    }

    /// Access the mapped memory at the specified address
    ///
    /// # Note
    ///
    /// This method does not modify open bus
    pub fn access<A: Access>(&mut self, access: A, addr: Addr24) -> A::Output {
        if (0x7e..=0x7f).contains(&addr.bank) {
            // address bus A + /WRAM
            access.access_slice(
                &mut self.ram,
                ((addr.bank as usize & 1) << 16) | addr.addr as usize,
            )
        } else if addr.bank & 0xc0 == 0 || addr.bank & 0xc0 == 0x80 {
            macro_rules! rw {
                ($read:expr, $write:expr) => {{
                    let mut buf = A::Buf::default();
                    if A::is_read() {
                        for (i, v) in buf.as_mut().iter_mut().enumerate() {
                            *v = $read(addr.addr.wrapping_add(i as u16))
                        }
                        access.access_slice(buf.as_mut(), 0)
                    } else {
                        let out = access.access_slice(buf.as_mut(), 0);
                        for (i, v) in buf.as_ref().iter().enumerate() {
                            $write(addr.addr.wrapping_add(i as u16), *v)
                        }
                        out
                    }
                }};
            }
            match addr.addr {
                0x0000..=0x1fff => {
                    // address bus A + /WRAM
                    access.access_slice(&mut self.ram, addr.addr as usize)
                }
                (0x2000..=0x20ff) | (0x2200..=0x3fff) | (0x4400..=0x7fff) => {
                    // address bus A
                    todo!()
                }
                0x2100..=0x21ff => {
                    // address bus B
                    match addr.addr {
                        0x2140..=0x2143 => access.access_slice(
                            if A::is_read() {
                                &mut self.spc.output
                            } else {
                                &mut self.spc.input
                            },
                            (addr.addr & 0b11) as usize,
                        ),
                        _ => todo!("unimplemented address bus B read at 0x{:04x}", addr.addr),
                    }
                }
                0x4000..=0x43ff => {
                    // internal CPU registers
                    // see https://wiki.superfamicom.org/registers
                    rw!(
                        |addr| self
                            .cpu
                            .read_internal_register(addr)
                            .unwrap_or(self.open_bus),
                        |addr, val| self.cpu.write_internal_register(addr, val)
                    )
                }
                0x8000..=0xffff => {
                    // cartridge read on region $8000-$FFFF
                    self.cartridge
                        .as_mut()
                        .unwrap()
                        .access(access, addr)
                        .unwrap_or_else(|| A::Output::from_open_bus(self.open_bus))
                }
            }
        } else {
            // cartridge read of bank $40-$7D or $C0-$FF
            todo!()
        }
    }
}
