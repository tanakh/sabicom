use anyhow::Result;
use std::path::Path;

fn test_rom(path: impl AsRef<Path>) -> Result<()> {
    // let test_name = path.as_ref().file_stem().unwrap().to_str().unwrap();

    let dat = std::fs::read(path.as_ref())?;
    let rom = sabicom::rom::Rom::from_bytes(&dat)?;
    let mut nes = sabicom::nes::Nes::new(rom, None);

    let mut cnt = 0;
    let mut starting = true;

    // status code is at 0x6000
    // - 0x80: running
    // - 0x81: require reset
    // - 0x00..=0x7f: exit code (0 for success)

    let exit_code = loop {
        assert!(cnt < 3000, "too long time");

        nes.exec_frame();

        let stat = nes.mem.borrow().read(0x6000);
        if !starting && stat < 0x80 {
            break stat;
        }

        if !starting && stat == 0x81 {
            todo!("need to reset");
        }

        if starting {
            if stat == 0x80 {
                starting = false;
            }
        } else {
            assert_eq!(stat, 0x80, "invalid stat = ${:02X}", stat);
            cnt += 1;
        }
    };

    let tag = (1..=3)
        .map(|i| nes.mem.borrow().read(0x6000 + i))
        .collect::<Vec<_>>();

    assert_eq!(tag, [0xDE, 0xB0, 0x61]);

    let mut msg = String::new();
    for i in 0x6004.. {
        let c = nes.mem.borrow().read(i);
        if c == 0 {
            break;
        }
        msg.push(c as char);
    }

    assert_eq!(exit_code, 0x00, "Exit code is not 0: {exit_code}, {msg}",);
    assert!(msg.ends_with("\nPassed\n"), "msg: {msg}");

    Ok(())
}

macro_rules! test_rom {
    ($title:ident, $path:literal) => {
        #[test]
        fn $title() -> anyhow::Result<()> {
            test_rom(&format!("{}", $path))
        }
    };
}

macro_rules! test_roms {
    ($($title:ident => $path:literal,)*) => {
        $(
            test_rom!($title, $path);
        )*
    };
}

