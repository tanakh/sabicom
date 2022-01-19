mod apu;
mod consts;
mod cpu;
mod mapper;
mod memory;
mod nes;
mod ppu;
mod rom;
mod util;

use anyhow::{anyhow, Result};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    EventPump,
};
use std::{collections::VecDeque, path::PathBuf, time::Duration};

const SCALING: u32 = 2;
const FPS: f64 = 60.0;

#[argopt::cmd(verbose)]
fn main(file: PathBuf) -> Result<()> {
    let dat = std::fs::read(file)?;

    let rom = rom::Rom::from_bytes(&dat)?;
    rom.print_info();

    let mut nes = nes::Nes::new(rom, None);

    let (width, height) = {
        let buf = nes.get_frame_buf();
        (buf.width, buf.height)
    };

    let screen_width = width as u32 * SCALING;
    let screen_height = height as u32 * SCALING;

    let sdl_context = sdl2::init().map_err(|e| anyhow!("{e}"))?;
    let video_subsystem = sdl_context.video().map_err(|e| anyhow!("{e}"))?;

    let window = video_subsystem
        .window("runes", screen_width, screen_height)
        .build()?;

    let mut canvas = window.into_canvas().build()?;
    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let ttf_context = sdl2::ttf::init().map_err(|e| anyhow!("{e}"))?;
    let font = ttf_context
        .load_font("./assets/fonts/Inconsolata-Regular.ttf", 32)
        .map_err(|e| anyhow!("{e}"))?;

    let mut surface = sdl2::surface::Surface::new(width as _, height as _, PixelFormatEnum::RGB24)
        .map_err(|e| anyhow!("{e}"))?;

    let audio_subsystem = sdl_context.audio().map_err(|e| anyhow!("{e}"))?;
    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(2),
        samples: Some(2048),
    };
    let device: AudioQueue<i16> = audio_subsystem
        .open_queue(None, &desired_spec)
        .map_err(|e| anyhow!("{e}"))?;
    device.resume();

    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow!("{e}"))?;

    let mut timer = Timer::new();

    while process_events(&mut event_pump) {
        nes.exec_frame();

        surface.with_lock_mut(|r| {
            let buf = nes.get_frame_buf();

            for y in 0..height {
                for x in 0..width {
                    let ix = y * width + x;
                    let p = buf.get(x, y);
                    r[ix * 3 + 0] = p.r;
                    r[ix * 3 + 1] = p.g;
                    r[ix * 3 + 2] = p.b;
                }
            }
        });

        let texture = surface.as_texture(&texture_creator)?;
        canvas
            .copy(&texture, None, None)
            .map_err(|e| anyhow!("{e}"))?;

        {
            let fps_tex = font
                .render(&format!("{:.2}", timer.fps()))
                .blended(Color::WHITE)?
                .as_texture(&texture_creator)?;

            let (w, h) = {
                let q = fps_tex.query();
                (q.width, q.height)
            };

            canvas
                .copy(
                    &fps_tex,
                    None,
                    Rect::new(screen_width as i32 - w as i32, 0, w, h),
                )
                .map_err(|e| anyhow!("{e}"))?;
        }

        canvas.present();

        timer.wait_for_frame(FPS);
    }

    Ok(())
}

fn process_events(event_pump: &mut EventPump) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => return false,
            _ => {}
        }
    }
    true
}

use std::time::SystemTime;

struct Timer {
    hist: VecDeque<SystemTime>,
    prev: SystemTime,
}

impl Timer {
    fn new() -> Self {
        Self {
            hist: VecDeque::new(),
            prev: SystemTime::now(),
        }
    }

    fn wait_for_frame(&mut self, fps: f64) {
        let span = 1.0 / fps;

        let elapsed = self.prev.elapsed().unwrap().as_secs_f64();

        if elapsed < span {
            let wait = span - elapsed;
            std::thread::sleep(Duration::from_secs_f64(wait));
        }

        self.prev = SystemTime::now();

        self.hist.push_back(self.prev);
        while self.hist.len() > 60 {
            self.hist.pop_front();
        }
    }

    fn fps(&self) -> f64 {
        if self.hist.len() < 60 {
            return 0.0;
        }

        let span = self.hist.len() - 1;
        let dur = self
            .hist
            .back()
            .unwrap()
            .duration_since(*self.hist.front().unwrap())
            .unwrap()
            .as_secs_f64();

        span as f64 / dur
    }
}
