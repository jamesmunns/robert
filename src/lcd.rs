
// * 08 - LCD D/C
// * 09 - LCD CSn
// * 10 - LCD CLK
// * 11 - LCD DIN
// * 12 - LCD Reset
// * 25 - LCD Backlight

use embassy_rp::{spi::Spi, gpio::{AnyPin, Output}, peripherals::{SPI1, PWM_CH4}};

use crate::gc9a01a::registers::{GC9A01A_CASET, GC9A01A_PASET, GC9A01A_RAMWR};

pub struct LcdPins {
    pub spi: Spi<'static, SPI1, embassy_rp::spi::Async>,
    pub dc: Output<'static, AnyPin>,
    pub cs: Output<'static, AnyPin>,
    pub rst: Output<'static, AnyPin>,
    pub backlight: embassy_rp::pwm::Pwm<'static, PWM_CH4>,
}

pub struct LcdBuf {
    pub line0: [u8; 7],
    pub line1: [u8; 11],
    pub line2: [u8; 13],
    pub line3: [u8; 14],
    pub line4: [u8; 13],
    pub line5: [u8; 11],
    pub line6: [u8; 7],
}

impl LcdBuf {
    pub fn new() -> Self {
        Self {
            line0: [b' '; 7],
            line1: [b' '; 11],
            line2: [b' '; 13],
            line3: [b' '; 14],
            line4: [b' '; 13],
            line5: [b' '; 11],
            line6: [b' '; 7],
        }
    }

    pub fn get_line(&self, idx: u8) -> Option<&[u8]> {
        match idx {
            0 => Some(&self.line0),
            1 => Some(&self.line1),
            2 => Some(&self.line2),
            3 => Some(&self.line3),
            4 => Some(&self.line4),
            5 => Some(&self.line5),
            6 => Some(&self.line6),
            _ => None,
        }
    }

    pub fn get_line_mut(&mut self, idx: u8) -> Option<&mut [u8]> {
        match idx {
            0 => Some(&mut self.line0),
            1 => Some(&mut self.line1),
            2 => Some(&mut self.line2),
            3 => Some(&mut self.line3),
            4 => Some(&mut self.line4),
            5 => Some(&mut self.line5),
            6 => Some(&mut self.line6),
            _ => None,
        }
    }

    pub fn get_x_range(&self, idx: u8) -> Option<(u8, u8)> {
        match idx {
            0 => Some((64, 176)),
            1 => Some((32, 208)),
            2 => Some((16, 224)),
            3 => Some((8, 232)),
            4 => Some((16, 224)),
            5 => Some((32, 208)),
            6 => Some((64, 176)),
            _ => None,
        }
    }

    pub fn get_y_range(&self, idx: u8) -> Option<(u8, u8)> {
        match idx {
            0 => Some((15, 45)),
            1 => Some((45, 75)),
            2 => Some((75, 105)),
            3 => Some((105, 135)),
            4 => Some((135, 165)),
            5 => Some((165, 195)),
            6 => Some((195, 225)),
            _ => None,
        }
    }
}

// height      width       # chars
// 15-45       64-176      7
// 45-75       32-208      11
// 75-105      16-224      13
// 105-135     8-232       14
// 135-165     16-224      13
// 165-195     32-208      11
// 195-225     64-176      7

impl LcdPins {
    pub async fn command(&mut self, cmd: &[u8]) -> Result<(), embassy_rp::spi::Error> {
        // command
        self.cs.set_low();
        self.dc.set_low();
        self.spi.write(cmd).await?;
        self.cs.set_high();
        Ok(())
    }

    pub async fn data(&mut self, data: &[u8]) -> Result<(), embassy_rp::spi::Error> {
        // data
        self.cs.set_low();
        self.dc.set_high();
        self.spi.write(data).await?;
        self.cs.set_high();
        Ok(())
    }

    pub async fn draw(
        &mut self,
        start_x: u8,
        end_x: u8,
        start_y: u8,
        end_y: u8,
        data: &[u8],
    ) -> Result<(), embassy_rp::spi::Error> {
        self.command(&[GC9A01A_CASET]).await?;
        self.data(&[0x00, start_x, 0x00, end_x - 1]).await?;
        self.command(&[GC9A01A_PASET]).await?;
        self.data(&[0x00, start_y, 0x00, end_y - 1]).await?;
        self.command(&[GC9A01A_RAMWR]).await?;

        self.cs.set_low();
        self.dc.set_high();

        self.spi.write(data).await?;

        self.cs.set_high();

        Ok(())
    }
}
