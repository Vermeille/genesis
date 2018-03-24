use std::collections::VecDeque;

#[derive(Copy, Clone)]
enum Reg {
    D(usize),
    A(usize),
    PC,
    CCR,
    InTmp(usize),
    In0,
    In1,
    IOBuffer,
    Immediate(i32),
}

const NB_INTERNAL_REGS: usize = 8;

struct M68K {
    data_r: [u32; 8],

    addr_r: [u32; 8],

    pc: u32,
    ccr: u8,

    intern_r: [u32; NB_INTERNAL_REGS + 3],

    instrs: VecDeque<MicroI>,
}

enum MicroI {
    Zero(Reg),
    Set(Reg, u32),
    Mov(Reg, Reg),
    Add(Reg, Reg), // AddS(Reg, i32),
    Scale(Reg, Size),
    RequestMem(Reg),
}

impl M68K {
    fn exec(&mut self, m: MicroI) -> bool {
        match m {
            MicroI::Zero(r) => self.write_reg(r, 0),
            MicroI::Set(r, x) => self.write_reg(r, x),
            MicroI::Mov(dst, src) => {
                let x = self.read_reg(src);
                self.write_reg(dst, x);
            }
            MicroI::Add(r, x) => {
                let x = self.read_reg(r) + self.read_reg(x);
                self.write_reg(r, x);
            }
            MicroI::Scale(r, s) => {
                let x = self.read_reg(r) << s.shift();
                self.write_reg(r, x);
            }
            MicroI::RequestMem(_addr) => return false,
        }
        true
    }


    fn read_reg(&self, r: Reg) -> u32 {
        match r {
            Reg::D(r) => self.data_r[r],
            Reg::A(r) => self.addr_r[r],
            Reg::PC => self.pc,
            Reg::CCR => self.ccr as u32,
            Reg::InTmp(r) => self.intern_r[r],
            Reg::In0 => self.intern_r[NB_INTERNAL_REGS],
            Reg::In1 => self.intern_r[NB_INTERNAL_REGS + 1],
            Reg::IOBuffer => self.intern_r[NB_INTERNAL_REGS + 2],
            Reg::Immediate(x) => x as u32,
        }
    }

    fn write_reg(&mut self, r: Reg, x: u32) {
        match r {
            Reg::D(r) => self.data_r[r] = x,
            Reg::A(r) => self.addr_r[r] = x,
            Reg::PC => self.pc = x,
            Reg::CCR => self.ccr = x as u8,
            Reg::InTmp(r) => self.intern_r[r] = x,
            Reg::In0 => self.intern_r[NB_INTERNAL_REGS] = x,
            Reg::In1 => self.intern_r[NB_INTERNAL_REGS + 1] = x,
            Reg::IOBuffer => self.intern_r[NB_INTERNAL_REGS + 2] = x,
            Reg::Immediate(_) => unreachable!(),
        }
    }

    fn load_effaddr(&mut self, ea: EffAddr) {
        use Reg::*;
        use MicroI::*;
        match ea {
            EffAddr::DataReg { r } => self.instrs.push_back(Mov(In0, D(r as usize))),
            EffAddr::AddrReg { r } => self.instrs.push_back(Mov(In0, A(r as usize))),
            EffAddr::Addr { r } => {
                self.instrs.push_back(RequestMem(A(r as usize)));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::PostInc { r, s } => {
                let a = A(r as usize);
                self.instrs.push_back(RequestMem(a));
                self.instrs.push_back(Mov(In0, IOBuffer));
                self.instrs.push_back(Add(a, Immediate(s.value())));
            }
            EffAddr::PreDec { r, s } => {
                let a = A(r as usize);
                self.instrs.push_back(Add(a, Immediate(-s.value())));
                self.instrs.push_back(RequestMem(a));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::AddrDisp { r, d } => {
                let a = A(r as usize);
                self.instrs.push_back(Mov(In0, a));
                self.instrs.push_back(Add(In0, Immediate(d as i32)));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIdx { r, idx, d, s } => {
                let a = A(r as usize);
                self.instrs.push_back(Mov(In0, a));
                self.instrs.push_back(Add(In0, Immediate(d)));
                self.instrs.push_back(Mov(In1, idx));
                self.instrs.push_back(Scale(In1, s));
                self.instrs.push_back(Add(In0, In1));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIndPostIdx { r, d, idx, s, od } => {
                let a = A(r as usize);
                self.instrs.push_back(Mov(In0, a));
                self.instrs.push_back(Add(In0, Immediate(d)));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
                self.instrs.push_back(Mov(In1, idx));
                self.instrs.push_back(Scale(In1, s));
                self.instrs.push_back(Add(In0, In1));
                self.instrs.push_back(Add(In0, Immediate(od)));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIndPreIdx { r, d, idx, s, od } => {
                let a = A(r as usize);
                self.instrs.push_back(Mov(In0, a));
                self.instrs.push_back(Add(In0, Immediate(d)));
                self.instrs.push_back(Mov(In1, idx));
                self.instrs.push_back(Scale(In1, s));
                self.instrs.push_back(Add(In0, In1));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
                self.instrs.push_back(Add(In0, Immediate(od)));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAdddr::PCIndDisp { d } => {
                self.instrs.push_back(Mov(In0, PC));
                self.instrs.push_back(Add(In0, d));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
            EffAddr::PCIndIdx { d, idx, s } => {
                self.instrs.push_back(Mov(In0, PC));
                self.instrs.push_back(Add(In0, d));
                self.instrs.push_back(Mov(In1, idx));
                self.instrs.push_back(Scale(In1, s));
                self.instrs.push_back(Add(In0, In1));
                self.instrs.push_back(RequestMem(In0));
                self.instrs.push_back(Mov(In0, IOBuffer));
            }
        }
    }
}

enum EffAddr {
    DataReg { r: u8 }, // 000
    AddrReg { r: u8 }, // 001
    Addr { r: u8 }, // 010
    PostInc { r: u8, s: Size }, // 011
    PreDec { r: u8, s: Size }, // 100
    AddrDisp { r: u8, d: i16 }, // 101
    AddrIdx { r: u8, idx: Reg, d: i32, s: Size }, // 110, 110
    // 110
    AddrIndPostIdx {
        r: u8,
        d: i32,
        idx: Reg,
        s: Size,
        od: i32,
    },
    // 110
    AddrIndPreIdx {
        r: u8,
        d: i32,
        idx: Reg,
        s: Size,
        od: i32,
    },
    // 111
    PCIndDisp { d: i32 },
    PCIndIdx { d: i32, idx: Reg, s: Size },
}

enum AddrMode {
    // Register
    DataReg,
    AddrReg,
    // Register Indirect
    Addr,
    AddrPostInc,
    AddrPreDec,
    AddrDisp,
    // Register with Index
    AddrIdx,
    PCDisp,
    PCIdx,
    AbsShort,
    AbsLong,
    Imm,
}

impl From<u8> for AddrMode {
    fn from(x: u8) -> AddrMode {
        unsafe { std::mem::transmute(x) }
    }
}

#[derive(Clone, Copy)]
enum Size {
    Byte,
    Word,
    Long,
}

impl Size {
    fn shift(self) -> u8 {
        match self {
            Size::Byte => 0,
            Size::Word => 1,
            Size::Long => 2,
        }
    }

    fn value(self) -> i32 {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }
}

fn decode(opcode: u16) -> AddrMode {
    AddrMode::from((opcode & 0b11_1111) as u8)
}

fn main() {
    println!("{}", decode(4u16) as i16);
    println!("{}", decode(5u16) as i16);
}
