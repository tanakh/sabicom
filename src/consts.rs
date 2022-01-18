pub const CLOCK_PER_LINE: u64 = 114;
pub const CLOCK_PER_FRAME: u64 = CLOCK_PER_LINE * TOTAL_LINES as u64;

pub const VISIBLE_LINES: usize = 240;
pub const VBLANK_LINES: usize = 20;
pub const POST_RENDER_LINE: usize = 240;
pub const PRE_RENDER_LINE: usize = 261;
pub const TOTAL_LINES: usize = 262;