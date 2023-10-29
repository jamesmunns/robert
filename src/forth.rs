use core::{
    alloc::Layout, cell::UnsafeCell, fmt::Write, future::Future, mem::MaybeUninit, ptr::NonNull,
    unreachable,
};

use embassy_rp::rom_data;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, pipe::Pipe};
use embassy_time::{Duration, Timer};
use forth3::{
    async_builtin, builtin,
    dictionary::{
        AsyncBuiltinEntry, AsyncBuiltins, BuiltinEntry, Dictionary, DropDict, EntryHeader,
        EntryKind, OwnedDict,
    },
    fastr::comptime_fastr,
    input::WordStrBuf,
    output::OutputBuf,
    word::Word,
    AsyncForth, Buffers, CallContext, Forth,
};
use smart_leds::{colors, RGB8};

use crate::{
    gc9a01a::registers::{InitOp, GC9A01A_CASET, GC9A01A_PASET, GC9A01A_RAMWR, INIT_SEQ},
    lcd::LcdBuf,
    leds::Leds,
    ws2812::wheel,
    LcdPins, spiflash::SpiFlash,
};

const FONT: Font = Font {
    font: include_bytes!("../ProFont24Point.raw"),
    font_width_chars: 32,
    font_height_chars: 6,
    char_width_px: 16,
    char_height_px: 29,
};

const FONT2: Font = Font {
    font: include_bytes!("../assets/source-code-pro-ascii.aff"),
    font_width_chars: 32,
    font_height_chars: 3,
    char_width_px: 14,
    char_height_px: 31,
};

pub struct RobertCtx {
    pub has_init: bool,
    pub lcd: LcdPins,
    pub lcd_buf: LcdBuf,
    pub leds: Leds,
    pub spif: SpiFlash,
}

impl RobertCtx {
    pub fn new(lcd: LcdPins, leds: Leds, spif: SpiFlash) -> Self {
        Self {
            has_init: false,
            lcd,
            lcd_buf: LcdBuf::new(),
            leds,
            spif,
        }
    }
}

pub struct RobertAlloc {}

impl DropDict for RobertAlloc {
    unsafe fn drop_dict(_ptr: NonNull<u8>, _layout: Layout) {
        panic!()
    }
}

fn rgb_to_i32(rgb: RGB8) -> i32 {
    let (red, green, blue): (u8, u8, u8) = rgb.into();
    let val = [blue, green, red, 0];
    i32::from_le_bytes(val)
}

fn i32_to_rgb(val: i32) -> RGB8 {
    let [blue, green, red, _] = val.to_le_bytes();
    (red, green, blue).into()
}

fn vals_to_rgb(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let rgb: RGB8 = unsafe {
        let blue = forth.data_stack.try_pop()?.data as u8;
        let green = forth.data_stack.try_pop()?.data as u8;
        let red = forth.data_stack.try_pop()?.data as u8;
        (red, green, blue).into()
    };
    let val = rgb_to_i32(rgb);
    forth.data_stack.push(Word::data(val))?;

    Ok(())
}

async fn init(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    init_disp(forth).await?;
    forth.data_stack.push(Word::data(32768))?;
    forth.data_stack.push(Word::data(0))?;
    forth.data_stack.push(Word::data(240))?;
    forth.data_stack.push(Word::data(0))?;
    forth.data_stack.push(Word::data(240))?;
    forth.data_stack.push(Word::data(0))?;
    rect(forth).await?;
    Timer::after(Duration::from_millis(50)).await;
    set_backlight(forth)?;
    Ok(())
}

async fn get_spi_id(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let vals = forth.host_ctxt.spif.get_id().await;
    writeln!(&mut forth.output, "SPI said: {:02X?}\r", &vals)?;

    Ok(())
}


async fn blank_line(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let idx = unsafe { forth.data_stack.try_pop()?.data } as u8;

    let col = linecolor(idx);
    let lcd = &mut forth.host_ctxt.lcd;
    let lcd_buf = &mut forth.host_ctxt.lcd_buf;
    let xrange = lcd_buf.get_x_range(idx);
    let yrange = lcd_buf.get_y_range(idx);

    let (Some(color), Some((xs, xe)), Some((ys, ye))) = (col, xrange, yrange) else {
        return Ok(());
    };

    rect_inner(lcd, xs, xe, ys, ye, color).await.ok();

    Ok(())
}

