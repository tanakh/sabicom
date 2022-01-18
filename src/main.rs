mod apu;
mod consts;
mod cpu;
mod mapper;
mod memory;
mod nes;
mod ppu;
mod rom;
mod util;

use anyhow::Result;
use std::path::PathBuf;

#[argopt::cmd(verbose)]
fn main(file: PathBuf) -> Result<()> {
    let dat = std::fs::read(file)?;

    let rom = rom::Rom::from_bytes(&dat)?;
    rom.print_info();

    let mut nes = nes::Nes::new(rom, None);

    loop {
        nes.exec_frame();
    }
}
