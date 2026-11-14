//! Every method the app uses: what it computes, its parameters, the data it needs, how far to trust it on wrist data
//! (A solid, B good for your own trends, C experimental, D shown for completeness) and the papers it comes from.

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Ref {
    pub key: &'static str,
    pub citation: &'static str,
    pub title: &'static str,
    pub year: u16,
    /// a DOI link, or a title search where no DOI was recorded
    pub url: String,
    pub doi: bool,
}

#[derive(Serialize, Clone)]
pub struct ParamSpec {
    pub name: &'static str,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: &'static str,
    pub help: &'static str,
}

#[derive(Serialize, Clone)]
pub struct MethodSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub summary: &'static str,
    pub formula: &'static str,
    pub params: Vec<ParamSpec>,
    pub inputs: Vec<&'static str>,
    pub min_resolution_s: Option<u32>,
    pub validity: char,
    pub refs: Vec<&'static str>,
}

// key, citation, title, year, doi (empty: search by title)
const REFS: &[(&str, &str, &str, u16, &str)] = &[
    ("taskforce1996", "Task Force 1996", "Heart rate variability: standards of measurement, physiological interpretation and clinical use", 1996, "10.1161/01.CIR.93.5.1043"),
    ("shaffer2017", "Shaffer & Ginsberg 2017", "An overview of heart rate variability metrics and norms", 2017, "10.3389/fpubh.2017.00258"),
    ("laborde2017", "Laborde 2017", "Heart rate variability and cardiac vagal tone in psychophysiological research", 2017, "10.3389/fpsyg.2017.00213"),
    ("sacha2013", "Sacha 2013", "Why should one normalize heart rate variability with respect to average heart rate", 2013, "10.3389/fphys.2013.00306"),
    ("berntson1997", "Berntson 1997", "Heart rate variability: origins, methods, and interpretive caveats", 1997, ""),
    ("plews2014", "Plews 2012-2014", "The practical application of heart rate variability (PhD thesis, contains Plews 2012, 2013 and 2014)", 2014, "10.1007/s40279-013-0071-8"),
    ("buchheit2014", "Buchheit 2014", "Monitoring training status with HR measures: do all roads lead to Rome?", 2014, "10.3389/fphys.2014.00073"),
    ("schneider2018", "Schneider 2018", "Heart rate monitoring in team sports: a conceptual framework", 2018, ""),
    ("esco2025", "Esco 2025", "Monitoring training adaptation and recovery status in athletes using HRV via mobile devices", 2025, ""),
    ("lemeur2013", "Le Meur 2013", "Evidence of parasympathetic hyperactivity in functionally overreached athletes", 2013, "10.1249/MSS.0b013e3182980125"),
    ("stanley2013", "Stanley 2013", "Cardiac parasympathetic reactivation following exercise", 2013, "10.1007/s40279-013-0083-4"),
    ("vesterinen2016", "Vesterinen 2016", "Individual endurance training prescription with heart rate variability", 2016, "10.1249/MSS.0000000000000910"),
    ("nuuttila2022", "Nuuttila 2022", "Individualized endurance training based on recovery and training status in recreational runners", 2022, ""),
    ("manresa2021", "Manresa-Rocamora 2021", "HRV-guided training: a methodological systematic review with meta-analysis", 2021, ""),
    ("alfonso2025", "Alfonso 2025", "Individual training prescribed by HRV, heart rate and well-being scores in experienced cyclists", 2025, ""),
    ("schaffarczyk2026", "Schaffarczyk 2026", "HRV-guided endurance training: strengths, weaknesses, opportunities and threats", 2026, ""),
    ("altini2021", "Altini & Plews 2021", "What is behind changes in resting heart rate and heart rate variability?", 2021, "10.3390/s21237932"),
    ("piatrikova2021", "Piatrikova 2021", "Monitoring the HRV responses to training loads in competitive swimmers using a smartphone app and the Banister model", 2021, ""),
    ("mishica2022", "Mishica 2022", "Evaluation of nocturnal vs. morning measures of heart rate indices in young athletes", 2022, ""),
    ("nuuttila2024", "Nuuttila 2024", "Morning versus nocturnal HR and HRV responses to intensified training in recreational runners", 2024, "10.1186/s40798-024-00806-5"),
    ("nuuttila2025", "Nuuttila 2025", "Monitoring sleep and nightly recovery with wrist-worn wearables", 2025, ""),
    ("herzig2017", "Herzig 2017", "Reproducibility of heart rate variability is parameter and sleep stage dependent", 2017, "10.3389/fphys.2017.01100"),
    ("dial2025", "Dial 2025", "Validation of nocturnal resting heart rate and HRV in consumer wearables", 2025, "10.14814/phy2.70527"),
    ("kinnunen2020", "Kinnunen 2020", "Accuracy of nocturnal HR and HRV assessed via ring PPG", 2020, ""),
    ("natarajan2020hrv", "Natarajan 2020", "Heart rate variability with photoplethysmography in 8 million individuals", 2020, "10.1016/S2589-7500(20)30246-6"),
    ("peng1995", "Peng 1995", "Quantification of scaling exponents and crossover phenomena in nonstationary heartbeat time series", 1995, "10.1063/1.166141"),
    ("gronwald2020", "Gronwald 2020", "Fractal correlation properties of HRV: a new biomarker for intensity distribution", 2020, ""),
    ("rogers2021aet", "Rogers 2021 (aerobic)", "A new detection method defining the aerobic threshold based on fractal correlation properties of HRV", 2021, ""),
    ("rogers2021ant", "Rogers 2021 (anaerobic)", "Detection of the anaerobic threshold in endurance sports using correlation properties of HRV", 2021, ""),
    ("rogers2021sensors", "Rogers 2021 (artefacts)", "Influence of artefact correction and recording device type on a non-linear HRV biomarker", 2021, ""),
    ("rogers2022", "Rogers & Gronwald 2022", "Fractal correlation properties of HRV as a biomarker for intensity distribution: an update", 2022, ""),
    ("schaffarczyk2023", "Schaffarczyk 2023", "Validation of a non-linear index of HRV to determine thresholds in women", 2023, ""),
    ("vanhooren2023", "Van Hooren 2023", "The between-day reliability of correlation properties of HRV during running", 2023, ""),
    ("vanhooren2025", "Van Hooren 2025", "Correlation properties of HRV to assess the first ventilatory threshold and fatigue in runners", 2025, "10.1080/02640414.2023.2277034"),
    ("brennan2001", "Brennan 2001", "Do existing measures of Poincare plot geometry reflect nonlinear features of HRV?", 2001, "10.1109/10.959330"),
    ("lipponen2019", "Lipponen & Tarvainen 2019", "A robust algorithm for HRV time series artefact correction using novel beat classification", 2019, "10.1080/03091902.2019.1640306"),
    ("roshan2021", "Roshan 2021", "A comparison of methods to generate adaptive reference ranges in longitudinal monitoring", 2021, ""),
    ("williams2018", "Williams 2018", "Modelling the HRV response to training loads in elite rugby sevens players", 2018, ""),
    ("chalencon2012", "Chalencon 2012", "A model for the training effects in swimming: parasympathetic activity, performance and fatigue", 2012, "10.1371/journal.pone.0052636"),
    ("kolossa2017", "Kolossa 2017", "Performance estimation using the fitness-fatigue model with Kalman filter feedback", 2017, "10.1515/ijcss-2017-0010"),
    ("phillips2017", "Phillips 2017", "Irregular sleep/wake patterns are associated with poorer academic performance and delayed circadian and sleep/wake timing", 2017, "10.1038/s41598-017-03171-4"),
    ("vansomeren1999", "Van Someren 1999", "Bright light therapy: improved sensitivity to its effects on rest-activity rhythms in Alzheimer patients by application of nonparametric methods", 1999, "10.3109/07420529909998724"),
    ("wittmann2006", "Wittmann 2006", "Social jetlag: misalignment of biological and social time", 2006, "10.1080/07420520500545979"),
    ("jankowski2017", "Jankowski 2017", "Sleep-corrected social jetlag", 2017, "10.1080/07420528.2016.1231689"),
    ("roenneberg2012", "Roenneberg 2012", "Social jetlag and obesity", 2012, "10.1016/j.cub.2012.03.038"),
    ("fischer2016", "Fischer 2016", "Composite phase deviation: a new measure of sleep timing irregularity", 2016, "10.1038/srep38601"),
    ("cornelissen2014", "Cornelissen 2014", "Cosinor-based rhythmometry", 2014, "10.1186/1742-4682-11-16"),
    ("bowman2021", "Bowman 2021", "A method for characterizing daily physiology from widely used wearables", 2021, "10.1016/j.crmeth.2021.100058"),
    ("forger1999", "Forger, Jewett & Kronauer 1999", "A simpler model of the human circadian pacemaker", 1999, "10.1177/074873099129000867"),
    ("huang2021", "Huang 2021", "Predicting circadian phase across populations: a comparison of mathematical models and wearable devices", 2021, ""),
    ("vandongen2003", "Van Dongen 2003", "The cumulative cost of additional wakefulness", 2003, "10.1093/sleep/26.2.117"),
    ("mishra2020", "Mishra 2020", "Pre-symptomatic detection of COVID-19 from smartwatch data", 2020, "10.1038/s41551-020-00640-6"),
    ("alavi2022", "Alavi 2022", "Real-time alerting system for COVID-19 and other stress events using wearable data", 2022, "10.1038/s41591-021-01593-2"),
    ("quer2021", "Quer 2021", "Wearable sensor data and self-reported symptoms for COVID-19 detection", 2021, "10.1038/s41591-020-1123-x"),
    ("natarajan2020", "Natarajan 2020", "Assessment of physiological signs associated with COVID-19 measured using wearable devices", 2020, "10.1038/s41746-020-00363-7"),
    ("pietila2018", "Pietila 2018", "Acute effect of alcohol intake on cardiovascular autonomic regulation during the first hours of sleep in a large real-world sample of Finnish employees", 2018, "10.2196/mental.9519"),
    ("grosicki2026", "Grosicki 2026", "Within-person effects of alcohol on resting heart rate and HRV in free-living wearable data", 2026, "10.1371/journal.pdig.0001284"),
    ("willoughby2025", "Willoughby 2025", "Sleep and travel across time zones in more than 57,000 wearable users", 2025, "10.1093/sleep/zsaf077"),
    ("quer2022", "Quer 2022", "Using smartwatch data to understand physiological responses to COVID-19 vaccination", 2022, "10.1038/s41746-022-00591-z"),
    ("guan2022", "Guan 2022", "Wearable sensor data detect vaccine side effects", 2022, "10.1038/s41746-022-00683-w"),
    ("shilaih2018", "Shilaih 2018", "Modern fertility awareness methods: wrist wearables capture the changes of temperature associated with the menstrual cycle", 2018, "10.1042/BSR20171279"),
    ("adams2007", "Adams & MacKay 2007", "Bayesian online changepoint detection", 2007, "arXiv:0710.3742"),
    ("page1954", "Page 1954", "Continuous inspection schemes", 1954, "10.1093/biomet/41.1-2.100"),
    ("hopkins2000", "Hopkins 2000", "Measures of reliability in sports medicine and science", 2000, "10.2165/00007256-200030010-00001"),
    ("swinton2018", "Swinton 2018", "A statistical framework to interpret individual response to intervention", 2018, "10.3389/fnut.2018.00041"),
    ("koo2016", "Koo & Li 2016", "A guideline of selecting and reporting intraclass correlation coefficients for reliability research", 2016, "10.1016/j.jcm.2016.02.012"),
    ("schyvens2025", "Schyvens 2025", "Performance of wearable sleep trackers against polysomnography", 2025, ""),
    ("schiefer2021", "Schiefer 2021", "Pulse oximetry of the Garmin Fenix 5X Plus at altitude", 2021, "10.3390/s21196363"),
    ("mouritzen2020", "Mouritzen 2020", "Assessing the performance of a commercial multisensory sleep tracker", 2020, "10.1371/journal.pone.0243214"),
    ("chinoy2021", "Chinoy 2021", "Performance of seven consumer sleep-tracking devices compared with polysomnography", 2021, "10.1093/sleep/zsaa291"),
    ("benDov2007", "Ben-Dov 2007", "Predictors of all-cause mortality in clinical ambulatory monitoring", 2007, ""),
    ("lomb1976", "Lomb 1976", "Least-squares frequency analysis of unequally spaced data", 1976, "10.1007/BF00648343"),
    ("daza2018", "Daza 2018", "The N-of-1 causal framework for personalized health", 2018, "10.3414/ME16-02-0044"),
    ("brodersen2015", "Brodersen 2015", "Inferring causal impact using Bayesian structural time-series models", 2015, "10.1214/14-AOAS788"),
    ("ledoit2004", "Ledoit & Wolf 2004", "A well-conditioned estimator for large-dimensional covariance matrices", 2004, "10.1016/S0047-259X(03)00096-4"),
    ("shumway1982", "Shumway & Stoffer 1982", "An approach to time series smoothing and forecasting using the EM algorithm", 1982, "10.1111/j.1467-9892.1982.tb00349.x"),
    ("kario2003", "Kario 2003", "Morning surge in blood pressure as a predictor of silent and clinical cerebrovascular disease", 2003, "10.1161/01.CIR.0000056521.67546.AA"),
    ("jugel2014", "Jugel 2014", "M4: a visualization-oriented time series data aggregation", 2014, ""),
    ("cleveland1990", "Cleveland 1990", "STL: A seasonal-trend decomposition procedure based on loess", 1990, ""),
    ("firstbeat_stress", "Firstbeat 2014", "Stress and recovery analysis method based on 24-hour heart rate variability (white paper)", 2014, ""),
    ("firstbeat_sleep", "Firstbeat 2019", "A sleep analysis method based on heart rate variability (white paper)", 2019, ""),
];

