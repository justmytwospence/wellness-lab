use std::time::Instant;
use wellness_lab_core::circadian::*;
use wellness_lab_core::synth::synth;

fn main() {
    let (d, _) = synth(42, 730, 19_723);
    let t = Instant::now();
    let g = grid(&d, 19_723, 20_452);
    println!("grid {:?}", t.elapsed());
    macro_rules! tm {
        ($n:expr, $e:expr) => {{
            let t = Instant::now();
            let r = $e;
            println!("{} {:?}", $n, t.elapsed());
            r
        }};
    }
    tm!("nonparametric", nonparametric(&g));
    tm!("cosinor", cosinor_weekly(&g));
    tm!("sri", sri(&g));
    tm!("hr_rhythm", hr_rhythm(&g));
    tm!("dlmo", forger_dlmo(&g));
}
