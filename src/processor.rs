use std::fs::File;
use std::io::Read;

use crate::font;
use rand;
use rand::Rng;

pub struct CPU {
    pub opcode: u16,
    pub memory: [u8; 4096],
    pub v: [u8; 16],
    pub i: u8,
    pub pc: usize,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub stack: [usize; 16],
    pub sp: usize,
    pub key: [bool; 16],
    pub gfx: [[u8; 64]; 32],
    pub draw_flag: bool,
    pub keypad: [bool; 16],
    pub keypad_waiting: bool,
    pub keypad_register: usize,
}

impl CPU {
    pub fn new() -> Self {
        let init_ram = CPU::init_ram();
        CPU {
            memory: init_ram,
            v: [0; 16],
            i: 0,
            pc: 0x200,
            delay_timer: 0,
            sound_timer: 0,
            stack: [0; 16],
            sp: 0,
            key: [false; 16],
            gfx: [[0; 64]; 32],
            draw_flag: false,
            keypad: [false; 16],
            keypad_waiting: false,
            keypad_register: 0,
            opcode: 0,
        }
    }

    pub fn load(&mut self, filename: &str) {
        let mut f = File::open(filename).unwrap();
        let mut buffer = [0u8; 3584];

        f.read(&mut buffer).unwrap();

        for (i, &byte) in buffer.iter().enumerate() {
            let addr = 0x200 + i;
            if addr < 4096 {
                self.memory[addr] = byte;
            } else {
                break;
            }
        }
    }

