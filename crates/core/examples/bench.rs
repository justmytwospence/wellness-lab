//! Timing of the main jobs on synthetic data: `cargo run --release -p wellness-lab-core --example bench`.
use std::time::Instant;
use wellness_lab_core::lab::Lab;
use wellness_lab_core::synth::synth;

fn main() {
    let t = Instant::now();
    let (d, truth) = synth(42, 730, 19_723);
    println!("synth {:?}", t.elapsed());
    let mut lab = Lab::with_data(d);
    lab.set_labels(truth);
    for job in [
        r#"{"kind":"nights"}"#,
        r#"{"kind":"detect"}"#,
        r#"{"kind":"night","day":20080}"#,
        r#"{"kind":"hrv4t"}"#,
        r#"{"kind":"bench"}"#,
        r#"{"kind":"circadian","from":19723,"to":20452}"#,
        r#"{"kind":"event_study","label_kind":"illness","metrics":["hr_min30","ln_rmssd"],"n_boot":200}"#,
        r#"{"kind":"teardown"}"#,
        r#"{"kind":"dfm"}"#,
    ] {
        let t = Instant::now();
        let r = lab.run(job).unwrap();
        println!(
            "{:<60} {:>8.0?} {} bytes",
            &job[..job.len().min(60)],
            t.elapsed(),
            r.json.len()
        );
    }
}
