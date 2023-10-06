// use crate::GC9A01A;

// use core::ops::Range;

// use embedded_hal::digital::v2::OutputPin;
// use embedded_hal::PwmPin;

// use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};

// use embedded_graphics_core::prelude::*;
// use embedded_graphics_core::{pixelcolor::Rgb565, primitives::Rectangle};

// impl<DI, RST, PWM> DrawTarget for GC9A01A<DI, RST, PWM>
// where
//     DI: WriteOnlyDataCommand,
//     RST: OutputPin,
//     PWM: PwmPin,
// {
//     type Color = Rgb565;
//     type Error = DisplayError;

//     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Pixel<Self::Color>>,
//     {
//         for Pixel(coord, color) in pixels.into_iter() {
//             // TODO: get rid of hardcoded window size
//             if let Ok((x @ 0..=240, y @ 0..=240)) = coord.try_into() {
//                 let x = u8::try_from(x).unwrap();
//                 let y = u8::try_from(y).unwrap();
//                 self.draw_bytes(x, x, y, y, &color.into_storage().to_be_bytes())?;
//             }
//         }

//         Ok(())
//     }

//     fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Self::Color>,
//     {
//         // Clamp area to drawable part of the display target
//         let drawable_area = area.intersection(&self.bounding_box());

//         // Check that there are visible pixels to be drawn
//         if drawable_area.size != Size::zero() {
//             let Range {
//                 start: x_start,
//                 end: x_end,
//             } = drawable_area.columns();
//             let Range {
//                 start: y_start,
//                 end: y_end,
//             } = drawable_area.rows();
//             self.draw_color(
//                 u8::try_from(x_start).unwrap(),
//                 u8::try_from(x_end - 1).unwrap(),
//                 u8::try_from(y_start).unwrap(),
//                 u8::try_from(y_end - 1).unwrap(),
//                 &mut area
//                     .points()
//                     .zip(colors)
//                     .filter(|(pos, _color)| drawable_area.contains(*pos))
//                     .map(|(_, color)| color.into_storage()),
//             )
//         } else {
//             Ok(())
//         }
//     }

//     // Keep the default `fill_solid` implementation that calls `fill_continguous`
//     // fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error>;

//     fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
//         self.set_windows(0, Self::WIDTH - 1, 0, Self::HEIGHT - 1)?;
//         let size = Self::WIDTH as usize * Self::HEIGHT as usize;
//         self.itf.send_data(DataFormat::U8Iter(
//             &mut (0..size).flat_map(|_| color.into_storage().to_be_bytes()),
//         ))
//     }
// }

// impl<DI, RST, PWM> OriginDimensions for GC9A01A<DI, RST, PWM> {
//     fn size(&self) -> Size {
//         // TODO: get rid of hardcoded window size
//         Size::new(240, 240)
//     }
// }
