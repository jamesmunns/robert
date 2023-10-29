use embassy_rp::{pwm, peripherals::{PWM_CH3, PWM_CH5, PWM_CH0, PWM_CH7}};

pub struct Leds {
    pub led_1: pwm::Pwm<'static, PWM_CH3>,
    pub led_2: pwm::Pwm<'static, PWM_CH5>,
    pub led_3: pwm::Pwm<'static, PWM_CH0>,
    pub led_4: pwm::Pwm<'static, PWM_CH7>,
}

impl Leds {
    pub fn set_led(&mut self, idx: u8, val: u16) -> Result<(), ()> {
        let mut config = embassy_rp::pwm::Config::default();
        config.top = u16::MAX;
        // config.compare_b = data;
        // config.enable = true;

        // forth.host_ctxt.lcd.backlight.set_config(&config);
        // * 22 - (EXT) LED1 - PWM3A
        // * 27 - (EXT) LED2 - PWM5B
        // * 00 - (EXT) LED3 - PWM0A
        // * 14 - (EXT) LED4 - PWM7A
        match (idx, val) {
            (0, 0) => {
                // * 22 - (EXT) LED1 - PWM3A
                config.enable = false;
                self.led_1.set_config(&config);
            }
            (1, 0) => {
                // * 27 - (EXT) LED2 - PWM5B
                config.enable = false;
                self.led_2.set_config(&config);
            }
            (2, 0) => {
                // * 00 - (EXT) LED3 - PWM0A
                config.enable = false;
                self.led_3.set_config(&config);
            }
            (3, 0) => {
                // * 14 - (EXT) LED4 - PWM7A
                config.enable = false;
                self.led_4.set_config(&config);
            }
            (0, n) => {
                // * 22 - (EXT) LED1 - PWM3A
                config.enable = true;
                config.compare_a = n;
                self.led_1.set_config(&config);
            }
            (1, n) => {
                // * 27 - (EXT) LED2 - PWM5B
                config.enable = true;
                config.compare_b = n;
                self.led_2.set_config(&config);
            }
            (2, n) => {
                // * 00 - (EXT) LED3 - PWM0A
                config.enable = true;
                config.compare_a = n;
                self.led_3.set_config(&config);
            }
            (3, n) => {
                // * 14 - (EXT) LED4 - PWM7A
                config.enable = true;
                config.compare_a = n;
                self.led_4.set_config(&config);
            }
            _ => return Err(()),
        }

        Ok(())
    }
}
