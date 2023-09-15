use embassy_rp::{bind_interrupts, pio::{InterruptHandler, Instance, StateMachine, Common, PioPin, Config, FifoJoin, ShiftConfig, ShiftDirection}, peripherals::PIO0, dma::{AnyChannel, Channel}, PeripheralRef, Peripheral, into_ref, clocks, gpio::{Output, AnyPin}};
use fixed::types::U24F8;
use smart_leds::RGB8;
use fixed_macro::fixed;

bind_interrupts!(pub struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

pub struct Ws2812<'d, P: Instance, const S: usize, const N: usize> {
    dma: PeripheralRef<'d, AnyChannel>,
    sm: StateMachine<'d, P, S>,
    _pow: Output<'static, AnyPin>,
}

impl<'d, P: Instance, const S: usize, const N: usize> Ws2812<'d, P, S, N> {
    pub fn new(
        pio: &mut Common<'d, P>,
        mut sm: StateMachine<'d, P, S>,
        dma: impl Peripheral<P = impl Channel> + 'd,
        pin: impl PioPin,
        mut pow: Output<'static, AnyPin>,
    ) -> Self {
        into_ref!(dma);

        // Setup sm0

        // prepare the PIO program
        let side_set = pio::SideSet::new(false, 1, false);
        let mut a: pio::Assembler<32> = pio::Assembler::new_with_side_set(side_set);

        const T1: u8 = 2; // start bit
        const T2: u8 = 5; // data bit
        const T3: u8 = 3; // stop bit
        const CYCLES_PER_BIT: u32 = (T1 + T2 + T3) as u32;

        let mut wrap_target = a.label();
        let mut wrap_source = a.label();
        let mut do_zero = a.label();
        a.set_with_side_set(pio::SetDestination::PINDIRS, 1, 0);
        a.bind(&mut wrap_target);
        // Do stop bit
        a.out_with_delay_and_side_set(pio::OutDestination::X, 1, T3 - 1, 0);
        // Do start bit
        a.jmp_with_delay_and_side_set(pio::JmpCondition::XIsZero, &mut do_zero, T1 - 1, 1);
        // Do data bit = 1
        a.jmp_with_delay_and_side_set(pio::JmpCondition::Always, &mut wrap_target, T2 - 1, 1);
        a.bind(&mut do_zero);
        // Do data bit = 0
        a.nop_with_delay_and_side_set(T2 - 1, 0);
        a.bind(&mut wrap_source);

        let prg = a.assemble_with_wrap(wrap_source, wrap_target);
        let mut cfg = Config::default();

        // Pin config
        let out_pin = pio.make_pio_pin(pin);
        cfg.set_out_pins(&[&out_pin]);
        cfg.set_set_pins(&[&out_pin]);

        cfg.use_program(&pio.load_program(&prg), &[&out_pin]);

        // Clock config, measured in kHz to avoid overflows
        // TODO CLOCK_FREQ should come from embassy_rp
        let clock_freq = U24F8::from_num(clocks::clk_sys_freq() / 1000);
        let ws2812_freq = fixed!(800: U24F8);
        let bit_freq = ws2812_freq * CYCLES_PER_BIT;
        cfg.clock_divider = clock_freq / bit_freq;

        // FIFO config
        cfg.fifo_join = FifoJoin::TxOnly;
        cfg.shift_out = ShiftConfig {
            auto_fill: true,
            threshold: 24,
            direction: ShiftDirection::Left,
        };

        sm.set_config(&cfg);
        sm.set_enable(true);

        // todo, for now just turn on the LED immediately
        pow.set_high();

        Self {
            dma: dma.map_into(),
            sm,
            _pow: pow,
        }
    }

    pub async fn write(&mut self, colors: &[RGB8; N]) {
        // Precompute the word bytes from the colors
        let mut words = [0u32; N];
        for i in 0..N {
            let word = (u32::from(colors[i].g) << 24) | (u32::from(colors[i].r) << 16) | (u32::from(colors[i].b) << 8);
            words[i] = word;
        }

        // DMA transfer
        self.sm.tx().dma_push(self.dma.reborrow(), &words).await;
    }
}

/// Input a value 0 to 255 to get a color value
/// The colours are a transition r - g - b - back to r.
pub fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}

pub fn gamma_one(rgb: RGB8) -> RGB8 {
    let (r, g, b): (u8, u8, u8) = rgb.into();
    (GAMMA8[r as usize], GAMMA8[g as usize], GAMMA8[b as usize]).into()
}

pub fn brightness_one(rgb: RGB8, brightness: u8) -> RGB8 {
    if brightness == u8::MAX {
        return rgb;
    }
    RGB8 {
        r: (rgb.r as u16 * (brightness as u16 + 1) / 256) as u8,
        g: (rgb.g as u16 * (brightness as u16 + 1) / 256) as u8,
        b: (rgb.b as u16 * (brightness as u16 + 1) / 256) as u8,
    }
}

// This table remaps linear input values
// (the numbers we’d like to use; e.g. 127 = half brightness)
// to nonlinear gamma-corrected output values
// (numbers producing the desired effect on the LED;
// e.g. 36 = half brightness).
const GAMMA8: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4,
    4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11,
    12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22,
    22, 23, 24, 24, 25, 25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37,
    38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58,
    59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85,
    86, 87, 89, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104, 105, 107, 109, 110, 112, 114,
    115, 117, 119, 120, 122, 124, 126, 127, 129, 131, 133, 135, 137, 138, 140, 142, 144,
    146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 167, 169, 171, 173, 175, 177, 180,
    182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213, 215, 218, 220,
    223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];