async fn print_line(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let idx = unsafe { forth.data_stack.try_pop()?.data } as u8;

    let col = linecolor(idx);
    let lcd = &mut forth.host_ctxt.lcd;
    let lcd_buf = &mut forth.host_ctxt.lcd_buf;
    let xrange = lcd_buf.get_x_range(idx);
    let yrange = lcd_buf.get_y_range(idx);
    let txt_buf = lcd_buf.get_line(idx);

    let (Some(color), Some((xs, xe)), Some((ys, ye)), Some(txt_buf)) =
        (col, xrange, yrange, txt_buf)
    else {
        return Ok(());
    };

    // Blank the line
    rect_inner(lcd, xs, xe, ys, ye, color).await.ok();
    let txt = forth.output.as_str();

    let txt = txt.trim();

    if txt.is_empty() {
        return Ok(());
    }

    let len = txt.len().min(txt_buf.len());

    let txt = &txt[..len];

    let mut buf = [0u8; 1024];
    let buf = &mut buf[..FONT2.char_buf_size()];

    // Figure out the starting X position centered
    let txt_width = txt.len() * FONT2.char_width_px;
    let ttl_width = (xe - xs) as usize;
    let delta_w = ttl_width - txt_width;
    let half_delta_w = (delta_w / 2) as u8;
    let xs = xs + half_delta_w;
    let mut x_pos = xs;

    for ch in txt.as_bytes() {
        let idx = ch - b' ';
        let ch_x = idx % 32;
        let ch_y = idx / 32;

        let rgb = {
            let r = ((color >> 8) & 0b11111000) as u8;
            let g = ((color >> 2) & 0b11111100) as u8;
            let b = ((color << 3) & 0b11111000) as u8;
            (r, g, b)
        };

        FONT2
            .font_alpha_to_be_bytes(buf, ch_x.into(), ch_y.into(), colors::WHITE, rgb.into())
            .unwrap();

        lcd.draw(
            x_pos,
            x_pos + FONT2.char_width_px as u8,
            ys,
            ys + FONT2.char_height_px as u8,
            buf,
        )
        .await
        .ok();

        x_pos += FONT2.char_width_px as u8;
    }

    forth.output.clear();

    Ok(())
}

fn linecolor(idx: u8) -> Option<u16> {
    #[allow(clippy::unusual_byte_groupings)]
    match idx {
        0 => Some(0b00000_000000_10000),
        1 => Some(0b00000_000000_01100),
        2 => Some(0b00000_000000_01000),
        3 => Some(0b00000_000000_00110),
        4 => Some(0b00000_000000_01000),
        5 => Some(0b00000_000000_01100),
        6 => Some(0b00000_000000_10000),
        _ => None,
    }
}

fn set_backlight(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let data = unsafe { forth.data_stack.try_pop()?.data };
    let data = data.max(0).min(u16::MAX.into());
    let data = data as u16;

    let mut config = embassy_rp::pwm::Config::default();
    config.top = u16::MAX;
    config.compare_b = data;
    config.enable = true;

    forth.host_ctxt.lcd.backlight.set_config(&config);

    Ok(())
}

// idx amt set_led
fn set_led(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let amt = unsafe { forth.data_stack.try_pop()?.data };
    let amt = amt.max(0).min(u16::MAX.into());
    let amt = amt as u16;

    let idx = unsafe { forth.data_stack.try_pop()?.data } as u8;

    forth
        .host_ctxt
        .leds
        .set_led(idx, amt)
        .map_err(|_| forth3::Error::BadLiteral)
}

async fn init_disp(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    if forth.host_ctxt.has_init {
        return Ok(());
    }
    forth.host_ctxt.has_init = true;

    let lcd = &mut forth.host_ctxt.lcd;

    for c in INIT_SEQ {
        match c {
            InitOp::Cmd(c) => {
                // command
                lcd.command(&[c.cmd]).await.ok();
                Timer::after(Duration::from_micros(10)).await;

                // data
                lcd.data(c.data).await.ok();
                Timer::after(Duration::from_micros(10)).await;
            }
            InitOp::Delay(ms) => {
                Timer::after(Duration::from_millis(ms.into())).await;
            }
        }
    }

    Ok(())
}

