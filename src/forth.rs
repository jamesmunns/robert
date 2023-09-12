use core::{cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull, alloc::Layout, future::Future, unreachable};

use embassy_sync::{pipe::Pipe, blocking_mutex::raw::ThreadModeRawMutex};
use embassy_time::Duration;
use forth3::{word::Word, CallContext, Buffers, input::WordStrBuf, output::OutputBuf, dictionary::{Dictionary, DropDict, OwnedDict, AsyncBuiltins, AsyncBuiltinEntry}, AsyncForth, Forth, async_builtin};

pub struct RobertCtx {

}

pub struct RobertAlloc {

}

impl DropDict for RobertAlloc {
    unsafe fn drop_dict(_ptr: NonNull<u8>, _layout: Layout) {
        panic!()
    }
}

pub struct RobertAsync {

}

impl<'forth> AsyncBuiltins<'forth, RobertCtx> for RobertAsync {
    type Future = impl Future<Output = Result<(), forth3::Error>> + 'forth;

    const BUILTINS: &'static [AsyncBuiltinEntry<RobertCtx>] = &[
        async_builtin!("sleep::s"),
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

unsafe impl<T, const N: usize> Sync for MemChunk<T, N> { }

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
    OwnedDict::new::<RobertAlloc>(
        NonNull::new(ptr).unwrap(),
        DICT_BUF_LEN,
    )
}

pub unsafe fn forth(ctx: RobertCtx) -> AsyncForth<RobertCtx, RobertAsync> {
    AsyncForth::new(
        buffers(),
        dict(),
        ctx,
        Forth::FULL_BUILTINS,
        RobertAsync { },
    ).unwrap()
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
            let is_lf = (*chb == b'\n') || (*chb == b'\r');
            let is_control = chb.is_ascii_control();
            match (is_ascii, is_lf, is_control) {
                (true, false, false) => {
                    let bstr = &[*chb];
                    OUTPIPE.write_all(bstr).await;
                    strbuf.push(*chb).unwrap();
                }
                (true, true, _) => {
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
                        },
                        Err(e) => {
                            OUTPIPE.write_all(b"ERROR\r\n").await;
                            let es = match e {
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
                            };
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
                },
            }
        }
    }
}
