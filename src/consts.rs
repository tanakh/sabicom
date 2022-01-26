use std::ops::Range;

pub const PPU_CLOCK_PER_LINE: u64 = 341;
pub const PPU_CLOCK_PER_FRAME: u64 = PPU_CLOCK_PER_LINE * LINES_PER_FRAME as u64;
pub const PPU_CLOCK_PER_CPU_CLOCK: u64 = 3;

pub const SCREEN_RANGE: Range<usize> = 0..240;
pub const VBLANK_LINES: usize = 20;
pub const POST_RENDER_LINE: usize = 240;
pub const PRE_RENDER_LINE: usize = 261;
pub const LINES_PER_FRAME: usize = SCREEN_RANGE.end - SCREEN_RANGE.start + VBLANK_LINES + 1 + 1;

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;