async fn rect_inner(
    lcd: &mut LcdPins,
    xs: u8,
    xe: u8,
    ys: u8,
    ye: u8,
    color: u16,
) -> Result<(), ()> {
    if (ye <= ys) || (xe <= xs) {
        return Ok(());
    }

    lcd.command(&[GC9A01A_CASET]).await.ok();
    lcd.data(&[0x00, xs, 0x00, xe - 1]).await.ok();
    lcd.command(&[GC9A01A_PASET]).await.ok();
    lcd.data(&[0x00, ys, 0x00, ye - 1]).await.ok();
    lcd.command(&[GC9A01A_RAMWR]).await.ok();

    let mut buf = [0u8; 4096];
    let color = color.to_be_bytes();

    buf.chunks_exact_mut(2)
        .for_each(|b| b.copy_from_slice(&color));

    let mut remaining = (ye as usize - ys as usize) * (xe as usize - xs as usize) * 2;

    lcd.cs.set_low();
    lcd.dc.set_high();

    while remaining != 0 {
        let take = remaining.min(4096);
        remaining -= take;
        lcd.spi.write(&buf[..take]).await.ok();
    }

    lcd.cs.set_high();

    Ok(())
}

// xs xe ys ye rgb rect
async fn rect(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let rgb = unsafe { forth.data_stack.try_pop()?.data } as u16;
    let ye = unsafe { forth.data_stack.try_pop()?.data } as u8;
    let ys = unsafe { forth.data_stack.try_pop()?.data } as u8;
    let xe = unsafe { forth.data_stack.try_pop()?.data } as u8;
    let xs = unsafe { forth.data_stack.try_pop()?.data } as u8;

    let lcd = &mut forth.host_ctxt.lcd;

    rect_inner(lcd, xs, xe, ys, ye, rgb).await.ok();
    Ok(())
}

struct Font<'a> {
    font: &'a [u8],
    font_width_chars: usize,
    font_height_chars: usize,
    char_width_px: usize,
    char_height_px: usize,
}

impl<'a> Font<'a> {
    fn char_buf_size(&self) -> usize {
        self.char_width_px * self.char_height_px * core::mem::size_of::<u16>()
    }

    fn font_bin_size(&self) -> usize {
        let char_ttl_px = self.char_width_px * self.char_height_px;
        char_ttl_px * self.font_width_chars * self.font_height_chars / 8
    }

    fn font_alpha_to_be_bytes(
        &self,
        buf: &mut [u8],
        char_x: usize,
        char_y: usize,
        set_val: RGB8,
        clr_val: RGB8,
    ) -> Result<(), ()> {
        // Break apart colors
        let (sr, sg, sb): (u8, u8, u8) = set_val.into();
        let (cr, cg, cb): (u8, u8, u8) = clr_val.into();

        // How many pixels wide is the source font table?
        let font_width_px = self.char_width_px * self.font_width_chars;

        // One row at a time of the destination (in 16-bit 565 values)
        let dst_rows = buf.chunks_exact_mut(self.char_width_px * 2);

        // This is an iterator of all font rows
        let rows = self.font.chunks_exact(font_width_px);

        // This is only the font rows that are relevant to the character we want
        let relevant = rows
            .skip(self.char_height_px * char_y)
            .take(self.char_height_px);

        // For each destination row zipped with each font source row...
        dst_rows
            .zip(relevant)
            .flat_map(|(dst_row, src_row)| {
                // ... We only want the columns in each font row that are relevant
                // for this character...
                let cols = src_row
                    .iter()
                    .skip(char_x * self.char_width_px)
                    .take(self.char_width_px);

                // Then for each dest 16-bit value, apply the 1-byte alpha
                dst_row.chunks_exact_mut(2).zip(cols)
            })
            .for_each(|(dst_2b, alpha_1b)| {
                // Here "plus" is how much we take from the "set" color,
                // and "minus" is how much we take from the "clear" color.
                //
                // This is to fade the transparency to the background color
                let plus = *alpha_1b as u16;
                let minus = 255 - plus;

                // Apply alpha to each channel, extending it to 16 bits for each
                // channel (8b x 8b = 16b)
                let r0 = sr as u16 * plus;
                let r1 = cr as u16 * minus;
                let g0 = sg as u16 * plus;
                let g1 = cg as u16 * minus;
                let b0 = sb as u16 * plus;
                let b1 = cb as u16 * minus;

                // Then add the two halves together, and convert to 565 format
                let r = (r0 + r1) & 0b1111100000000000;
                let g = ((g0 + g1) & 0b1111110000000000) >> 5;
                let b = ((b0 + b1) & 0b1111100000000000) >> 11;

                // Then write to the dest buffer
                dst_2b.copy_from_slice(&(r + g + b).to_be_bytes());
            });
        Ok(())
    }

