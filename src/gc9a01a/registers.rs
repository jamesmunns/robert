#![allow(dead_code)]

///< Software reset register
pub const GC9A01A_SWRESET: u8 = 0x01;

///< Read display identification information
// const GC9A01A: u8 = 0x04;
///< Read Display Status
// const GC9A01A: u8 = 0x09;

///< Enter Sleep Mode
pub const GC9A01A_SLPIN: u8 = 0x10;
///< Sleep Out
pub const GC9A01A_SLPOUT: u8 = 0x11;
///< Partial Mode ON
pub const GC9A01A_PTLON: u8 = 0x12;
///< Normal Display Mode ON
pub const GC9A01A_NORON: u8 = 0x13;

///< Display Inversion OFF
pub const GC9A01A_INVOFF: u8 = 0x20;
///< Display Inversion ON
pub const GC9A01A_INVON: u8 = 0x21;
///< Display OFF
pub const GC9A01A_DISPOFF: u8 = 0x28;
///< Display ON
pub const GC9A01A_DISPON: u8 = 0x29;

///< Column Address Set
pub const GC9A01A_CASET: u8 = 0x2A;
///< Page Address Set
pub const GC9A01A_PASET: u8 = 0x2B;
///< Memory Write
pub const GC9A01A_RAMWR: u8 = 0x2C;

///< Partial Area
pub const GC9A01A_PTLAR: u8 = 0x30;
///< Vertical Scrolling Definition
pub const GC9A01A_VSCRDEF: u8 = 0x33;
///< Tearing effect line off
pub const GC9A01A_TEOFF: u8 = 0x34;
///< Tearing effect line on
pub const GC9A01A_TEON: u8 = 0x35;
///< Memory Access Control
pub const GC9A01A_MADCTL: u8 = 0x36;
///< Vertical Scrolling Start Address
pub const GC9A01A_VSCRSADD: u8 = 0x37;
///< COLMOD: Pixel Format Set
pub const GC9A01A_PIXFMT: u8 = 0x3A;

///< RGB Interface Signal Control (B0h)
pub const GC9A01A1_RGBISCTL: u8 = 0xB0;
///< Blanking Porch Control (B5h)
pub const GC9A01A1_BLPCTL: u8 = 0xB5;
///< Display Function Control
pub const GC9A01A1_DFUNCTL: u8 = 0xB6;
///< Tearing Effect Control (BAh)
pub const GC9A01A1_TECTL: u8 = 0xBA;
///< Interface Control (F6h)
pub const GC9A01A1_ITFCTL: u8 = 0xF6;

///< Power control 1
pub const GC9A01A1_PWRCTL1: u8 = 0xC1;
///< Power control 2
pub const GC9A01A1_PWRCTL2: u8 = 0xC3;
///< Power control 3
pub const GC9A01A1_PWRCTL3: u8 = 0xC4;
///< Power control 4
pub const GC9A01A1_PWRCTL4: u8 = 0xC9;
///< Power control 7
pub const GC9A01A1_PWRCTL7: u8 = 0xA7;

///< Positive Gamma Correction
pub const GC9A01A_GMCTRP1: u8 = 0xE0;
///< Negative Gamma Correction
pub const GC9A01A_GMCTRN1: u8 = 0xE1;
///< Frame rate control
pub const GC9A01A_FRAMERATE: u8 = 0xE8;

///< Inter register enable 1
pub const GC9A01A_INREGEN1: u8 = 0xFE;
///< Inter register enable 2
pub const GC9A01A_INREGEN2: u8 = 0xEF;
///< Set gamma 1
pub const GC9A01A_GAMMA1: u8 = 0xF0;
///< Set gamma 2
pub const GC9A01A_GAMMA2: u8 = 0xF1;
///< Set gamma 3
pub const GC9A01A_GAMMA3: u8 = 0xF2;
///< Set gamma 4
pub const GC9A01A_GAMMA4: u8 = 0xF3;

///< Bottom to top
pub const MADCTL_MY: u8 = 0x80;
///< Right to left
pub const MADCTL_MX: u8 = 0x40;
///< Reverse Mode
pub const MADCTL_MV: u8 = 0x20;
///< LCD refresh Bottom to top
pub const MADCTL_ML: u8 = 0x10;
///< Red-Green-Blue pixel order
pub const MADCTL_RGB: u8 = 0x00;
///< Blue-Green-Red pixel order
pub const MADCTL_BGR: u8 = 0x08;
///< LCD refresh right to left
pub const MADCTL_MH: u8 = 0x04;

pub struct InitCmd {
    pub cmd: u8,
    pub data: &'static [u8],
}

pub enum InitOp {
    Cmd(InitCmd),
    Delay(u32),
}

pub const INIT_SEQ: [InitOp; 52] = [
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_INREGEN2,
        data: &[],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xEB,
        data: &[0x14],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_INREGEN1,
        data: &[],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_INREGEN2,
        data: &[],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xEB,
        data: &[0x14],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x84,
        data: &[0x40],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x85,
        data: &[0xFF],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x86,
        data: &[0xFF],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x87,
        data: &[0xFF],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x88,
        data: &[0x0A],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x89,
        data: &[0x21],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8A,
        data: &[0x00],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8B,
        data: &[0x80],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8C,
        data: &[0x01],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8D,
        data: &[0x01],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8E,
        data: &[0xFF],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x8F,
        data: &[0xFF],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xB6,
        data: &[0x00, 0x00],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_MADCTL,
        data: &[MADCTL_MX | MADCTL_BGR],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_PIXFMT,
        data: &[0x05],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x90,
        data: &[0x08, 0x08, 0x08, 0x08],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xBD,
        data: &[0x06],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xBC,
        data: &[0x00],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xFF,
        data: &[0x60, 0x01, 0x04],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A1_PWRCTL2,
        data: &[0x13],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A1_PWRCTL3,
        data: &[0x13],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A1_PWRCTL4,
        data: &[0x22],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xBE,
        data: &[0x11],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_GMCTRN1,
        data: &[0x10, 0x0E],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xDF,
        data: &[0x21, 0x0c, 0x02],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_GAMMA1,
        data: &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_GAMMA2,
        data: &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_GAMMA3,
        data: &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_GAMMA4,
        data: &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xED,
        data: &[0x1B, 0x0B],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xAE,
        data: &[0x77],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0xCD,
        data: &[0x63],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x70,
        data: &[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_FRAMERATE,
        data: &[0x34],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x62,
        data: &[
            0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70,
        ],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x63,
        data: &[
            0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70,
        ],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x64,
        data: &[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x66,
        data: &[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x67,
        data: &[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x74,
        data: &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00],
    }),
    InitOp::Cmd(InitCmd {
        cmd: 0x98,
        data: &[0x3e, 0x07],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_TEON,
        data: &[],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_INVON,
        data: &[],
    }),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_SLPOUT,
        data: &[],
    }),
    InitOp::Delay(120),
    InitOp::Cmd(InitCmd {
        cmd: GC9A01A_DISPON,
        data: &[],
    }),
    InitOp::Delay(120),
];
