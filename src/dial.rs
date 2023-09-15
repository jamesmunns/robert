use core::fmt::Write;

use embassy_rp::{bind_interrupts, adc::{self, Adc, Channel}};
use embassy_time::{Duration, Timer};

use crate::forth::OUTPIPE;

bind_interrupts!(pub struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
});


#[embassy_executor::task]
pub async fn dial(mut adc: Adc<'static, adc::Async>, mut pin: Channel<'static>) {
    let mut strbuf = heapless::String::<32>::new();
    let mut level = adc.read(&mut pin).await.unwrap();
    loop {
        let new_level = adc.read(&mut pin).await.unwrap();
        if level.abs_diff(new_level) >= 16 {
            write!(&mut strbuf, "\r\n{new_level}\r\n").ok();
            OUTPIPE.write_all(strbuf.as_bytes()).await;
            strbuf.clear();
            level = new_level;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