    fn font_bit_to_be_bytes(
        &self,
        buf: &mut [u8],
        char_x: usize,
        char_y: usize,
        set_val: u16,
        clr_val: u16,
    ) -> Result<(), ()> {
        if buf.len() != self.char_buf_size() {
            return Err(());
        }
        if self.font.len() != self.font_bin_size() {
            return Err(());
        }

        let row_width_bytes = (self.char_width_px * self.font_width_chars) / 8;

        buf.chunks_mut(self.char_width_px * 2)
            .zip(self.font.chunks_exact(row_width_bytes))
            .skip(self.char_height_px * char_y)
            .take(self.char_height_px)
            .flat_map(|(by, bi)| {
                by.chunks_exact_mut(16)
                    .zip(bi.iter().skip(2 * char_x).take(2))
            })
            .for_each(|(by16, bi)| {
                let mut val = *bi;
                for by in by16.chunks_exact_mut(2) {
                    let contents = if val & 0x80 != 0 { set_val } else { clr_val };
                    by.copy_from_slice(&contents.to_be_bytes());
                    val <<= 1;
                }
            });

        Ok(())
    }
}

// x y font
async fn font(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let y_pos = unsafe { forth.data_stack.try_pop()?.data } as u8;
    let mut x_pos = unsafe { forth.data_stack.try_pop()?.data } as u8;

    let mut buf = [0u8; 1024];
    let buf = &mut buf[..FONT.char_buf_size()];

    let lcd = &mut forth.host_ctxt.lcd;

    for ch in b"butts" {
        let idx = ch - b' ';
        let ch_x = idx % 32;
        let ch_y = idx / 32;

        FONT.font_bit_to_be_bytes(buf, ch_x.into(), ch_y.into(), 0xFFFF, 0x0000)
            .unwrap();

        lcd.draw(x_pos, x_pos + 16, y_pos, y_pos + 29, buf)
            .await
            .ok();

        x_pos += 16;
    }

    Ok(())
}

async fn font2(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let y_pos = unsafe { forth.data_stack.try_pop()?.data } as u8;
    let mut x_pos = unsafe { forth.data_stack.try_pop()?.data } as u8;

    let mut buf = [0u8; 1024];
    let buf = &mut buf[..FONT2.char_buf_size()];

    let lcd = &mut forth.host_ctxt.lcd;

    for ch in b"butts" {
        let idx = ch - b' ';
        let ch_x = idx % 32;
        let ch_y = idx / 32;

        FONT2
            .font_alpha_to_be_bytes(buf, ch_x.into(), ch_y.into(), colors::WHITE, colors::BLACK)
            .unwrap();

        lcd.draw(
            x_pos,
            x_pos + FONT2.char_width_px as u8,
            y_pos,
            y_pos + FONT2.char_height_px as u8,
            buf,
        )
        .await
        .ok();

        x_pos += FONT2.char_width_px as u8;
    }

    Ok(())
}

// fn set_gamma(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     let val = unsafe { forth.data_stack.try_pop()?.data };
//     forth.host_ctxt.enable_gamma = val != 0;
//     Ok(())
// }

// fn set_brightness(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     let val = unsafe { forth.data_stack.try_pop()?.data } as u8;
//     forth.host_ctxt.brightness = val;
//     Ok(())
// }

// fn red_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     forth.data_stack.push(Word::data(RED))?;
//     Ok(())
// }

// fn green_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     forth.data_stack.push(Word::data(GREEN))?;
//     Ok(())
// }

// fn blue_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     forth.data_stack.push(Word::data(BLUE))?;
//     Ok(())
// }

// fn led_on(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     let val = forth.data_stack.try_pop()?;
//     let val = unsafe { val.data };
//     let led = match val {
//         RED => &mut forth.host_ctxt.rgb.r,
//         GREEN => &mut forth.host_ctxt.rgb.g,
//         BLUE => &mut forth.host_ctxt.rgb.b,
//         _ => return Err(forth3::Error::BadLiteral),
//     };
//     led.set_low();
//     Ok(())
// }

// fn led_off(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
//     let val = forth.data_stack.try_pop()?;
//     let val = unsafe { val.data };
//     let led = match val {
//         RED => &mut forth.host_ctxt.rgb.r,
//         GREEN => &mut forth.host_ctxt.rgb.g,
//         BLUE => &mut forth.host_ctxt.rgb.b,
//         _ => return Err(forth3::Error::BadLiteral),
//     };
//     led.set_high();
//     Ok(())
// }

