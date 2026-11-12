# Inference

Wellness Lab looks through your nightly numbers for stretches that depart from your own normal, proposes what they might be, and asks you. It reports signs in the data, never a diagnosis, and it is not a medical device: it cannot tell you that you are ill, only that your nights look like the ones around an illness you labelled before, or like a hard week, a late drink, a flight. If you are unwell or worried, ask a clinician.

All of it runs on your own data and your own baseline. Nothing is compared with other people, and nothing leaves your browser or server (see [privacy](privacy.md)). Each method has an entry, with its citations and a validity grade, on the Methods page (`/methods`); the grades are in the [README](../README.md#validity-grades).

## What is detected

Everything starts from nightly features (`crates/core/src/nights.rs`): the lowest 30-minute heart rate of the night, overnight HRV (ln RMSSD), breathing rate, sleep stress, skin temperature when the watch records it, and the share of the night spent at rest.

### Personal baselines

Each signal is compared with a rolling robust baseline: the median and median absolute deviation of the previous 28 nights (the "baseline window" setting, 14 to 90), leaving out nights you labelled as illness, vaccination, travel, altitude or medication change (you can change that per label). The comparison is a z-score signed so that positive means "more strain": a higher heart rate, lower HRV, faster breathing, more stress, warmer skin, less rest. A signal is used only if it is present on 60 % of the last 90 nights.

### Fusion (grade B)

Looking at one number at a time misses episodes where five of them each move a little. The fused test takes all the signals present on a night and asks how unusual their combination is for you: Hotelling's T-squared with a correlation matrix estimated from your own previous nights (up to 120, at least 40 complete ones) and shrunk toward the identity (0.8 R + 0.2 I), with a chi-square p-value. A night counts when the signals together lean toward strain and p is below 0.05. It needs at least three signals and 30 nights of data.

### CUSUM and the two-night rule (grade B)

Two quicker alarms on nocturnal heart rate. CUSUM adds up the heart-rate z-score minus 0.5 each night and raises an alarm when the sum passes 4, then starts over: it catches a slow drift. The two-night rule (the `night_signal` alarm) fires when the lowest heart rate is at least 4 bpm above your median of the previous 28 nights on two nights in a row.

### Runs of nights

Flagged nights at most one night apart are grouped into one episode. A group is kept when it spans at least two nights, or has a night with p below 0.01, or has an alarm. A group that overlaps something you already labelled is skipped.

### Changepoints (grade C)

Bayesian online changepoint detection on HRV and nocturnal heart rate looks for lasting shifts: a new medication, a new watch, getting fitter. A shift is accepted only if the next 14 nights sit more than half a standard deviation from the 28 before. It is proposed as a medication change or something else and always asked about, never labelled on its own.

### Time zones and altitude (grade A)

These are certain rather than inferred. A change of time zone in Garmin's local clock offset becomes a travel episode (the spring and autumn clock changes are ignored), and days spent at least 700 m, and at least 1500 m, above your home altitude (set in Settings, otherwise the median of the last 90 nights) become an altitude episode. Both are labelled automatically, with an Undo, unless switched off in Settings. Episodes that overlap them are marked and asked about less (their priority is cut to 30 %), because travel and altitude move the same numbers.

## What kind of episode (grade C)

`crates/core/src/classify.rs` is a naive Bayes classifier over six classes: illness, vaccination, alcohol, hard training, stress and other. For an episode it weighs:

- how far resting heart rate, HRV, breathing, skin temperature and the early-versus-late-night heart-rate difference sit from your baseline, each as a Gaussian per class;
- how many nights it lasted (one, two to three, four or more): alcohol is usually one night, illness often several;
- context: a weekend (alcohol), whether the day before was a hard training day (hard training), and a rise in sleep stress (stress).

The starting point for each class is a template taken from published effect sizes (Mishra 2020, Alavi 2022 and Natarajan 2020 for illness, Quer 2022 and Guan 2022 for vaccination, Pietila 2018 and Altini and Plews 2021 for alcohol and training), with prior weights that sum to a small pseudo-count. Your answers then move it toward you: every episode you confirm, and every label you add yourself, adds to its class's count, and once a class has three examples its template mean shifts toward your own (weighted against the published value as if it were three more examples). Denied proposals teach the model what "other" looks like for you. The templates are population averages from small studies; early on, treat the class as a guess and the evidence (what moved, by how much) as the real information. The Inbox shows the evidence and the probability for every class.

## Asking you

A proposal reaches you only through a policy (`apply_policy` in `crates/core/src/detect.rs`), so the app asks few questions:

- It asks when the most likely class has a probability between `ask_min` (0.30) and `ask_max` (0.95). A likely illness (p-value below 0.2 with some illness probability) is asked about even when its class probability is low.
- Above `ask_max` it labels the episode itself, shown as inferred with an Undo. Certain exposures (time zone, altitude) are labelled the same way.
- A weekly budget (default 2, settable 0 to 14) caps questions: the highest priority first, where priority is the strength of the evidence times how unsure the class is, weighted up for illness. One extra question is allowed in a week for a likely illness.
- An answer is Confirm, Deny, Unsure or Snooze. A confirmed episode becomes a label; a denied one is not asked again unless its evidence grows by half; an unanswered question expires after 60 days.
- Questions go to the Inbox and, if configured, to ntfy or a webhook (see [self-hosting](self-hosting.md)). The wording is "possible illness, 3 nights from Nov 3: resting HR +5 bpm, HRV -18 %", with a link to confirm or deny.

Confirmed and added labels feed the event studies, the metric bench, the baselines and your self-experiments, so the answers are worth giving.

## What is never inferred

Atrial fibrillation or any other rhythm disorder, pregnancy, and mental-health conditions. The wrist data cannot support those claims, and a wrong guess there does harm. The cycle-tracking switch in Settings only adds a kind of event you log yourself; detection does not use it. The sleep-stage numbers (grade D: Garmin detects only about a quarter of awake periods) are shown but never feed any detection.

## Limits

- Every detection is statistical and personal. A quiet baseline makes small changes stand out; a noisy one hides real ones. The metric bench shows, per number, how reliable it is for you.
- Labels you never add leave the classifier at its published starting point.
- A watch that was not worn, a device change or a firmware update can look like an episode; the changepoint method exists partly for that.
- Detection reruns after each sync or import and is deterministic for the same data, labels and settings.
