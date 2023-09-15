use core::{
    alloc::Layout, cell::UnsafeCell, future::Future, mem::MaybeUninit, ptr::NonNull, unreachable,
};

use embassy_rp::{rom_data, peripherals::PIO0};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, pipe::Pipe};
use embassy_time::Duration;
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
use smart_leds::{RGB8, colors};

use crate::{Rgb, ws2812::{wheel, Ws2812, gamma_one, brightness_one}};

pub struct RobertCtx {
    pub rgb: Rgb,
    pub ws2812: Ws2812<'static, PIO0, 0, 1>,
    pub enable_gamma: bool,
    pub brightness: u8,
}

impl RobertCtx {
    pub fn new(rgb: Rgb, ws2812: Ws2812<'static, PIO0, 0, 1>) -> Self {
        Self {
            rgb,
            ws2812,
            enable_gamma: true,
            brightness: 64,
        }
    }
}

pub struct RobertAlloc {}

impl DropDict for RobertAlloc {
    unsafe fn drop_dict(_ptr: NonNull<u8>, _layout: Layout) {
        panic!()
    }
}

const RED: i32 = 100;
const GREEN: i32 = 101;
const BLUE: i32 = 102;

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

fn set_gamma(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let val = unsafe { forth.data_stack.try_pop()?.data };
    forth.host_ctxt.enable_gamma = val != 0;
    Ok(())
}

fn set_brightness(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let val = unsafe { forth.data_stack.try_pop()?.data } as u8;
    forth.host_ctxt.brightness = val;
    Ok(())
}

fn red_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    forth.data_stack.push(Word::data(RED))?;
    Ok(())
}

fn green_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    forth.data_stack.push(Word::data(GREEN))?;
    Ok(())
}

fn blue_const(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    forth.data_stack.push(Word::data(BLUE))?;
    Ok(())
}

fn led_on(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let val = forth.data_stack.try_pop()?;
    let val = unsafe { val.data };
    let led = match val {
        RED => &mut forth.host_ctxt.rgb.r,
        GREEN => &mut forth.host_ctxt.rgb.g,
        BLUE => &mut forth.host_ctxt.rgb.b,
        _ => return Err(forth3::Error::BadLiteral),
    };
    led.set_low();
    Ok(())
}

fn led_off(forth: &mut Forth<RobertCtx>) -> Result<(), forth3::Error> {
    let val = forth.data_stack.try_pop()?;
    let val = unsafe { val.data };
    let led = match val {
        RED => &mut forth.host_ctxt.rgb.r,
        GREEN => &mut forth.host_ctxt.rgb.g,
        BLUE => &mut forth.host_ctxt.rgb.b,
        _ => return Err(forth3::Error::BadLiteral),
    };
    led.set_high();
    Ok(())
}

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
        async_builtin!("set_smartled"),
        async_builtin!("smartled_off"),
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
                    embassy_time::Timer::after(Duration::from_millis(secs.try_into().unwrap())).await;
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
                "set_smartled" => {
                    let val = forth.data_stack.try_pop()?;
                    let val = unsafe { val.data };
                    let mut data = i32_to_rgb(val);

                    if forth.host_ctxt.enable_gamma {
                        data = gamma_one(data);
                    }

                    data = brightness_one(data, forth.host_ctxt.brightness);


                    forth.host_ctxt.ws2812.write(&[data]).await;
                    Ok(())
                }
                "smartled_off" => {
                    forth.host_ctxt.ws2812.write(&[colors::BLACK]).await;
                    Ok(())
                }
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
    builtin!("on", led_on),
    builtin!("off", led_off),
    builtin!("red", red_const),
    builtin!("green", green_const),
    builtin!("blue", blue_const),
    builtin!("wheel", conv_wheel),
    builtin!("rgb", vals_to_rgb),
    builtin!("set_gamma", set_gamma),
    builtin!("set_brightness", set_brightness),
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
