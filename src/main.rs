use std::thread;
use std::time::Duration;

use sdl2;
mod display;
mod font;
mod input;
mod processor;

fn main() {
    let file_name = "Astro Dodge [Revival Studios, 2008].ch8";
    let mut cpu = processor::CPU::new();
    cpu.load(file_name);

    let sleep_duration = Duration::from_millis(2);

    let sdl_context = sdl2::init().unwrap();
    let mut display = display::Display::new(&sdl_context);
    let mut input = input::Input::new(&sdl_context);

    while let Ok(keypad) = input.poll() {
        cpu.cycle(keypad);

        if cpu.draw_flag {
            display.draw(&cpu.gfx);
        }
        thread::sleep(sleep_duration);
    }
}