pub fn refs() -> Vec<Ref> {
    REFS.iter()
        .map(|&(key, citation, title, year, doi)| Ref {
            key,
            citation,
            title,
            year,
            url: if doi.is_empty() {
                format!(
                    "https://scholar.google.com/scholar?q={}",
                    title.replace(' ', "+")
                )
            } else if let Some(id) = doi.strip_prefix("arXiv:") {
                format!("https://arxiv.org/abs/{id}")
            } else {
                format!("https://doi.org/{doi}")
            },
            doi: !doi.is_empty(),
        })
        .collect()
}

fn p(
    name: &'static str,
    default: f64,
    min: f64,
    max: f64,
    step: f64,
    unit: &'static str,
    help: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        default,
        min,
        max,
        step,
        unit,
        help,
    }
}

#[allow(clippy::too_many_arguments)]
fn m(
    id: &'static str,
    name: &'static str,
    group: &'static str,
    validity: char,
    summary: &'static str,
    formula: &'static str,
    inputs: &[&'static str],
    res: Option<u32>,
    refs: &[&'static str],
    params: Vec<ParamSpec>,
) -> MethodSpec {
    MethodSpec {
        id,
        name,
        group,
        summary,
        formula,
        params,
        inputs: inputs.to_vec(),
        min_resolution_s: res,
        validity,
        refs: refs.to_vec(),
    }
}

pub fn methods() -> Vec<MethodSpec> {
    vec![
        m("night.timing", "Sleep timing", "Nights", 'A', "Onset, wake and midpoint of the main sleep from Garmin's sleep window, as clock times.", "mid = (onset + wake) / 2; clock times are local, averaged as angles on a 24 h circle.", &["sleeps"], None, &["phillips2017"], vec![]),
        m("night.hr", "Nocturnal heart rate", "Nights", 'B', "The lowest 30-minute mean of heart rate while asleep, the nadir of a 15-minute median, the first-half-hour level, and the early-minus-late difference.", "hr_min30 = min over centred 30-min means (at least 20 samples); nadir_frac = time of nadir / time in bed.", &["hr", "sleeps"], Some(120), &["benDov2007", "pietila2018"], vec![]),
        m("night.dip", "Heart-rate dip", "Nights", 'B', "How far sleeping heart rate falls below the quiet daytime heart rate of the day before.", "dip% = 100 (day_rest_hr - sleep_hr_mean) / day_rest_hr, from 10:00-20:00 minutes with stress at most 25 and no activity within 30 min.", &["hr", "stress", "activities"], Some(120), &["benDov2007"], vec![]),
        m("night.hrv", "Overnight HRV", "Nights", 'B', "RMSSD of the night from Garmin's five-minute values: mean, mean of logs, quadratic mean, first four hours, last two hours.", "ln_rmssd = ln(mean of the five-minute values); rmssd_4h = mean from 30 to 270 minutes after onset; rmssd_norm = rmssd / (60000 / mean HR).", &["hrv5"], Some(300), &["taskforce1996", "plews2014", "sacha2013", "nuuttila2024", "dial2025"], vec![]),
        m("night.stages", "Sleep stages", "Nights", 'D', "Stage minutes, awakenings, WASO and transitions from Garmin's wrist-estimated stages. Garmin detects only about 28 % of awake periods, so these are shown for completeness, never used for inference.", "waso = awake minutes between the first and last non-awake segment.", &["stages"], None, &["schyvens2025", "mouritzen2020", "chinoy2021"], vec![]),
        m("night.stress", "Sleep stress and rest share", "Nights", 'C', "Garmin's HRV-derived stress while asleep, and the share of measured minutes at rest (stress below 26).", "mean of measured one-minute stress between onset and wake.", &["stress"], Some(180), &["firstbeat_stress"], vec![]),
        m("night.resp", "Breathing rate", "Nights", 'B', "Mean, spread and range of the wrist-estimated breathing rate through the night.", "mean and standard deviation of one-minute values.", &["resp"], Some(120), &["natarajan2020"], vec![]),
        m("night.spo2", "Blood oxygen", "Nights", 'C', "Mean and lowest SpO2, minutes below 90 %, and drops of 3 points per hour (ODI3). Garmin overestimates SpO2 at altitude, so show it as a trend only.", "ODI3 counts readings at least 3 points under the median of the previous 10 minutes, at least 3 minutes apart, per hour in bed.", &["spo2"], Some(60), &["schiefer2021"], vec![]),
        m("baseline.normal_range", "Baseline and normal range", "HRV", 'B', "The 7-night mean of ln RMSSD (the baseline), judged against the mean plus or minus a multiple of the standard deviation of the nights before it (the normal range).", "base7 = mean of ln RMSSD over 7 nights (at least 4); range = mean +/- k SD of the previous N nights (at least max(14, N/3)).", &["hrv5", "days"], Some(300), &["plews2014", "vesterinen2016", "nuuttila2022", "altini2021"], vec![p("range_nights", 60.0, 14.0, 120.0, 1.0, "nights", "nights the normal range is built from"), p("k_night", 1.5, 0.5, 2.5, 0.25, "SD", "width of the band a night is judged against"), p("k_baseline", 0.5, 0.25, 1.0, 0.05, "SD", "width of the band the baseline is judged against (the smallest worthwhile change)")]),
        m("hrv.trends", "Trend patterns", "HRV", 'C', "Four 7-night series (baseline, sleeping heart rate, coefficient of variation, normalised HRV) with their 14-night slopes compared with all earlier slopes; the combinations name coping well, accumulated fatigue, saturation and maladaptation.", "pattern from the sign of z for slopes beyond one SD, after 60 nights of history.", &["hrv5", "sleeps"], Some(300), &["plews2014", "buchheit2014", "sacha2013"], vec![]),
        m("hrv.response", "Response to training", "HRV", 'B', "How far each night lands from the baseline that ended the night before, by the training load of the day before, for the three nights after.", "dev = ln RMSSD - base7 (shifted one night); mean and 95 % interval by load class.", &["hrv5", "activities"], Some(300), &["altini2021", "stanley2013"], vec![]),
        m("hrv.inside_night", "Inside the night", "HRV", 'C', "Every night's five-minute values since falling asleep, HRV by sleep stage, and which version of the number (whole night, first four hours, Garmin's) is most sensitive to training.", "effect = (deviation after rest days - deviation after hard days) / SD of deviations.", &["hrv5", "stages"], Some(300), &["herzig2017", "nuuttila2024", "mishica2022"], vec![]),
        m("hrv.state_space", "Training-response model", "HRV", 'C', "A state-space model of nightly ln RMSSD: a slowly wandering level, a training effect that decays, and noise; fitted by maximum likelihood, it says what tonight should be and how surprising it was.", "y_t = level_t + beta g_t + e_t; g_t = exp(-1/tau) g_{t-1} + load_{t-1}/100; Kalman filter, Nelder-Mead.", &["hrv5", "activities"], Some(300), &["chalencon2012", "williams2018", "kolossa2017", "roshan2021"], vec![]),
        m("beats.cleaning", "Beat cleaning", "Beats", 'B', "Beat intervals from activities recorded with Log HRV: sensor gaps dropped, out-of-range beats and beats more than 20 % from the 11-beat median marked as artefacts.", "kept: 300-2000 ms and within 20 % of the rolling median of 11.", &["rr"], None, &["lipponen2019", "taskforce1996"], vec![]),
        m("beats.rmssd", "RMSSD, SD1 and SD2", "Beats", 'B', "RMSSD over a window or the recording, and the Poincare plot's SD1 (short-term) and SD2 (long-term) variability.", "RMSSD = sqrt(mean of squared successive differences); SD1 = SD(diff)/sqrt 2; SD2 = SD(sum)/sqrt 2.", &["rr"], None, &["taskforce1996", "brennan2001", "shaffer2017"], vec![]),
        m("beats.spectrum", "Spectrum", "Beats", 'C', "Lomb-Scargle power of the cleaned beat series in 5-minute half-overlapping segments, with VLF, LF and HF bands.", "linear detrend, unnormalised Lomb-Scargle at 300 frequencies from 0.01 to 0.6 Hz, averaged.", &["rr"], None, &["lomb1976", "taskforce1996"], vec![]),
        m("dfa.a1", "DFA alpha-1 thresholds", "Thresholds", 'B', "The fractal scaling of beat intervals during exercise falls from about 1 (easy) through 0.75 (first threshold) to 0.5 (second threshold); the heart rates where a fitted line crosses them are your thresholds.", "alpha-1 = slope of log F(n) against log n over 4 to 16 beats in 2-minute windows every 30 s; thresholds from the first hour of efforts over 6 weeks.", &["rr"], None, &["peng1995", "gronwald2020", "rogers2021aet", "rogers2021ant", "rogers2021sensors", "vanhooren2023", "vanhooren2025", "lipponen2019"], vec![]),
        m("circadian.nonparametric", "Interdaily stability, intradaily variability, L5 and M10", "Circadian", 'A', "How regular the rest-activity pattern is from day to day (IS), how fragmented within the day (IV), and the least and most active hours (L5, M10).", "IS = n sum_h (xbar_h - xbar)^2 / (p sum (x_i - xbar)^2); IV = n sum (x_i - x_{i-1})^2 / ((n-1) sum (x_i - xbar)^2); RA = (M10 - L5)/(M10 + L5).", &["steps", "hr"], Some(60), &["vansomeren1999"], vec![]),
        m("circadian.sri", "Sleep Regularity Index", "Circadian", 'B', "The chance of being in the same sleep state at the same clock time on consecutive days.", "SRI = 200 P(same state 24 h apart) - 100, per rolling 14 days.", &["sleeps"], Some(60), &["phillips2017"], vec![]),
        m("circadian.cosinor", "Cosinor", "Circadian", 'B', "A 24-hour cosine fitted to hourly activity and to heart rate away from exercise: mesor, amplitude and acrophase (the clock hour of the peak).", "y = M + A cos(2 pi (t - phi) / 24), by least squares on cos and sin.", &["steps", "hr"], Some(60), &["cornelissen2014"], vec![]),
        m("circadian.chronotype", "Social jet lag, chronotype, phase deviation", "Circadian", 'B', "The gap between midsleep on free days and workdays, a chronotype corrected for sleeping in, and how far each night is from your recent pattern.", "SJL = |MSF - MSW|; MSFsc = MSF - (SD_f - SD_week)/2; CPD = sqrt(mistiming^2 + irregularity^2).", &["sleeps"], None, &["wittmann2006", "jankowski2017", "roenneberg2012", "fischer2016"], vec![p("free_days", 5.0, 0.0, 6.0, 1.0, "weekday", "wake weekdays counted as free days (Monday = 0)")]),
        m("circadian.sleep_debt", "Sleep debt", "Circadian", 'C', "How far recent nights fell short of your sleep need, weighted toward the last few nights, with Garmin's overestimate of sleep time removed.", "debt = sum_k w_k (need - (TST_k - bias)), w_k proportional to exp(-k/4) over 14 nights.", &["sleeps"], None, &["vandongen2003"], vec![p("tst_bias_min", 30.0, 0.0, 60.0, 5.0, "min", "how much Garmin overestimates total sleep time")]),
        m("circadian.hr_rhythm", "Heart-rate rhythm", "Circadian", 'B', "Heart rate across the day as a cosine plus the effects of movement and sleep: the clock hour it peaks, its amplitude and what a step costs.", "HR = b + a cos(2 pi (t - phi)/24) + c ln(1 + steps) + d asleep.", &["hr", "steps", "sleeps"], Some(300), &["bowman2021"], vec![]),
        m("circadian.dlmo", "Melatonin onset (model)", "Circadian", 'C', "A model of the circadian pacemaker driven by a light proxy from sleep and movement estimates when melatonin would rise; experimental, accurate to about an hour on regular schedules at best.", "Forger, Jewett and Kronauer 1999 as in Arcascope's circadian: dx/dt = pi/12 (xc + B), CBTmin = minimum of x, DLMO = CBTmin - 7 h.", &["steps", "sleeps"], Some(60), &["forger1999", "huang2021"], vec![]),
        m("detect.fusion", "Unusual runs of nights", "Inference", 'B', "Each night's signals (resting heart rate, HRV, breathing, stress, temperature) are compared with your own robust baseline; their shared deviation is tested with Hotelling's T-squared against the correlation seen in your previous nights.", "z = (x - median) / MAD over the previous 28 non-excluded nights; T2 = z' R_s^-1 z with R_s = 0.8 R + 0.2 I; p from chi-square.", &["nights"], None, &["mishra2020", "alavi2022", "natarajan2020", "ledoit2004"], vec![p("baseline_window", 28.0, 14.0, 90.0, 1.0, "nights", "nights the personal baseline is built from")]),
        m("detect.cusum", "CUSUM and two-night rule", "Inference", 'B', "Slow accumulation of high nocturnal heart rate, and the rule 'at least 4 bpm above your median on two nights in a row'.", "S_t = max(0, S_{t-1} + z_t - 0.5), alarm above 4; or hr_min30 - median(previous 28) >= 4 on 2 nights.", &["nights"], None, &["page1954", "mishra2020", "alavi2022"], vec![]),
        m("detect.changepoint", "Changepoints", "Inference", 'C', "Bayesian online changepoint detection on HRV and heart rate finds lasting shifts (a medication, a new device, fitness) and asks what changed.", "run-length posterior with a Normal-Gamma model, hazard 1/250.", &["nights"], None, &["adams2007"], vec![]),
        m("detect.classify", "What kind of episode", "Inference", 'C', "Naive Bayes over illness, vaccination, alcohol, hard training, stress and other, starting from published effect sizes and moving toward your own as you confirm or deny proposals.", "log posterior = log prior + sum of Gaussian log likelihoods of the deviations + night-count and context terms.", &["nights"], None, &["alavi2022", "natarajan2020", "quer2022", "guan2022", "pietila2018", "altini2021", "grosicki2026"], vec![p("ask_min", 0.3, 0.0, 1.0, 0.05, "", "ask when the top class is at least this likely"), p("ask_max", 0.95, 0.5, 1.0, 0.01, "", "label on its own above this"), p("question_budget_per_week", 2.0, 0.0, 14.0, 1.0, "questions", "at most this many questions a week")]),
        m("detect.exposures", "Time zones and altitude", "Inference", 'A', "Changes of time zone (clock changes excepted) and days spent well above your home altitude are certain exposures, labelled automatically with a one-click undo.", "tz change = offset(day) - offset(day-1); altitude above max(home + 700 m, 1500 m).", &["days", "altitude"], None, &["willoughby2025"], vec![]),
        m("events.study", "Event study", "Analysis", 'B', "Every metric around the nights of an event, against control nights of the same weekday, season and training load that sit well away from any other event, with a bootstrap interval over events.", "diff(r) = mean over events of (x_e(r) - mean of matched controls' x(r)); percentile bootstrap.", &["nights", "labels"], None, &["daza2018", "brodersen2015"], vec![p("controls", 5.0, 1.0, 20.0, 1.0, "nights", "control nights per event"), p("doy_window", 45.0, 7.0, 182.0, 1.0, "days", "how far apart in the season controls may be")]),
        m("bench.reliability", "Metric bench", "Analysis", 'B', "For every number: its typical error and reliability on quiet nights, the smallest change that is real, how it responds to your confirmed events, and its signal to noise.", "TE = SD of consecutive quiet-night differences / sqrt 2; MDC95 = 1.96 sqrt 2 TE; ICC(3,1); local-level Kalman q.", &["nights"], None, &["hopkins2000", "swinton2018", "koo2016"], vec![]),
        m("compare.nights", "Compare nights", "Analysis", 'B', "What is different about a chosen set of nights across every feature, label and tag, ranked by Cliff's delta.", "delta = P(a > b) - P(a < b).", &["nights"], None, &[], vec![]),
        m("model.strain", "Strain factor", "Analysis", 'C', "One latent factor behind nocturnal heart rate, HRV, breathing, stress and rest share, with training, travel, alcohol and illness as inputs; its surprises are the anomaly score.", "y_it = lambda_i f_t + e_it; f_t = phi f_{t-1} + gamma x_t + eta_t; EM with a Kalman smoother.", &["nights"], None, &["shumway1982"], vec![]),
        m("garmin.teardown", "Garmin score teardown", "Analysis", 'C', "What Garmin's own published inputs predict for its sleep score and training readiness, a rebuilt Body Battery, and the window and width that reproduce the HRV status band.", "OLS and non-negative least squares on the components; Body Battery as a rate table by stress band and sleep; grid search of the baseline rule.", &["days", "body_battery", "stress"], None, &["firstbeat_stress", "firstbeat_sleep"], vec![]),
        m("experiments.n_of_1", "Self-experiments", "Analysis", 'B', "Alternating blocks of a change and the usual, order randomised, washout nights left out, adherence recorded; the answer is a probability of a worthwhile effect, with 'inconclusive' kept separate from 'no effect'.", "effect = mean(A) - mean(B); se from an autocorrelation-corrected sample size; normal posterior against the smallest worthwhile change.", &["nights"], None, &["daza2018", "swinton2018"], vec![]),
        m("render.m4", "Line downsampling (M4)", "Display", 'A', "Long lines are drawn from the first, lowest, highest and last value of each pixel column, which is exactly what a full-resolution line would draw.", "per pixel column: first, min, max, last.", &[], None, &["jugel2014"], vec![]),
        m("display.seasonal", "Seasonal view", "Display", 'B', "Each day's deviation from the metric's own centred one-year median, smoothed over 15 days, drawn one line per year on a shared calendar so the same weeks of different years line up.", "dev = mean over 15 centred days of (x - median of x over 365 centred days, at least 120); years with at least 180 days in the range, at least two years.", &["days", "nights"], None, &["cleveland1990"], vec![]),
    ]
}

pub fn registry_json() -> String {
    serde_json::json!({"methods": methods(), "refs": refs()}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reference_cited_exists_and_grades_are_valid() {
        let keys: Vec<&str> = REFS.iter().map(|r| r.0).collect();
        for method in methods() {
            assert!("ABCD".contains(method.validity), "{}", method.id);
            for r in &method.refs {
                assert!(keys.contains(r), "{} cites unknown {}", method.id, r);
            }
        }
        let mut ids: Vec<&str> = methods().iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), methods().len());
        assert!(refs()
            .iter()
            .any(|r| r.url == "https://doi.org/10.1161/01.CIR.93.5.1043"));
        assert!(refs()
            .iter()
            .any(|r| r.url == "https://arxiv.org/abs/0710.3742"));
        assert!(registry_json().contains("\"methods\""));
    }
}
