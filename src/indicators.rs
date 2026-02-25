pub trait Ohlc {
    fn open(&self) -> f32;
    fn high(&self) -> f32;
    fn low(&self) -> f32;
    fn close(&self) -> f32;
}

pub struct HeikinAshi {
    open: f32,
    high: f32,
    low: f32,
    close: f32,
}

impl HeikinAshi {
    /// Creates a Heikin Ashi candle from the current candle and previous candle.
    ///
    /// * `current`: the current raw OHLC candle.
    /// * `prev`: the previous raw OHLC candle.
    pub fn from_ohlc<O>(current: O, prev: O) -> HeikinAshi
    where
        O: Ohlc,
    {
        // midpoint of the previous bar
        let heikin_open = (prev.open() + prev.close()) / 2.0;

        // average of the current bar.
        let heikin_close =
            (current.open() + current.high() + current.low() + current.close()) / 4.0;

        let heikin_high = [current.high(), current.open(), current.close()]
            .iter()
            .fold(0.0, |max, &val| if val > max { val } else { max });

        let heikin_low = [current.high(), current.open(), current.close()]
            .iter()
            .fold(0.0, |min, &val| if val < min { val } else { min });

        HeikinAshi {
            open: heikin_open,
            high: heikin_high,
            low: heikin_low,
            close: heikin_close,
        }
    }
}
