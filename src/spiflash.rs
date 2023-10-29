use embassy_rp::{spi::{Spi, Async}, peripherals::SPI0, gpio::{Output, AnyPin, Input}};

pub struct SpiFlash {
    pub spi: Spi<'static, SPI0, Async>,
    pub csn: Output<'static, AnyPin>,
    // For now, keep these floating
    pub io2: Input<'static, AnyPin>,
    pub io3: Input<'static, AnyPin>,
}

impl SpiFlash {
    pub async fn get_id(&mut self) -> [u8; 3] {
        let mut buf = [0x9F, 0x00, 0x00, 0x00];

        self.csn.set_low();
        self.spi.transfer_in_place(&mut buf).await.ok();
        self.csn.set_high();

        [buf[1], buf[2], buf[3]]
    }
}
