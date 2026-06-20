pub const PIXEL_SIZE: f32 = 8.0;
pub const COLS: usize = 12;
pub const ROWS: usize = 20;

pub type Cell = Option<[f32; 4]>;

const W: Cell = Some([1.000, 1.000, 1.000, 1.0]); // white body
const P: Cell = Some([0.961, 0.745, 0.773, 1.0]); // pink ear / cheek
const K: Cell = Some([0.106, 0.106, 0.173, 1.0]); // dark eye
const H: Cell = Some([1.000, 1.000, 1.000, 1.0]); // eye highlight
const N: Cell = Some([0.941, 0.627, 0.690, 1.0]); // nose
const D: Cell = Some([0.784, 0.471, 0.471, 1.0]); // mouth corner
const E: Cell = None;

#[rustfmt::skip]
pub const GRID: [[Cell; COLS]; ROWS] = [
    [E,E,W,W,E,E,E,E,W,W,E,E], // R0  ear tops
    [E,E,W,W,E,E,E,E,W,W,E,E], // R1
    [E,W,W,W,W,E,E,W,W,W,W,E], // R2  ears widen
    [E,W,P,P,W,E,E,W,P,P,W,E], // R3  inner ear pink
    [E,W,P,P,W,E,E,W,P,P,W,E], // R4  inner ear pink
    [E,W,W,W,W,W,W,W,W,W,W,E], // R5  head starts
    [E,W,W,W,W,W,W,W,W,W,W,E], // R6
    [E,W,K,H,W,W,W,W,K,H,W,E], // R7  eyes
    [E,W,K,K,W,W,W,W,K,K,W,E], // R8  eyes lower
    [E,P,W,W,W,N,N,W,W,W,P,E], // R9  cheeks + nose
    [E,W,W,W,D,W,W,D,W,W,W,E], // R10 mouth corners
    [E,W,W,W,W,W,W,W,W,W,W,E], // R11 chin
    [E,W,W,W,W,W,W,W,W,W,W,E], // R12 body top
    [W,W,W,W,W,W,W,W,W,W,W,W], // R13
    [W,W,W,W,W,W,W,W,W,W,W,W], // R14
    [W,W,W,W,W,W,W,W,W,W,W,W], // R15
    [E,W,W,W,E,E,E,E,W,W,W,E], // R16 legs
    [E,W,W,W,E,E,E,E,W,W,W,E], // R17
    [W,W,W,W,E,E,E,E,W,W,W,W], // R18 feet (wider)
    [W,W,W,W,E,E,E,E,W,W,W,W], // R19
];

pub const EYE_ROWS: [usize; 2] = [7, 8];
pub const EYE_COLS: [usize; 4] = [2, 3, 8, 9];