    pub fn get_opcode(&mut self) {
        self.opcode = (self.memory[self.pc] as u16) << 8 | (self.memory[self.pc + 1] as u16);
    }
    pub fn cycle(&mut self, keypad: [bool; 16]) {
        if self.keypad_waiting {
            for i in 0..keypad.len() {
                if keypad[i] {
                    self.keypad_waiting = false;
                    self.v[self.keypad_register] = i as u8;
                    break;
                }
            }
        } else {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }

            if self.sound_timer > 0 {
                println!("Beep!");
                self.sound_timer -= 1;
            }
            self.get_opcode();
            self.run_opcode();
        }
    }

    fn run_opcode(&mut self) {
        println!("{:x} {:x}", self.opcode, self.pc);
        
        match self.opcode & 0xF000 {
            0x0000 => match self.opcode & 0x000F {
                //00E0  Display disp_clear()    Clears the screen.
                0x0000 => {
                    for i in 0..self.gfx.len() {
                        for j in 0..self.gfx[i].len() {
                            self.gfx[i][j] = 0;
                        }
                    }
                    self.draw_flag = true;
                    self.pc += 2;
                }
                //00EE  Flow    return; Returns from a subroutine.
                0x000E => {
                    self.sp -= 1;
                    self.pc += self.stack[self.sp];
                }
                _ => panic!("Unknown opcode {}!", self.opcode),
            },
            0x1000 => {
                //1NNN  Flow    goto NNN;   Jumps to address NNN.
                self.pc = (self.opcode & 0x0FFF) as usize;
            }
            //2NNN  Flow    *(0xNNN)()  Calls subroutine at NNN.
            0x2000 => {
                let nnn: usize = (self.opcode & 0x0FFF) as usize;
                self.stack[self.sp] = self.pc + 2;
                self.sp += 1;
                self.pc = nnn;
            }
            0x3000 => {
                //3XNN  Cond    if(Vx==NN)  Skips the next instruction if VX equals NN.
                // (Usually the next instruction is a jump to skip a code block)
                if self.v[self.op_x()] == self.opcode as u8 & 0x00FF {
                    self.pc += 4;
                } else {
                    self.pc += 2;
                }
            }
            0x6000 => {
                //6XNN  Const   Vx = NN Sets VX to NN.
                self.v[self.op_x()] = self.opcode as u8 & 0x00FF;
                self.pc += 2;
            }
            0x7000 => {
                //7XNN  Const   Vx += NN    Adds NN to VX. (Carry flag is not changed)
                let nn = (self.opcode & 0x00FF) as u16;
                let x = self.op_x();
                let vx = self.v[x] as u16;
                let result = vx + nn;
                self.v[x] = result as u8;
                self.pc += 2;
            }
            0x8000 => {
                let x = self.op_x();
                let y = self.op_y();

                match self.opcode & 0x000F {
                    0x0000 => {
                        //Assign
                        self.v[x] = self.v[y];
                        self.pc += 2;
                    }
                    0x0001 => {
                        //BitOp OR
                        self.v[x] = self.v[x] | self.v[y];
                        self.pc += 2;
                    }
                    0x0002 => {
                        //BitOp AND
                        self.v[x] = self.v[x] & self.v[y];
                        self.pc += 2;
                    }
                    0x0003 => {
                        //BitOp XOR
                        self.v[x] = self.v[x] ^ self.v[y];
                        self.pc += 2;
                    }
                    0x0004 => {
                        // Addition Example 2 on multigesture...

                        self.v[0xF] = if self.v[x >> 4] > (0xFF - self.v[x >> 8]) {
                            1
                        } else {
                            0
                        };

                        self.v[x >> 8] += self.v[y >> 4];
                        self.pc += 2;
                    }
                    0x0005 => {
                        //8XY5  Math    Vx -= Vy    VY is subtracted from VX. VF is set to 0 when there's a borrow,
                        // and 1 when there isn't.
                        self.v[0x0f] = if self.v[x] > self.v[y] { 1 } else { 0 };
                        self.v[x] = self.v[x].wrapping_sub(self.v[y]);
                        self.pc += 2;
                    }
                    0x0006 => {
                        //8XY6[a]   BitOp   Vx>>=1  Stores the least significant bit of VX in VF and then shifts
                        //VX to the right by 1.[b]
                        self.v[0x0f] = self.v[x] & 1;
                        self.v[x] >>= 1;
                        self.pc += 2;
                    }
                    0x0007 => {
                        //8XY7[a]   Math    Vx=Vy-Vx    Sets VX to VY minus VX. VF is set to 0 when there's a borrow,
                        //and 1 when there isn't.
                        self.v[0x0f] = if self.v[y] > self.v[x] { 1 } else { 0 };
                        self.v[x] = self.v[y].wrapping_sub(self.v[x]);
                        self.pc += 2;
                    }
                    0x000E => {
                        //8XYE[a]   BitOp   Vx<<=1  Stores the most significant bit of VX in VF and then shifts VX to the left by 1.[b]
                        self.v[0x0f] = self.v[x] & 0b10000000;
                        self.v[x] <<= 1;
                        self.pc += 2;
                    }
                    _ => panic!("Unknown opcode {}", self.opcode),
                }
            }
            0x9000 => {
                //9XY0  Cond    if(Vx!=Vy)  Skips the next instruction if VX doesn't equal VY.
                //(Usually the next instruction is a jump to skip a code block)
                self.pc += if self.v[self.op_x()] != self.v[self.op_y()] {
                    4
                } else {
                    2
                };
            }
            0xA000 => {
                //ANNN  MEM I = NNN Sets I to the address NNN.
                self.i = (self.opcode & 0x0FFF) as u8;
                self.pc += 2;
            }
            0xB000 => {
                //BNNN  Flow    PC=V0+NNN   Jumps to the address NNN plus V0.
                self.pc = self.v[0] as usize + (self.opcode & 0x0FFF) as usize;
            }
            0xC000 => {
                //CXNN  Rand    Vx=rand()&NN    Sets VX to the result of a bitwise and operation on a random number
                //(Typically: 0 to 255) and NN.
                let x = self.op_x();
                let nn = (self.opcode & 0x00FF) as usize;
                let mut rng = rand::thread_rng();
                let r = rng.gen_range(0, 255);
                self.v[x] = (r & nn) as u8;
                self.pc += 2;
            }
            0xD000 => {
                self.draw_flag = true;
                let x = self.op_x();
                let y = (self.opcode & 0x00F0 >> 4) as usize;
                let n = (self.opcode & 0x000F) as usize;
                for byte in 0..n {
                    let y = (self.v[y] as usize + byte) % 32;
                    for bit in 0..8 {
                        let x = (self.v[x] as usize + byte) % 64;
                        let color = (self.memory[self.i as usize + byte] >> (7 - bit)) & 1;
                        self.v[0x0f] |= color & self.gfx[y][x];
                        self.gfx[y][x] ^= color;
                    }
                }
                self.pc += 2;
            }
            0xF000 => {
                let x = self.op_x();
                match self.opcode & 0x00FF {
                    0x0007 => {
                        self.v[x] = self.delay_timer;
                        self.pc += 2;
                    }
                    0x000A => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    0x0015 => {
                        self.delay_timer = self.v[x];
                        self.pc += 2;
                    }
                    0x0018 => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    0x001E => {
                        self.pc += 2;
                    }
                    0x0029 => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    0x0033 => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    0x0055 => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    0x0065 => {
                        unimplemented!("Not implemented: {:x}", self.opcode);
                        self.pc += 2;
                    }
                    _ => unimplemented!("Unknown self.opcode {}", self.opcode),
                }
            }
            _ => unimplemented!(
                "{} at {}",
                format!("Yikes! self.opcode: {:x}", self.opcode),
                self.pc
            ),
        }
    }

    fn op_x(&self) -> usize {
        (self.opcode & 0x0F00 >> 8) as usize
    }

    fn op_y(&self) -> usize {
        (self.opcode & 0x00F0 >> 4) as usize
    }

    fn init_ram() -> [u8; 4096] {
        let mut ram = [0u8; 4096];

        for i in 0..font::FONT_SET.len() {
            ram[i] = font::FONT_SET[i];
        }

        ram
    }
}
