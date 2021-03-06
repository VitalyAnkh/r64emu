extern crate emu;

use emu::dbg::Operand;
use mips64::{DecodedInsn, REG_NAMES};

// Decoder constants
const VREG_NAMES: [&'static str; 32] = [
    "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13", "v14",
    "v15", "v16", "v17", "v18", "v19", "v20", "v21", "v22", "v23", "v24", "v25", "v26", "v27",
    "v28", "v29", "v30", "v31",
];

const VMEM_FMT: &'static str = "{}[e{}],{}({})";
const VREG2_FMT: &'static str = "{},{}[e{}]";
const VREG3_FMT: &'static str = "{},{},{}[e{}]";

pub(crate) fn decode(opcode: u32, _pc: u64) -> DecodedInsn {
    use self::Operand::*;

    let op = opcode >> 26;
    let func = opcode & 0x3F;
    let e = ((opcode >> 21) & 0xF) as u8;
    let rsx = (opcode >> 11) & 0x1f;
    let rdx = (opcode >> 6) & 0x1f;
    // let grs = REG_NAMES[((opcode >> 11) & 0x1f) as usize].into();
    let grt = REG_NAMES[((opcode >> 16) & 0x1f) as usize].into();
    // let grd = REG_NAMES[((opcode >> 6) & 0x1f) as usize].into();
    let vrs = VREG_NAMES[((opcode >> 11) & 0x1f) as usize].into();
    let vrt = VREG_NAMES[((opcode >> 16) & 0x1f) as usize].into();
    let vrd = VREG_NAMES[((opcode >> 6) & 0x1f) as usize].into();

    let vreg3insn_new = |name| {
        if vrd == vrs {
            DecodedInsn::new3(name, IOReg(vrd), IReg(vrt), Imm8(e)).with_fmt(VREG2_FMT)
        } else {
            DecodedInsn::new4(name, OReg(vrd), IReg(vrs), IReg(vrt), Imm8(e)).with_fmt(VREG3_FMT)
        }
    };

    match op {
        0x12 => {
            if opcode & (1 << 25) != 0 {
                match func {
                    0x00 => vreg3insn_new("vmulf"),
                    0x01 => vreg3insn_new("vmulu"),
                    0x04 => vreg3insn_new("vmudl"),
                    0x05 => vreg3insn_new("vmudm"),
                    0x06 => vreg3insn_new("vmudn"),
                    0x07 => vreg3insn_new("vmudh"),
                    0x08 => vreg3insn_new("vmacf"),
                    0x09 => vreg3insn_new("vmacu"),
                    0x0C => vreg3insn_new("vmadl"),
                    0x0D => vreg3insn_new("vmadm"),
                    0x0E => vreg3insn_new("vmadn"),
                    0x0F => vreg3insn_new("vmadh"),
                    0x10 => vreg3insn_new("vadd"),
                    0x11 => vreg3insn_new("vsub"),
                    0x13 => vreg3insn_new("vabs"),
                    0x14 => vreg3insn_new("vaddc"),
                    0x15 => vreg3insn_new("vsubc"),
                    0x17 => vreg3insn_new("vsubb"),
                    0x19 => vreg3insn_new("vsucb"),
                    0x1D => match e {
                        8 => DecodedInsn::new2("vsar", OReg(vrd), IReg("accum_lo")),
                        9 => DecodedInsn::new2("vsar", OReg(vrd), IReg("accum_md")),
                        10 => DecodedInsn::new2("vsar", OReg(vrd), IReg("accum_hi")),
                        _ => DecodedInsn::new2("vsar?", OReg(vrd), Imm8(e)),
                    },
                    0x20 => vreg3insn_new("vlt"),
                    0x21 => vreg3insn_new("veq"),
                    0x22 => vreg3insn_new("vne"),
                    0x23 => vreg3insn_new("vge"),
                    0x24 => vreg3insn_new("vcl"),
                    0x25 => vreg3insn_new("vch"),
                    0x26 => vreg3insn_new("vcr"),
                    0x28 => vreg3insn_new("vand"),
                    0x29 => vreg3insn_new("vnand"),
                    0x2A => vreg3insn_new("vor"),
                    0x2B => vreg3insn_new("vnor"),
                    0x2C => vreg3insn_new("vxor"),
                    0x2D => vreg3insn_new("vnxor"),
                    _ => DecodedInsn::new1("cop2", Imm32(func)),
                }
            } else {
                match e {
                    0x2 => match rsx {
                        0 => DecodedInsn::new2("cfc2", OReg(grt), IReg("vco")),
                        1 => DecodedInsn::new2("cfc2", OReg(grt), IReg("vcc")),
                        2 => DecodedInsn::new2("cfc2", OReg(grt), IReg("vce")),
                        _ => DecodedInsn::new2("cfc2?", OReg(grt), Imm8(rsx as u8)),
                    },
                    0x4 => DecodedInsn::new3("mtc2", IReg(grt), OReg(vrs), Imm8(rdx as u8 >> 1))
                        .with_fmt(VREG2_FMT),
                    0x6 => match rsx {
                        0 => DecodedInsn::new2("ctc2", OReg(grt), IReg("vco")),
                        1 => DecodedInsn::new2("ctc2", OReg(grt), IReg("vcc")),
                        2 => DecodedInsn::new2("ctc2", OReg(grt), IReg("vce")),
                        _ => DecodedInsn::new2("ctc2?", OReg(grt), Imm8(rsx as u8)),
                    },
                    _ => DecodedInsn::new1("cop2su?", Imm8(e)),
                }
            }
        }
        0x32 => {
            let oploadstore = (opcode >> 11) & 0x1F;
            let e = ((opcode >> 7) & 0xF) as u8;
            let base = REG_NAMES[((opcode >> 21) & 0x1F) as usize];
            let off = (opcode & 0x7F) as u16;

            let vloadinsn_new = |name, off| {
                DecodedInsn::new4(name, OReg(vrt), Imm8(e), Imm16(off), IReg(base))
                    .with_fmt(VMEM_FMT)
            };
            match oploadstore {
                0x00 => vloadinsn_new("lbv", off * 1),
                0x01 => vloadinsn_new("lsv", off * 2),
                0x02 => vloadinsn_new("llv", off * 4),
                0x03 => vloadinsn_new("ldv", off * 8),
                0x04 => vloadinsn_new("lqv", off * 16),
                0x05 => vloadinsn_new("lrv", off * 16),
                0x0B => vloadinsn_new("ltv", off * 16),
                _ => DecodedInsn::new1("lwc2", Imm32(oploadstore)),
            }
        }
        0x3A => {
            let oploadstore = (opcode >> 11) & 0x1F;
            let e = ((opcode >> 7) & 0xF) as u8;
            let base = REG_NAMES[((opcode >> 21) & 0x1F) as usize];
            let off = (opcode & 0x7F) as u16;

            let vstoreinsn_new = |name, off| {
                DecodedInsn::new4(name, IReg(vrt), Imm8(e), Imm16(off), IReg(base))
                    .with_fmt(VMEM_FMT)
            };
            match oploadstore {
                0x00 => vstoreinsn_new("sbv", off * 1),
                0x01 => vstoreinsn_new("ssv", off * 2),
                0x02 => vstoreinsn_new("slv", off * 4),
                0x03 => vstoreinsn_new("sdv", off * 8),
                0x04 => vstoreinsn_new("sqv", off * 16),
                0x05 => vstoreinsn_new("srv", off * 16),
                0x0A => vstoreinsn_new("swv", off * 16),
                0x0B => vstoreinsn_new("stv", off * 16),
                _ => DecodedInsn::new1("swc2", Imm32(oploadstore)),
            }
        }
        _ => DecodedInsn::new0("unkcop2?"),
    }
}
