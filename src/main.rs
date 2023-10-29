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
use embassy_rp::{
    adc::{self, Adc},
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, Pull},
    peripherals::{USB, PWM_CH3, PWM_CH5, PWM_CH0, PWM_CH7},
    pwm,
    spi::{Config as SpiConfig, Spi},
    usb::{Driver, Instance, InterruptHandler},
};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    Builder, Config,
};
use forth::{RobertCtx, INPIPE, OUTPIPE};



use crate::{forth::run_forth, lcd::LcdPins, leds::Leds, spiflash::SpiFlash};
use {defmt_rtt as _, panic_probe as _};
mod buttons;
mod buzzer;
mod dial;
mod forth;
mod gc9a01a;
mod ws2812;
mod lcd;
mod fmath;
mod leds;
mod spiflash;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});


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

    // PINS:
    //
    // * 00 - (EXT) LED3 - PWM0A
    // * 01 - (EXT) SW1
    // * 02 - N/A
    // * 03 - N/A
    // * 04 - (EXT) SPI0 IO3
    // * 05 - (EXT) SPI0 IO2
    // * 06 - (BRD) IMU/I2C SDA
    // * 07 - (BRD) IMU/I2C SCL
    // * 08 - (BRD) LCD D/C
    // * 09 - (BRD) LCD CSn
    // * 10 - (BRD) LCD CLK
    // * 11 - (BRD) LCD DIN
    // * 12 - (BRD) LCD Reset
    // * 13 - (EXT) SW2
    // * 14 - (EXT) LED4 - PWM7A
    // * 15 - (EXT) SW3
    // * 16 - (EXT) SPI0 RX
    // * 17 - (EXT) SPI0 CSn
    // * 18 - (EXT) SPI0 SCK
    // * 19 - (EXT) SPI0 TX
    // * 20 - (EXT) /!\ SPI0 PWR /!\
    // * 21 - (EXT) SW6
    // * 22 - (EXT) LED1 - PWM3A
    // * 23 - (BRD) IMU INT1
    // * 24 - (BRD) IMU INT2
    // * 25 - (BRD) LCD Backlight
    // * 26 - (EXT) SW4
    // * 27 - (EXT) LED2 - PWM5B
    // * 28 - (EXT) SW5
    // * 29 - (BRD) Battery ADC
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = 62_500_000;

    let spi = Spi::new_txonly(p.SPI1, p.PIN_10, p.PIN_11, p.DMA_CH0, spi_cfg);

    let bl = pwm::Pwm::new_output_b(p.PWM_CH4, p.PIN_25, pwm::Config::default());

    let lcd = LcdPins {
        spi,
        dc: Output::new(AnyPin::from(p.PIN_8), Level::Low),
        cs: Output::new(AnyPin::from(p.PIN_9), Level::High),
        rst: Output::new(AnyPin::from(p.PIN_12), Level::High),
        backlight: bl,
    };

    // * 22 - (EXT) LED1 - PWM3A
    // * 27 - (EXT) LED2 - PWM5B
    // * 00 - (EXT) LED3 - PWM0A
    // * 14 - (EXT) LED4 - PWM7A
    let led_1: pwm::Pwm<'static, PWM_CH3> = pwm::Pwm::new_output_a(p.PWM_CH3, p.PIN_22, pwm::Config::default());
    let led_2: pwm::Pwm<'static, PWM_CH5> = pwm::Pwm::new_output_b(p.PWM_CH5, p.PIN_27, pwm::Config::default());
    let led_3: pwm::Pwm<'static, PWM_CH0> = pwm::Pwm::new_output_a(p.PWM_CH0, p.PIN_0, pwm::Config::default());
    let led_4: pwm::Pwm<'static, PWM_CH7> = pwm::Pwm::new_output_a(p.PWM_CH7, p.PIN_14, pwm::Config::default());
    let leds = Leds { led_1, led_2, led_3, led_4 };

    let adc = Adc::new(p.ADC, dial::Irqs, adc::Config::default());
    let adc_pin = adc::Channel::new_pin(p.PIN_29, Pull::None);

    // * 04 - (EXT) SPI0 IO3
    // * 05 - (EXT) SPI0 IO2
    // * 16 - (EXT) SPI0 RX
    // * 17 - (EXT) SPI0 CSn
    // * 18 - (EXT) SPI0 SCK
    // * 19 - (EXT) SPI0 TX
    // * 20 - (EXT) SPI0 PWR
    let mut spif_cfg = SpiConfig::default();
    spif_cfg.frequency = 16_000_000;
    let spi = Spi::new(
        p.SPI0,    // Periph
        p.PIN_18,  // SCK
        p.PIN_19,  // MOSI
        p.PIN_16,  // MISO
        p.DMA_CH1, // TX DMA
        p.DMA_CH2, // RX DMA
        spif_cfg,
    );

    let spif = SpiFlash {
        spi,
        csn: Output::new(AnyPin::from(p.PIN_17), Level::High),
        io2: Input::new(AnyPin::from(p.PIN_5), Pull::None),
        io3: Input::new(AnyPin::from(p.PIN_4), Pull::None),
    };

    // spawner.spawn(blink(rgb)).unwrap();
    // * 01 - (EXT) SW1
    // * 13 - (EXT) SW2
    // * 15 - (EXT) SW3
    // * 26 - (EXT) SW4
    // * 28 - (EXT) SW5
    // * 21 - (EXT) SW6
    spawner.spawn(run_forth(RobertCtx::new(lcd, leds, spif))).unwrap();
    spawner
        .spawn(buttons::butt(
            buttons::Buttons {
                a: Input::new(AnyPin::from(p.PIN_1), Pull::Up),
                b: Input::new(AnyPin::from(p.PIN_13), Pull::Up),
                c: Input::new(AnyPin::from(p.PIN_15), Pull::Up),
                d: Input::new(AnyPin::from(p.PIN_26), Pull::Up),
                e: Input::new(AnyPin::from(p.PIN_28), Pull::Up),
                f: Input::new(AnyPin::from(p.PIN_21), Pull::Up),
            },
            // pw,
        ))
        .unwrap();
    spawner.spawn(dial::dial(adc, adc_pin)).unwrap();

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
        match embassy_time::with_timeout(Duration::from_millis(10), class.read_packet(&mut buf))
            .await
        {
            Ok(Ok(n)) => {
                INPIPE.write_all(&buf[..n]).await;
            }
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {}
        }

        while let Ok(n) = OUTPIPE.try_read(&mut buf) {
            class.write_packet(&buf[..n]).await?;
        }
        // class.write_packet(data).await?;
    }
}
