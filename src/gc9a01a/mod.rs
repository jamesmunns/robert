//! Library for the GC9A01A display driver
//!
//! From https://gitlab.com/jspngh/gc9a01a-rs
// #![no_std]

pub mod graphics;
pub mod registers;

// use embedded_hal::blocking::delay;
// use embedded_hal::digital::v2::OutputPin;
// use embedded_hal::PwmPin;

// use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};

// use registers::*;

// #[derive(Debug)]
// pub struct GC9A01A<DI, RST, PWM> {
//     /// Display interface.
//     itf: DI,
//     /// Reset pin.
//     rst: RST,
//     /// Backlight pin, pulse-width modulated.
//     bl: PWM,
// }

// impl<DI, RST, PWM> GC9A01A<DI, RST, PWM>
// where
//     DI: WriteOnlyDataCommand,
//     RST: OutputPin,
//     PWM: PwmPin,
// {
//     pub const WIDTH: u8 = 240;
//     pub const HEIGHT: u8 = 240;

//     pub fn new(itf: DI, rst: RST, bl: PWM) -> Self {
//         Self { itf, rst, bl }
//     }

//     pub fn initialize<D>(&mut self, delay: &mut D) -> Result<(), DisplayError>
//     where
//         D: delay::DelayMs<u32>,
//     {
//         for o in INIT_SEQ {
//             match o {
//                 InitOp::Cmd(c) => {
//                     self.itf.send_commands(DataFormat::U8(&[c.cmd]))?;
//                     self.itf.send_data(DataFormat::U8(c.data))?;
//                 }
//                 InitOp::Delay(d) => {
//                     delay.delay_ms(d);
//                 }
//             }
//         }

//         Ok(())
//     }

//     pub fn reset<D>(&mut self, delay: &mut D) -> Result<(), DisplayError>
//     where
//         D: delay::DelayMs<u32>,
//     {
//         self.rst.set_high().map_err(|_| DisplayError::RSError)?;
//         delay.delay_ms(100);
//         self.rst.set_low().map_err(|_| DisplayError::RSError)?;
//         delay.delay_ms(100);
//         self.rst.set_high().map_err(|_| DisplayError::RSError)?;
//         delay.delay_ms(100);
//         Ok(())
//     }

//     pub fn set_backlight(&mut self, duty: PWM::Duty) {
//         self.bl.set_duty(duty);
//     }

//     fn draw_color<C>(
//         &mut self,
//         x_begin: u8,
//         x_end: u8,
//         y_begin: u8,
//         y_end: u8,
//         data: &mut C,
//     ) -> Result<(), DisplayError>
//     where
//         C: Iterator<Item = u16>,
//     {
//         self.set_windows(x_begin, x_end, y_begin, y_end)?;
//         self.itf.send_data(DataFormat::U16BEIter(data))
//     }

//     fn draw_bytes(
//         &mut self,
//         x_begin: u8,
//         x_end: u8,
//         y_begin: u8,
//         y_end: u8,
//         data: &[u8],
//     ) -> Result<(), DisplayError> {
//         self.set_windows(x_begin, x_end, y_begin, y_end)?;
//         self.itf.send_data(DataFormat::U8(data))
//     }

//     // TODO: extend this to u16
//     fn set_windows(&mut self, xs: u8, xe: u8, ys: u8, ye: u8) -> Result<(), DisplayError> {
//         self.itf.send_commands(DataFormat::U8(&[GC9A01A_CASET]))?;
//         self.itf.send_data(DataFormat::U8(&[0x00, xs, 0x00, xe]))?;
//         self.itf.send_commands(DataFormat::U8(&[GC9A01A_PASET]))?;
//         self.itf.send_data(DataFormat::U8(&[0x00, ys, 0x00, ye]))?;
//         self.itf.send_commands(DataFormat::U8(&[GC9A01A_RAMWR]))
//     }
// }
