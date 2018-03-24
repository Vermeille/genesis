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

enum NextAction {
    Next,
    MemRequest(u32),
}

impl M68K {
    fn exec(&mut self, m: MicroI) -> NextAction {
        use NextAction::*;
        match m {
            MicroI::Zero(r) => {
                self.write_reg(r, 0);
                Next
            }
            MicroI::Set(r, x) => {
                self.write_reg(r, x);
                Next
            }
            MicroI::Mov(dst, src) => {
                let x = self.read_reg(src);
                self.write_reg(dst, x);
                Next
            }
            MicroI::Add(r, x) => {
                let x = self.read_reg(r) + self.read_reg(x);
                self.write_reg(r, x);
                Next
            }
            MicroI::Scale(r, s) => {
                let x = self.read_reg(r) << s.shift();
                self.write_reg(r, x);
                Next
            }
            MicroI::RequestMem(addr) => MemRequest(self.read_reg(addr)),
        }
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

    fn add_instr(&mut self, mi: MicroI) {
        self.instrs.push_back(mi);
    }

    fn load_effaddr(&mut self, ea: EffAddr) {
        use Reg::*;
        use MicroI::*;
        match ea {
            EffAddr::DataReg { r } => self.add_instr(Mov(In0, D(r as usize))),
            EffAddr::AddrReg { r } => self.add_instr(Mov(In0, A(r as usize))),
            EffAddr::Addr { r } => {
                self.add_instr(RequestMem(A(r as usize)));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::PostInc { r, s } => {
                let a = A(r as usize);
                self.add_instr(RequestMem(a));
                self.add_instr(Mov(In0, IOBuffer));
                self.add_instr(Add(a, Immediate(s.value())));
            }
            EffAddr::PreDec { r, s } => {
                let a = A(r as usize);
                self.add_instr(Add(a, Immediate(-s.value())));
                self.add_instr(RequestMem(a));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AddrDisp { r, d } => {
                let a = A(r as usize);
                self.add_instr(Mov(In0, a));
                self.add_instr(Add(In0, Immediate(d as i32)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIdx { r, idx, d, s } => {
                let a = A(r as usize);
                self.add_instr(Mov(In0, a));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIndPostIdx { r, d, idx, s, od } => {
                let a = A(r as usize);
                self.add_instr(Mov(In0, a));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(Add(In0, Immediate(od)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AddrIndPreIdx { r, d, idx, s, od } => {
                let a = A(r as usize);
                self.add_instr(Mov(In0, a));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
                self.add_instr(Add(In0, Immediate(od)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::PCIndDisp { d } => {
                self.add_instr(Mov(In0, PC));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::PCIndIdx { d, idx, s } => {
                self.add_instr(Mov(In0, PC));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::PCIndPostIdx { d, idx, s, od } => {
                self.add_instr(Mov(In0, PC));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(Add(In0, Immediate(od)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::PCIndPreIdx { d, idx, s, od } => {
                self.add_instr(Mov(In0, PC));
                self.add_instr(Add(In0, Immediate(d)));
                self.add_instr(Mov(In1, idx));
                self.add_instr(Scale(In1, s));
                self.add_instr(Add(In0, In1));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
                self.add_instr(Add(In0, Immediate(od)));
                self.add_instr(RequestMem(In0));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AbsShort { addr } => {
                self.add_instr(RequestMem(Immediate(addr as i32)));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::AbsLong { hi, lo } => {
                self.add_instr(RequestMem(
                        Immediate(((hi as u32) << 16 | (lo as u32)) as i32)));
                self.add_instr(Mov(In0, IOBuffer));
            }
            EffAddr::Immediate { addr } => {
                self.add_instr(RequestMem(Immediate(addr as i32)));
                self.add_instr(Mov(In0, IOBuffer));
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
    PCIndPostIdx { d: i32, idx: Reg, s: Size, od: i32 },
    PCIndPreIdx { d: i32, idx: Reg, s: Size, od: i32 },
    AbsShort { addr: i16 },
    AbsLong { hi: u16, lo: u16 },
    Immediate { addr: u32 },
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
    println!("{}", (-1i16 as u32) as i32);
    println!("{}", -1i16 as i32);
}
