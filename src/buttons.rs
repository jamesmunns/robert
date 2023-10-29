use embassy_rp::{gpio::{Input, AnyPin}, pwm};
use embassy_time::{Timer, Duration};

use crate::{buzzer::Pwim, forth::OUTPIPE};


pub struct Buttons {
    pub a: Input<'static, AnyPin>,
    pub b: Input<'static, AnyPin>,
    pub c: Input<'static, AnyPin>,
    pub d: Input<'static, AnyPin>,
    pub e: Input<'static, AnyPin>,
    pub f: Input<'static, AnyPin>,
}

impl Buttons {
    pub fn read_all(&self) -> [bool; 6] {
        [
            self.a.is_low(),
            self.b.is_low(),
            self.c.is_low(),
            self.d.is_low(),
            self.e.is_low(),
            self.f.is_low(),
        ]
    }
}


#[embassy_executor::task]
pub async fn butt(
    btn: Buttons,
    // mut p: Pwim
) {
    let mut state = btn.read_all();
    loop {
        let new_state = btn.read_all();
        if new_state != state {
            // if new_state[0] {
            //     let mut c: pwm::Config = Default::default();
            //     c.top = 0x8000;
            //     c.compare_a = 0x4000;
            //     p.pwm.set_config(&c);
            // } else {
            //     let mut c: pwm::Config = Default::default();
            //     c.top = 0x8000;
            //     c.compare_a = 0;
            //     p.pwm.set_config(&c);
            // }
            OUTPIPE.write_all(b"\r\n").await;
            for b in new_state.iter().copied() {
                OUTPIPE.write_all(if b {
                    b"X"
                } else {
                    b"_"
                }).await;
            }
            OUTPIPE.write_all(b"\r\n").await;
            state = new_state;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
