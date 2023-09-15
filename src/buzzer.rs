use embassy_rp::{pwm::Pwm, peripherals::PWM_CH3};


pub struct Pwim {
    pub pwm: Pwm<'static, PWM_CH3>,
}