fn conv_wheel(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let val = forth.data_stack.try_pop()?;
    let val = unsafe { val.data } as u8;
    let val = wheel(val);
    let val = rgb_to_i32(val);
    forth.data_stack.push(Word::data(val))?;
    Ok(())
}

pub struct RobertAsync {}

impl<'forth> AsyncBuiltins<'forth, RobertCtx> for RobertAsync {
    type Future = impl Future<Output = Result<(), forth3::Error>> + 'forth;

    const BUILTINS: &'static [AsyncBuiltinEntry<RobertCtx>] = &[
        async_builtin!("sleep::s"),
        async_builtin!("sleep::ms"),
        async_builtin!("reboot"),
        async_builtin!("flush"),
        // async_builtin!("set_smartled"),
        // async_builtin!("smartled_off"),
        async_builtin!("init"),
        async_builtin!("init_lcd"),
        async_builtin!("rect"),
        async_builtin!("font"),
        async_builtin!("font2"),
        async_builtin!("blank_line"),
        async_builtin!("print_line"),
        async_builtin!("get_spi_id"),
    ];

    fn dispatch_async(
        &self,
        id: &'static forth3::fastr::FaStr,
        forth: &'forth mut forth3::Forth<RobertCtx>,
    ) -> Self::Future {
        async {
            match id.as_str() {
                "sleep::s" => {
                    let secs = unsafe { forth.data_stack.try_pop()?.data };
                    embassy_time::Timer::after(Duration::from_secs(secs.try_into().unwrap())).await;
                    Ok(())
                }
                "sleep::ms" => {
                    let secs = unsafe { forth.data_stack.try_pop()?.data };
                    embassy_time::Timer::after(Duration::from_millis(secs.try_into().unwrap()))
                        .await;
                    Ok(())
                }
                "reboot" => {
                    OUTPIPE.write_all(b"\r\nrebooting in 3s...\r\n").await;
                    embassy_time::Timer::after(Duration::from_secs(3)).await;
                    rom_data::reset_to_usb_boot(0, 0);
                    embassy_time::Timer::after(Duration::from_secs(10)).await;
                    Ok(())
                }
                "flush" => {
                    OUTPIPE.write_all(forth.output.as_str().as_bytes()).await;
                    forth.output.clear();
                    Ok(())
                }
                "init_lcd" => init_disp(forth).await,
                "rect" => rect(forth).await,
                "font" => font(forth).await,
                "font2" => font2(forth).await,
                "blank_line" => blank_line(forth).await,
                "print_line" => print_line(forth).await,
                "init" => init(forth).await,
                "get_spi_id" => get_spi_id(forth).await,
                // "set_smartled" => {
                //     let val = forth.data_stack.try_pop()?;
                //     let val = unsafe { val.data };
                //     let mut data = i32_to_rgb(val);

                //     if forth.host_ctxt.enable_gamma {
                //         data = gamma_one(data);
                //     }

                //     data = brightness_one(data, forth.host_ctxt.brightness);

                //     forth.host_ctxt.ws2812.write(&[data]).await;
                //     Ok(())
                // }
                // "smartled_off" => {
                //     forth.host_ctxt.ws2812.write(&[colors::BLACK]).await;
                //     Ok(())
                // }
                _ => Err(forth3::Error::WordNotInDict),
            }
        }
    }
}

#[repr(C)]
struct DictBuf<const N: usize> {
    d: Dictionary<RobertCtx>,
    buf: [u8; N],
}

unsafe impl<T, const N: usize> Sync for MemChunk<T, N> {}

struct MemChunk<T, const N: usize> {
    inner: UnsafeCell<[MaybeUninit<T>; N]>,
}

impl<T, const N: usize> MemChunk<T, N> {
    const ONE: MaybeUninit<T> = MaybeUninit::uninit();

    const fn uninit() -> Self {
        Self {
            inner: UnsafeCell::new([Self::ONE; N]),
        }
    }

    unsafe fn arr_len(&'static self) -> (*mut T, usize) {
        let ptr: *mut T = self.inner.get().cast();
        (ptr, N)
    }
}

const DICT_BUF_LEN: usize = 4096;

