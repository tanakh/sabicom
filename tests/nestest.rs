use meru_interface::EmulatorCore;
use sabicom::{context::Cpu, Nes};

#[test]
fn test_nestest() -> anyhow::Result<()> {
    use std::fmt::Write;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct NestestLogger(Mutex<String>);

    impl log::Log for NestestLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.target() == "disasm-nestest" && metadata.level() <= log::Level::Trace
        }

        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                writeln!(self.0.lock().unwrap(), "{}", record.args()).unwrap();
            }
        }

        fn flush(&self) {}
    }

    static LOGGER: NestestLogger = NestestLogger(Mutex::new(String::new()));

    log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Trace))?;

    let path = format!("./nes-test-roms/other/nestest.nes");
    let dat = std::fs::read(std::path::Path::new(&path))?;
    let mut nes = Nes::try_from_file(&dat, None, &Default::default())?;

    // nestest.nes batch mode is start at 0xC000
    nes.ctx.cpu_mut().set_pc(0xC000);

    nes.exec_frame(false);

    let my_output = LOGGER.0.lock().unwrap();

    const REFERENCE_OUTPUT: &str = include_str!("../nes-test-roms/other/nestest.log");

    let my = my_output.lines().collect::<Vec<_>>();
    let ref_ = REFERENCE_OUTPUT.lines().take(8980).collect::<Vec<_>>();

    assert!(my.len() >= ref_.len());

    for i in 0..ref_.len() {
        if ref_[i] != my[i] {
            for j in (0..i).rev().take(5).rev() {
                println!("  {} | {}", my[j], ref_[j]);
            }
            println!("> {} | {}", my[i], ref_[i]);
            for j in (i + 1..).take(5) {
                println!("  {} | {}", my[j], ref_[j]);
            }
        }

        assert_eq!(ref_[i], my[i]);
    }

    Ok(())
}