test_roms! {
    instr_test_v3_01_implied => "nes-test-roms/instr_test-v3/rom_singles/01-implied.nes",
    instr_test_v3_02_immediate => "nes-test-roms/instr_test-v3/rom_singles/02-immediate.nes",
    instr_test_v3_03_zero_page => "nes-test-roms/instr_test-v3/rom_singles/03-zero_page.nes",
    instr_test_v3_04_zp_xy => "nes-test-roms/instr_test-v3/rom_singles/04-zp_xy.nes",
    instr_test_v3_05_absolute => "nes-test-roms/instr_test-v3/rom_singles/05-absolute.nes",
    instr_test_v3_06_abs_xy => "nes-test-roms/instr_test-v3/rom_singles/06-abs_xy.nes",
    instr_test_v3_07_ind_x => "nes-test-roms/instr_test-v3/rom_singles/07-ind_x.nes",
    instr_test_v3_08_ind_y => "nes-test-roms/instr_test-v3/rom_singles/08-ind_y.nes",
    instr_test_v3_09_branches => "nes-test-roms/instr_test-v3/rom_singles/09-branches.nes",
    instr_test_v3_10_stack => "nes-test-roms/instr_test-v3/rom_singles/10-stack.nes",
    instr_test_v3_11_jmp_jsr => "nes-test-roms/instr_test-v3/rom_singles/11-jmp_jsr.nes",
    instr_test_v3_12_rts => "nes-test-roms/instr_test-v3/rom_singles/12-rts.nes",
    instr_test_v3_13_rti => "nes-test-roms/instr_test-v3/rom_singles/13-rti.nes",
    instr_test_v3_14_brk => "nes-test-roms/instr_test-v3/rom_singles/14-brk.nes",
    instr_test_v3_15_special => "nes-test-roms/instr_test-v3/rom_singles/15-special.nes",

    instr_test_v5_01_basics => "instr_test_v5/01-basics.nes",
    instr_test_v5_02_implied => "instr_test_v5/02-implied.nes",
    instr_test_v5_03_immediate => "instr_test_v5/03-immediate.nes",
    instr_test_v5_04_zero_page => "instr_test_v5/04-zero_page.nes",
    instr_test_v5_05_zp_xy => "instr_test_v5/05-zp_xy.nes",
    instr_test_v5_06_absolute => "instr_test_v5/06-absolute.nes",
    instr_test_v5_07_abs_xy => "instr_test_v5/07-abs_xy.nes",
    instr_test_v5_08_ind_x => "instr_test_v5/08-ind_x.nes",
    instr_test_v5_09_ind_y => "instr_test_v5/09-ind_y.nes",
    instr_test_v5_10_branches => "instr_test_v5/10-branches.nes",
    instr_test_v5_11_stack => "instr_test_v5/11-stack.nes",
    instr_test_v5_12_jmp_jsr => "instr_test_v5/12-jmp_jsr.nes",
    instr_test_v5_13_rts => "instr_test_v5/13-rts.nes",
    instr_test_v5_14_rti => "instr_test_v5/14-rti.nes",
    instr_test_v5_15_brk => "instr_test_v5/15-brk.nes",
    instr_test_v5_16_special => "instr_test_v5/16-special.nes",
    // instr_test_v5_all_instrs => "instr_test_v5/all_instrs.nes",
    // instr_test_v5_official_only => "instr_test_v5/official_only.nes",

    // cpu_dummy_reads => "nes-test-roms/cpu_dummy_reads/cpu_dummy_reads.nes",
    cpu_dummy_writes_oam => "nes-test-roms/cpu_dummy_writes/cpu_dummy_writes_oam.nes",
    cpu_dummy_writes_ppumem => "nes-test-roms/cpu_dummy_writes/cpu_dummy_writes_ppumem.nes",

    // "MMC1_A12/mmc1_a12.nes",
    // "PaddleTest3/PaddleTest.nes",
    // "apu_mixer/dmc.nes",
    // "apu_mixer/noise.nes",
    // "apu_mixer/square.nes",
    // "apu_mixer/triangle.nes",
    // "apu_reset/4015_cleared.nes",
    // "apu_reset/4017_timing.nes",
    // "apu_reset/4017_written.nes",
    // "apu_reset/irq_flag_cleared.nes",
    // "apu_reset/len_ctrs_enabled.nes",
    // "apu_reset/works_immediately.nes",
    // "apu_test/apu_test.nes",
    // // "apu_test/rom_singles/1-len_ctr.nes",
    // // "apu_test/rom_singles/2-len_table.nes",
    // // "apu_test/rom_singles/3-irq_flag.nes",
    // // "apu_test/rom_singles/4-jitter.nes",
    // // "apu_test/rom_singles/5-len_timing.nes",
    // // "apu_test/rom_singles/6-irq_flag_timing.nes",
    // // "apu_test/rom_singles/7-dmc_basics.nes",
    // // "apu_test/rom_singles/8-dmc_rates.nes",
    // "blargg_apu_2005.07.30/01.len_ctr.nes",
    // "blargg_apu_2005.07.30/02.len_table.nes",
    // "blargg_apu_2005.07.30/03.irq_flag.nes",
    // "blargg_apu_2005.07.30/04.clock_jitter.nes",
    // "blargg_apu_2005.07.30/05.len_timing_mode0.nes",
    // "blargg_apu_2005.07.30/06.len_timing_mode1.nes",
    // "blargg_apu_2005.07.30/07.irq_flag_timing.nes",
    // "blargg_apu_2005.07.30/08.irq_timing.nes",
    // "blargg_apu_2005.07.30/09.reset_timing.nes",
    // "blargg_apu_2005.07.30/10.len_halt_timing.nes",
    // "blargg_apu_2005.07.30/11.len_reload_timing.nes",
    // "blargg_litewall/blargg_litewall-10c.nes",
    // "blargg_litewall/blargg_litewall-9.nes",
    // "blargg_litewall/litewall2.nes",
    // "blargg_litewall/litewall3.nes",
    // "blargg_litewall/litewall5.nes",
    // "blargg_nes_cpu_test5/cpu.nes",
    // "blargg_nes_cpu_test5/official.nes",
    // "blargg_ppu_tests_2005.09.15b/palette_ram.nes",
    // "blargg_ppu_tests_2005.09.15b/power_up_palette.nes",
    // "blargg_ppu_tests_2005.09.15b/sprite_ram.nes",
    // "blargg_ppu_tests_2005.09.15b/vbl_clear_time.nes",
    // "blargg_ppu_tests_2005.09.15b/vram_access.nes",
    // "branch_timing_tests/1.Branch_Basics.nes",
    // "branch_timing_tests/2.Backward_Branch.nes",
    // "branch_timing_tests/3.Forward_Branch.nes",
    // "cpu_exec_space/test_cpu_exec_space_apu.nes",
    // "cpu_exec_space/test_cpu_exec_space_ppuio.nes",
    // "cpu_interrupts_v2/cpu_interrupts.nes",
    // // "cpu_interrupts_v2/rom_singles/1-cli_latency.nes",
    // // "cpu_interrupts_v2/rom_singles/2-nmi_and_brk.nes",
    // // "cpu_interrupts_v2/rom_singles/3-nmi_and_irq.nes",
    // // "cpu_interrupts_v2/rom_singles/4-irq_and_dma.nes",
    // // "cpu_interrupts_v2/rom_singles/5-branch_delays_irq.nes",
    // "cpu_reset/ram_after_reset.nes",
    // "cpu_reset/registers.nes",
    // "cpu_timing_test6/cpu_timing_test.nes",
    // "dmc_dma_during_read4/dma_2007_read.nes",
    // "dmc_dma_during_read4/dma_2007_write.nes",
    // "dmc_dma_during_read4/dma_4016_read.nes",
    // "dmc_dma_during_read4/double_2007_read.nes",
    // "dmc_dma_during_read4/read_write_2007.nes",
    // "dmc_tests/buffer_retained.nes",
    // "dmc_tests/latency.nes",
    // "dmc_tests/status.nes",
    // "dmc_tests/status_irq.nes",
    // "dpcmletterbox/dpcmletterbox.nes",
    // "dpcmletterbox/obj/nes",
    // "exram/mmc5exram.nes",
    // "full_palette/flowing_palette.nes",
    // "full_palette/full_palette.nes",
    // "full_palette/full_palette_smooth.nes",
    // "instr_misc/instr_misc.nes",
    // // "instr_misc/rom_singles/01-abs_x_wrap.nes",
    // // "instr_misc/rom_singles/02-branch_wrap.nes",
    // // "instr_misc/rom_singles/03-dummy_reads.nes",
    // // "instr_misc/rom_singles/04-dummy_reads_apu.nes",
    // "instr_test-v3/all_instrs.nes",
    // "instr_test-v3/official_only.nes",
    // "instr_timing/instr_timing.nes",
    // // "instr_timing/rom_singles/1-instr_timing.nes",
    // // "instr_timing/rom_singles/2-branch_timing.nes",
    // "m22chrbankingtest/0-127.nes",
    // "mmc3_irq_tests/1.Clocking.nes",
    // "mmc3_irq_tests/2.Details.nes",
    // "mmc3_irq_tests/3.A12_clocking.nes",
    // "mmc3_irq_tests/4.Scanline_timing.nes",
    // "mmc3_irq_tests/5.MMC3_rev_A.nes",
    // "mmc3_irq_tests/6.MMC3_rev_B.nes",
    // "mmc3_test/1-clocking.nes",
    // "mmc3_test/2-details.nes",
    // "mmc3_test/3-A12_clocking.nes",
    // "mmc3_test/4-scanline_timing.nes",
    // "mmc3_test/5-MMC3.nes",
    // "mmc3_test/6-MMC6.nes",
    // // "mmc3_test_2/rom_singles/1-clocking.nes",
    // // "mmc3_test_2/rom_singles/2-details.nes",
    // // "mmc3_test_2/rom_singles/3-A12_clocking.nes",
    // // "mmc3_test_2/rom_singles/4-scanline_timing.nes",
    // // "mmc3_test_2/rom_singles/5-MMC3.nes",
    // // "mmc3_test_2/rom_singles/6-MMC3_alt.nes",
    // "mmc5test/mmc5test.nes",
    // "mmc5test_v2/mmc5test.nes",
    // "nes15-1.0.0/nes15-NTSC.nes",
    // "nes15-1.0.0/nes15-PAL.nes",
    // // "nes_instr_test/rom_singles/01-implied.nes",
    // // "nes_instr_test/rom_singles/02-immediate.nes",
    // // "nes_instr_test/rom_singles/03-zero_page.nes",
    // // "nes_instr_test/rom_singles/04-zp_xy.nes",
    // // "nes_instr_test/rom_singles/05-absolute.nes",
    // // "nes_instr_test/rom_singles/06-abs_xy.nes",
    // // "nes_instr_test/rom_singles/07-ind_x.nes",
    // // "nes_instr_test/rom_singles/08-ind_y.nes",
    // // "nes_instr_test/rom_singles/09-branches.nes",
    // // "nes_instr_test/rom_singles/10-stack.nes",
    // // "nes_instr_test/rom_singles/11-special.nes",
    // "nmi_sync/demo_ntsc.nes",
    // "nmi_sync/demo_pal.nes",
    // "nrom368/fail368.nes",
    // "nrom368/test1.nes",
    // "ny2011/ny2011.nes",
    // "oam_read/oam_read.nes",
    // "oam_stress/oam_stress.nes",
    // "other/2003-test.nes",
    // "other/8bitpeoples_-_deadline_console_invitro.nes",
    // "other/BladeBuster.nes",
    // "other/Duelito.nes",
    // "other/PCM.demo.wgraphics.nes",
    // "other/SimpleParallaxDemo.nes",
    // "other/Streemerz_bundle.nes",
    // "other/apocalypse.nes",
    // "other/blargg_litewall-2.nes",
    // "other/blargg_litewall-9.nes",
    // "other/demo jitter.nes",
    // "other/demo.nes",
    // "other/fceuxd.nes",
    // "other/firefly.nes",
    // "other/high-hopes.nes",
    // "other/logo (E).nes",
    // "other/manhole.nes",
    // "other/max-300.nes",
    // "other/midscanline.nes",
    // "other/minipack.nes",
    // "other/nescafe.nes",
    // "other/nestest.nes",
    // "other/nestopia.nes",
    // "other/new-game.nes",
    // "other/nintendulator.nes",
    // "other/oam3.nes",
    // "other/oc.nes",
    // "other/physics.0.1.nes",
    // "other/pulsar.nes",
    // "other/quantum_disco_brothers_by_wAMMA.nes",
    // "other/rastesam4.nes",
    // "other/read2004.nes",
    // "other/snow.nes",
    // "other/test001.nes",
    // "other/test28.nes",
    // "other/window2_ntsc.nes",
    // "other/window2_pal.nes",
    // "other/window_old_ntsc.nes",
    // "other/window_old_pal.nes",
    // "pal_apu_tests/01.len_ctr.nes",
    // "pal_apu_tests/02.len_table.nes",
    // "pal_apu_tests/03.irq_flag.nes",
    // "pal_apu_tests/04.clock_jitter.nes",
    // "pal_apu_tests/05.len_timing_mode0.nes",
    // "pal_apu_tests/06.len_timing_mode1.nes",
    // "pal_apu_tests/07.irq_flag_timing.nes",
    // "pal_apu_tests/08.irq_timing.nes",
    // "pal_apu_tests/10.len_halt_timing.nes",
    // "pal_apu_tests/11.len_reload_timing.nes",
    // "ppu_open_bus/ppu_open_bus.nes",
    // "ppu_read_buffer/test_ppu_read_buffer.nes",
    // "ppu_vbl_nmi/ppu_vbl_nmi.nes",
    // // "ppu_vbl_nmi/rom_singles/01-vbl_basics.nes",
    // // "ppu_vbl_nmi/rom_singles/02-vbl_set_time.nes",
    // // "ppu_vbl_nmi/rom_singles/03-vbl_clear_time.nes",
    // // "ppu_vbl_nmi/rom_singles/04-nmi_control.nes",
    // // "ppu_vbl_nmi/rom_singles/05-nmi_timing.nes",
    // // "ppu_vbl_nmi/rom_singles/06-suppression.nes",
    // // "ppu_vbl_nmi/rom_singles/07-nmi_on_timing.nes",
    // // "ppu_vbl_nmi/rom_singles/08-nmi_off_timing.nes",
    // // "ppu_vbl_nmi/rom_singles/09-even_odd_frames.nes",
    // // "ppu_vbl_nmi/rom_singles/10-even_odd_timing.nes",
    // "read_joy3/count_errors.nes",
    // "read_joy3/count_errors_fast.nes",
    // "read_joy3/test_buttons.nes",
    // "read_joy3/thorough_test.nes",
    // "scanline-a1/scanline.nes",
    // "scanline/scanline.nes",
    // "scrolltest/scroll.nes",
    // "sprdma_and_dmc_dma/sprdma_and_dmc_dma.nes",
    // "sprdma_and_dmc_dma/sprdma_and_dmc_dma_512.nes",
    // "sprite_hit_tests_2005.10.05/01.basics.nes",
    // "sprite_hit_tests_2005.10.05/02.alignment.nes",
    // "sprite_hit_tests_2005.10.05/03.corners.nes",
    // "sprite_hit_tests_2005.10.05/04.flip.nes",
    // "sprite_hit_tests_2005.10.05/05.left_clip.nes",
    // "sprite_hit_tests_2005.10.05/06.right_edge.nes",
    // "sprite_hit_tests_2005.10.05/07.screen_bottom.nes",
    // "sprite_hit_tests_2005.10.05/08.double_height.nes",
    // "sprite_hit_tests_2005.10.05/09.timing_basics.nes",
    // "sprite_hit_tests_2005.10.05/10.timing_order.nes",
    // "sprite_hit_tests_2005.10.05/11.edge_timing.nes",
    // "sprite_overflow_tests/1.Basics.nes",
    // "sprite_overflow_tests/2.Details.nes",
    // "sprite_overflow_tests/3.Timing.nes",
    // "sprite_overflow_tests/4.Obscure.nes",
    // "sprite_overflow_tests/5.Emulator.nes",
    // "spritecans-2011/obj/nes",
    // "spritecans-2011/spritecans.nes",
    // "stomper/smwstomp.nes",
    // "tutor/tutor.nes",
    // "tvpassfail/tv.nes",
    // "vaus-test/obj/nes",
    // "vaus-test/vaus-test.nes",
    // "vbl_nmi_timing/1.frame_basics.nes",
    // "vbl_nmi_timing/2.vbl_timing.nes",
    // "vbl_nmi_timing/3.even_odd_frames.nes",
    // "vbl_nmi_timing/4.vbl_clear_timing.nes",
    // "vbl_nmi_timing/5.nmi_suppression.nes",
    // "vbl_nmi_timing/6.nmi_disable.nes",
    // "vbl_nmi_timing/7.nmi_timing.nes",
    // "volume_tests/obj/nes",
    // "volume_tests/volumes.nes",
    // "window5/colorwin_ntsc.nes",
    // "window5/colorwin_pal.nes",
}
