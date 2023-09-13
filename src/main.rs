//! This example shows how to use USB (Universal Serial Bus) in the RP2040 chip.
//!
//! This creates a USB serial port that echos.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]

use embassy_time::Duration;

use defmt::{info, panic};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::{bind_interrupts, gpio::{AnyPin, Level, Output}, peripherals::USB, usb::{Driver, Instance, InterruptHandler}};
use embassy_time::Ticker;
use embassy_usb::{class::cdc_acm::{CdcAcmClass, State}, driver::EndpointError, Builder, Config};
use forth::{INPIPE, OUTPIPE, RobertCtx};

use crate::forth::run_forth;
use {defmt_rtt as _, panic_probe as _};
mod forth;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

pub struct Rgb {
    pub r: Output<'static, AnyPin>,
    pub g: Output<'static, AnyPin>,
    pub b: Output<'static, AnyPin>,
}

// #[embassy_executor::task]
// async fn blink(mut rgb: Rgb) {
//     let Rgb { r, g, b } = &mut rgb;
//     let mut leds = [r, g, b];
//     let mut ctr = 0u8;

//     let mut ticker = Ticker::every(Duration::from_millis(250));

//     fn bool2lvl(active: bool) -> Level {
//         if active {
//             Level::Low
//         } else {
//             Level::High
//         }
//     }

//     loop {
//         ticker.next().await;
//         ctr = ctr.wrapping_add(1);
//         let vals = [ctr & 0b100 != 0, ctr & 0b010 != 0, ctr & 0b001 != 0];
//         leds.iter_mut().zip(vals).for_each(|(l, v)| {
//             l.set_level(bool2lvl(v));
//         });
//     }
// }

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello there!");

    let p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-serial example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut device_descriptor = [0; 256];
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut control_buf,
    );

    // Pins:
    //
    // * IO25 - Blue
    // * IO16 - Green
    // * IO17 - Red
    // * IO11 - NEO PWR
    // * IO12 - NEO PIX
    let red = Output::new(AnyPin::from(p.PIN_17), Level::High);
    let blue = Output::new(AnyPin::from(p.PIN_25), Level::High);
    let green = Output::new(AnyPin::from(p.PIN_16), Level::High);
    let rgb = Rgb {
        r: red,
        g: green,
        b: blue,
    };

    // spawner.spawn(blink(rgb)).unwrap();
    spawner.spawn(run_forth(RobertCtx { rgb })).unwrap();

    // Create classes on the builder.
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Do stuff with the class!
    let run_forth_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let _ = usb_forth(&mut class).await;
            info!("Disconnected");
        }
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, run_forth_fut).await;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn usb_forth<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        match embassy_time::with_timeout(Duration::from_millis(10), class.read_packet(&mut buf)).await {
            Ok(Ok(n)) => {
                INPIPE.write_all(&buf[..n]).await;
            },
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {},
        }

        while let Ok(n) = OUTPIPE.try_read(&mut buf) {
            class.write_packet(&buf[..n]).await?;
        }
        // class.write_packet(data).await?;
    }
}
