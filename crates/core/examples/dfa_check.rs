use wellness_lab_core::lab::Lab;
use wellness_lab_core::synth::synth;

fn main() {
    let (d, _) = synth(42, 730, 19_723);
    let mut lab = Lab::with_data(d);
    let r = lab.run(r#"{"kind":"dfa"}"#).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r.json).unwrap();
    println!(
        "keys {:?}",
        v.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    for k in ["now", "thresholds", "history", "durability"] {
        if let Some(x) = v.get(k) {
            println!(
                "{k}: {}",
                x.to_string().chars().take(300).collect::<String>()
            );
        }
    }
    let r = lab.run(r#"{"kind":"sessions"}"#).unwrap();
    println!("sessions {}", r.json.chars().take(300).collect::<String>());
}