static DSTACK: MemChunk<Word, 256> = MemChunk::uninit();
static RSTACK: MemChunk<Word, 256> = MemChunk::uninit();
static CSTACK: MemChunk<CallContext<RobertCtx>, 64> = MemChunk::uninit();
static INBUF: MemChunk<u8, 128> = MemChunk::uninit();
static OUTBUF: MemChunk<u8, 128> = MemChunk::uninit();
static DICTS: MemChunk<DictBuf<DICT_BUF_LEN>, 1> = MemChunk::uninit();
pub static INPIPE: Pipe<ThreadModeRawMutex, 256> = Pipe::new();
pub static OUTPIPE: Pipe<ThreadModeRawMutex, 256> = Pipe::new();

unsafe fn buffers() -> Buffers<RobertCtx> {
    // TODO: Singleton check
    let ibuf = INBUF.arr_len();
    let obuf = OUTBUF.arr_len();
    Buffers {
        dstack_buf: DSTACK.arr_len(),
        rstack_buf: RSTACK.arr_len(),
        cstack_buf: CSTACK.arr_len(),
        input: WordStrBuf::new(ibuf.0, ibuf.1),
        output: OutputBuf::new(obuf.0, obuf.1),
    }
}

unsafe fn dict() -> OwnedDict<RobertCtx> {
    let (ptr, len): (*mut DictBuf<DICT_BUF_LEN>, usize) = DICTS.arr_len();
    assert_eq!(len, 1);
    let ptr: *mut MaybeUninit<Dictionary<RobertCtx>> = ptr.cast();
    OwnedDict::new::<RobertAlloc>(NonNull::new(ptr).unwrap(), DICT_BUF_LEN)
}

pub unsafe fn forth(ctx: RobertCtx) -> AsyncForth<RobertCtx, RobertAsync> {
    AsyncForth::new(buffers(), dict(), ctx, ROBERT_BUILTINS, RobertAsync {}).unwrap()
}

#[embassy_executor::task]
pub async fn run_forth(ctx: RobertCtx) {
    let mut forth = unsafe { forth(ctx) };
    let mut ibuf = [0u8; 64];
    let mut strbuf = heapless::Vec::<u8, 128>::new();
    OUTPIPE.write_all(b"RP2040 Forth Says Hello!\r\n").await;
    loop {
        let ilen = INPIPE.read(&mut ibuf).await;
        for chb in &ibuf[..ilen] {
            let is_ascii = chb.is_ascii();
            let is_control = chb.is_ascii_control();
            match (is_ascii, is_control, *chb) {
                (true, false, _) => {
                    let bstr = &[*chb];
                    OUTPIPE.write_all(bstr).await;
                    strbuf.push(*chb).unwrap();
                }
                (true, true, b'\r') | (true, true, b'\n') => {
                    let s = core::str::from_utf8(strbuf.as_slice()).unwrap();
                    forth.input_mut().fill(s).unwrap();
                    strbuf.clear();
                    OUTPIPE.write_all(b"\r\n").await;
                    match forth.process_line().await {
                        Ok(()) => {
                            let om = forth.output_mut();
                            let out = om.as_str().as_bytes();
                            OUTPIPE.write_all(out).await;
                            OUTPIPE.write_all(b"\r").await;
                        }
                        Err(e) => {
                            OUTPIPE.write_all(b"ERROR\r\n").await;
                            let es = err2str(&e);
                            OUTPIPE.write_all(es.as_bytes()).await;
                            OUTPIPE.write_all(b"\r\n").await;
                        }
                    }
                    // TODO(ajm): I need a "clear" function for the input. This wont properly
                    // clear string literals either.
                    let inp = forth.input_mut();
                    while inp.cur_word().is_some() {
                        inp.advance();
                    }
                    forth.output_mut().clear();
                }
                (true, true, 0x7f) | (true, true, 0x08) => {
                    if strbuf.pop().is_some() {
                        OUTPIPE.write_all(&[0x08, b' ', 0x08]).await;
                    }
                }
                _ => {
                    let mut val = *chb;
                    OUTPIPE.write_all(b"?").await;
                    for _ in 0..2 {
                        let s = match (val & 0xF0) >> 4 {
                            0 => "0",
                            1 => "1",
                            2 => "2",
                            3 => "3",
                            4 => "4",
                            5 => "5",
                            6 => "6",
                            7 => "7",
                            8 => "8",
                            9 => "9",
                            10 => "A",
                            11 => "B",
                            12 => "C",
                            13 => "D",
                            14 => "E",
                            15 => "F",
                            _ => unreachable!(),
                        };
                        OUTPIPE.write_all(s.as_bytes()).await;
                        val <<= 4;
                    }
                    OUTPIPE.write_all(b"?").await;
                }
            }
        }
    }
}

