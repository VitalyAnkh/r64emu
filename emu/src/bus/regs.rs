extern crate byteorder;

use super::bus::{unmapped_area_r, unmapped_area_w, HwIoR, HwIoW, MemIoR, MemIoW};
use super::memint::{ByteOrderCombiner, MemInt};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

trait Register {
    type U: MemInt;
}

trait RegBank {
    fn get_regs<'a, U: MemInt>(&'a self) -> Vec<&(Register<U = U> + 'a)>;
}

bitflags! {
    struct RegFlags: u8 {
        const READACCESS = 0b00000001;
        const WRITEACCESS = 0b00000010;
    }
}

impl Default for RegFlags {
    fn default() -> RegFlags {
        return RegFlags::READACCESS | RegFlags::WRITEACCESS;
    }
}

#[derive(Default)]
pub struct Reg<'a, O, U>
where
    O: ByteOrderCombiner,
    U: MemInt,
{
    raw: RefCell<[u8; 8]>,
    romask: U,
    flags: RegFlags,
    wcb: Option<Box<'a + Fn(U, U)>>,
    rcb: Option<Box<'a + Fn(U) -> U>>,
    phantom: PhantomData<O>,
}

impl<'a, O, U> Reg<'a, O, U>
where
    O: ByteOrderCombiner,
    U: MemInt,
{
    fn new() -> Self {
        Default::default()
    }

    /// Get the current value of the register in memory, bypassing any callback.
    pub fn get(&self) -> U {
        U::endian_read_from::<O>(&self.raw.borrow()[..])
    }

    /// Set the current value of the register, bypassing any read/write mask or callback.
    pub fn set(&self, val: U) {
        U::endian_write_to::<O>(&mut self.raw.borrow_mut()[..], val)
    }

    fn hw_io_r<S>(&self) -> HwIoR
    where
        S: MemInt + Into<U>, // S is a smaller MemInt type than U
    {
        if !self.flags.contains(RegFlags::READACCESS) {
            return unmapped_area_r();
        }

        match self.rcb {
            Some(ref f) => HwIoR::Func(Rc::new(move |addr: u32| {
                let off = (addr as usize) & (U::SIZE - 1);
                let (_, shift) = O::subint_mask::<U, S>(off);
                let val: u64 = f(self.get()).into();
                S::truncate_from(val >> shift).into()
            })),
            None => HwIoR::Mem(&self.raw, (U::SIZE - 1) as u32),
        }
    }

    fn hw_io_w<S>(&mut self) -> HwIoW
    where
        S: MemInt + Into<U>, // S is a smaller MemInt type than U
    {
        if !self.flags.contains(RegFlags::WRITEACCESS) {
            return unmapped_area_w();
        }

        if self.romask == U::zero() && self.wcb.is_none() {
            HwIoW::Mem(&self.raw, (U::SIZE - 1) as u32)
        } else {
            HwIoW::Func(Rc::new(move |addr: u32, val64: u64| {
                let off = (addr as usize) & (U::SIZE - 1);
                let (mut mask, shift) = O::subint_mask::<U, S>(off);
                let mut val = U::truncate_from(val64) << shift;
                let old = self.get();
                mask = !mask | self.romask;
                val = (val & !mask) | (old & mask);
                self.set(val);
                if let Some(ref f) = self.wcb {
                    f(old, val);
                }
            }))
        }
    }

    fn read<S: MemInt + Into<U>>(&self, addr: u32) -> S {
        self.hw_io_r::<S>().at::<O, S>(addr).read()
    }

    fn write<S: MemInt + Into<U>>(&mut self, addr: u32, val: S) {
        self.hw_io_w::<S>().at::<O, S>(addr).write(val);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{be, le};
    use super::RegFlags;
    use std::cell::RefCell;

    #[test]
    fn reg32le_bare() {
        let mut r = le::Reg32::new();
        r.set(0xaaaaaaaa);

        r.write::<u32>(0, 0x12345678);
        assert_eq!(r.read::<u8>(0), 0x78);
        assert_eq!(r.read::<u8>(1), 0x56);
        assert_eq!(r.read::<u16>(2), 0x1234);
        r.write::<u16>(0, 0x6789);
        assert_eq!(r.get(), 0x12346789);
    }

    #[test]
    fn reg32be_bare() {
        let mut r = be::Reg32::new();
        r.set(0xaaaaaaaa);
        r.write::<u32>(0, 0x12345678);
        assert_eq!(r.read::<u8>(0), 0x12);
        assert_eq!(r.read::<u8>(1), 0x34);
        assert_eq!(r.read::<u16>(2), 0x5678);
        r.write::<u16>(0, 0x6789);
        assert_eq!(r.get(), 0x67895678);
    }

    #[test]
    fn reg32le_mask() {
        let mut r = le::Reg32 {
            romask: 0xff00ff00,
            ..Default::default()
        };
        r.set(0xddccbbaa);
        r.write::<u32>(0, 0x12345678);
        assert_eq!(r.get(), 0xdd34bb78);
        assert_eq!(r.read::<u8>(0), 0x78);
        assert_eq!(r.read::<u8>(1), 0xbb);
        assert_eq!(r.read::<u16>(2), 0xdd34);
        r.write::<u16>(0, 0x6789);
        assert_eq!(r.get(), 0xdd34bb89);
    }

    #[test]
    fn reg32be_mask() {
        let mut r = be::Reg32 {
            romask: 0xff00ff00,
            ..Default::default()
        };
        r.set(0xddccbbaa);
        r.write::<u32>(0, 0x12345678);
        assert_eq!(r.get(), 0xdd34bb78);
        assert_eq!(r.read::<u8>(0), 0xdd);
        assert_eq!(r.read::<u8>(1), 0x34);
        assert_eq!(r.read::<u16>(2), 0xbb78);
        r.write::<u16>(0, 0x6789);
        assert_eq!(r.get(), 0xdd89bb78);
    }

    #[test]
    fn reg32le_cb() {
        let mut r = le::Reg32 {
            rcb: Some(box |val| val | 0x1),
            ..Default::default()
        };

        r.set(0x12345678);
        assert_eq!(r.read::<u32>(0), 0x12345679);
        r.write::<u16>(0, 0x6788);
        assert_eq!(r.read::<u32>(0), 0x12346789);
        assert_eq!(r.get(), 0x12346788);
    }

    #[test]
    fn reg32le_rowo() {
        let mut r = le::Reg32 {
            flags: RegFlags::READACCESS,
            ..Default::default()
        };
        r.set(0x12345678);
        assert_eq!(r.read::<u32>(0), 0x12345678);
        r.write::<u32>(0, 0xaabbccdd);
        assert_eq!(r.read::<u32>(0), 0x12345678);

        let mut r = le::Reg32 {
            flags: RegFlags::WRITEACCESS,
            ..Default::default()
        };
        r.set(0x12345678);
        assert_eq!(r.read::<u32>(0), 0xffffffff);
        r.write::<u32>(0, 0xaabbccdd);
        assert_eq!(r.read::<u32>(0), 0xffffffff);
    }
}