pub const ROBERT_BUILTINS: &[BuiltinEntry<RobertCtx>] = &[
    // Custom operations
    // builtin!("on", led_on),
    // builtin!("off", led_off),
    // builtin!("red", red_const),
    // builtin!("green", green_const),
    // builtin!("blue", blue_const),
    builtin!("wheel", conv_wheel),
    builtin!("rgb", vals_to_rgb),
    // builtin!("set_gamma", set_gamma),
    // builtin!("set_brightness", set_brightness),
    builtin!("set_backlight", set_backlight),
    builtin!("set_led", set_led),
    //
    // Math operations
    //
    builtin!("+", Forth::add),
    builtin!("-", Forth::minus),
    builtin!("/", Forth::div),
    builtin!("mod", Forth::modu),
    builtin!("/mod", Forth::div_mod),
    builtin!("*", Forth::mul),
    builtin!("abs", Forth::abs),
    builtin!("negate", Forth::negate),
    builtin!("min", Forth::min),
    builtin!("max", Forth::max),
    //
    // Floating Math operations
    //
    // builtin_if_feature!("floats", "f+", Forth::float_add),
    // builtin_if_feature!("floats", "f-", Forth::float_minus),
    // builtin_if_feature!("floats", "f/", Forth::float_div),
    // builtin_if_feature!("floats", "fmod", Forth::float_modu),
    // builtin_if_feature!("floats", "f/mod", Forth::float_div_mod),
    // builtin_if_feature!("floats", "f*", Forth::float_mul),
    // builtin_if_feature!("floats", "fabs", Forth::float_abs),
    // builtin_if_feature!("floats", "fnegate", Forth::float_negate),
    // builtin_if_feature!("floats", "fmin", Forth::float_min),
    // builtin_if_feature!("floats", "fmax", Forth::float_max),
    //
    // Double intermediate math operations
    //
    builtin!("*/", Forth::star_slash),
    builtin!("*/mod", Forth::star_slash_mod),
    //
    // Logic operations
    //
    builtin!("not", Forth::invert),
    // NOTE! This is `bitand`, not logical `and`! e.g. `&` not `&&`.
    builtin!("and", Forth::and),
    builtin!("=", Forth::equal),
    builtin!(">", Forth::greater),
    builtin!("<", Forth::less),
    builtin!("0=", Forth::zero_equal),
    builtin!("0>", Forth::zero_greater),
    builtin!("0<", Forth::zero_less),
    //
    // Stack operations
    //
    builtin!("swap", Forth::swap),
    builtin!("dup", Forth::dup),
    builtin!("over", Forth::over),
    builtin!("rot", Forth::rot),
    builtin!("drop", Forth::ds_drop),
    //
    // Double operations
    //
    builtin!("2swap", Forth::swap_2),
    builtin!("2dup", Forth::dup_2),
    builtin!("2over", Forth::over_2),
    builtin!("2drop", Forth::ds_drop_2),
    //
    // String/Output operations
    //
    builtin!("emit", Forth::emit),
    builtin!("cr", Forth::cr),
    builtin!("space", Forth::space),
    builtin!("spaces", Forth::spaces),
    builtin!(".", Forth::pop_print),
    builtin!("u.", Forth::unsigned_pop_print),
    // builtin_if_feature!("floats", "f.", Forth::float_pop_print),
    //
    // Define/forget
    //
    builtin!(":", Forth::colon),
    builtin!("forget", Forth::forget),
    //
    // Stack/Retstack operations
    //
    builtin!("d>r", Forth::data_to_return_stack),
    // NOTE: REQUIRED for `do/loop`
    builtin!("2d>2r", Forth::data2_to_return2_stack),
    builtin!("r>d", Forth::return_to_data_stack),
    //
    // Loop operations
    //
    builtin!("i", Forth::loop_i),
    builtin!("i'", Forth::loop_itick),
    builtin!("j", Forth::loop_j),
    builtin!("leave", Forth::loop_leave),
    //
    // Memory operations
    //
    builtin!("@", Forth::var_load),
    builtin!("!", Forth::var_store),
    builtin!("b@", Forth::byte_var_load),
    builtin!("b!", Forth::byte_var_store),
    builtin!("w+", Forth::word_add),
    builtin!("'", Forth::addr_of),
    builtin!("execute", Forth::execute),
    //
    // Constants
    //
    builtin!("0", Forth::zero_const),
    builtin!("1", Forth::one_const),
    //
    // Introspection
    //
    builtin!("builtins", Forth::list_builtins),
    builtin!("dict", Forth::list_dict),
    builtin!(".s", Forth::list_stack),
    builtin!("free", Forth::dict_free),
    //
    // Other
    //
    // NOTE: REQUIRED for `."`
    builtin!("(write-str)", Forth::write_str_lit),
    // NOTE: REQUIRED for `do/loop`
    builtin!("(jmp-doloop)", Forth::jump_doloop),
    // NOTE: REQUIRED for `if/then` and `if/else/then`
    builtin!("(jump-zero)", Forth::jump_if_zero),
    // NOTE: REQUIRED for `if/else/then`
    builtin!("(jmp)", Forth::jump),
    // NOTE: REQUIRED for `:` (if you want literals)
    builtin!("(literal)", Forth::literal),
    // NOTE: REQUIRED for `constant`
    builtin!("(constant)", Forth::constant),
    // NOTE: REQUIRED for `variable` or `array`
    builtin!("(variable)", Forth::variable),
];

fn err2str(e: &forth3::Error) -> &'static str {
    match e {
        forth3::Error::Stack(s) => match s {
            forth3::stack::StackError::StackEmpty => "StackEmpty",
            forth3::stack::StackError::StackFull => "StackFull",
            forth3::stack::StackError::OverwriteInvalid => "OverwriteInvalid",
        },
        forth3::Error::Bump(_) => "Bump",
        forth3::Error::Output(_) => "Output",
        forth3::Error::CFANotInDict(_) => "CFANotInDict",
        forth3::Error::WordNotInDict => "WordNotInDict",
        forth3::Error::ColonCompileMissingName => "ColonCompileMissingName",
        forth3::Error::ColonCompileMissingSemicolon => "ColonCompileMissingSemicolon",
        forth3::Error::LookupFailed => "LookupFailed",
        forth3::Error::WordToUsizeInvalid(_) => "WordToUsizeInvalid",
        forth3::Error::UsizeToWordInvalid(_) => "UsizeToWordInvalid",
        forth3::Error::ElseBeforeIf => "ElseBeforeIf",
        forth3::Error::ThenBeforeIf => "ThenBeforeIf",
        forth3::Error::IfWithoutThen => "IfWithoutThen",
        forth3::Error::DuplicateElse => "DuplicateElse",
        forth3::Error::IfElseWithoutThen => "IfElseWithoutThen",
        forth3::Error::CallStackCorrupted => "CallStackCorrupted",
        forth3::Error::InterpretingCompileOnlyWord => "InterpretingCompileOnlyWord",
        forth3::Error::BadCfaOffset => "BadCfaOffset",
        forth3::Error::LoopBeforeDo => "LoopBeforeDo",
        forth3::Error::DoWithoutLoop => "DoWithoutLoop",
        forth3::Error::BadCfaLen => "BadCfaLen",
        forth3::Error::BuiltinHasNoNextValue => "BuiltinHasNoNextValue",
        forth3::Error::UntaggedCFAPtr => "UntaggedCFAPtr",
        forth3::Error::LoopCountIsNegative => "LoopCountIsNegative",
        forth3::Error::LQuoteMissingRQuote => "LQuoteMissingRQuote",
        forth3::Error::LiteralStringTooLong => "LiteralStringTooLong",
        forth3::Error::NullPointerInCFA => "NullPointerInCFA",
        forth3::Error::BadStrLiteral(_) => "BadStrLiteral",
        forth3::Error::ForgetWithoutWordName => "ForgetWithoutWordName",
        forth3::Error::ForgetNotInDict => "ForgetNotInDict",
        forth3::Error::CantForgetBuiltins => "CantForgetBuiltins",
        forth3::Error::InternalError => "InternalError",
        forth3::Error::BadLiteral => "BadLiteral",
        forth3::Error::BadWordOffset => "BadWordOffset",
        forth3::Error::BadArrayLength => "BadArrayLength",
        forth3::Error::DivideByZero => "DivideByZero",
        forth3::Error::AddrOfMissingName => "AddrOfMissingName",
        forth3::Error::AddrOfNotAWord => "AddrOfNotAWord",
        forth3::Error::PendingCallAgain => "PendingCallAgain",
    }
}
