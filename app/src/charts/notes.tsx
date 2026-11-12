// The notes of the HRV deep-dive notebook (nb-ycda538mxkv4h0ct), with the reference links replaced by the registry's
// DOI links and the numbers filled in from the job results. The prose follows the notebook's markdown cells.
import { Ref } from './lab';

const A1 = '\u03b1\u2081';
const num = (v: number, d = 1) => (Number.isFinite(v) ? v.toFixed(d) : '\u2013');
const big = (v: number) => v.toLocaleString('en');

export function IntroNote() {
  return (
    <>
      <h2>HRV deep dive</h2>
      <p>Heart-rate variability from the raw heartbeats up. It reproduces what HRV4Training does with a morning reading (a 7-day baseline judged against your own normal range, trend patterns, the response to training), looks inside each night at the 5-minute values Garmin averages away, finds aerobic and anaerobic thresholds from the fractal correlation of beat intervals during exercise (DFA {A1}), and fits a state-space model that says what tonight&apos;s HRV should be given your training.</p>
      <p>Every method cites the paper it comes from; the info button on each chart lists them.</p>
    </>
  );
}

export function NightsNote({ n, r, mae, cuts }: { n: number; r: number; mae: number; cuts: [number, number] }) {
  return (
    <>
      <h2>Nights</h2>
      <p>
        Overnight, the watch computes RMSSD over 5-minute windows of beat-to-beat intervals through sleep and reports their average as &quot;last night&apos;s HRV&quot;. The 5-minute values are available, so the nightly number is rebuilt from them
        {n > 0 && <>: across {big(n)} nights the rebuilt average matches Garmin&apos;s to r = {num(r, 3)} (mean absolute difference {num(mae)} ms)</>}
        . Garmin&apos;s own night-to-night accuracy against an ECG is about 10 % mean absolute error (<Ref k="dial2025">Dial 2025</Ref>), so single nights are noisy and everything here works on rolling windows.
      </p>
      <p>
        HRV is analysed on a log scale. RMSSD is right-skewed and its day-to-day changes are multiplicative, so the literature (and HRV4Training) uses ln rMSSD; HRV4Training shows it as 20 x ln rMSSD, which puts most people between 60 and 100 (<Ref k="plews2014">Plews 2013</Ref>, <Ref k="buchheit2014">Buchheit 2014</Ref>).
        Training days are classed by Garmin&apos;s training load into rest (none), easy, moderate and hard, the last three being terciles of your training days (cuts at {Number.isFinite(cuts[0]) ? cuts[0].toFixed(0) : '\u2013'} and {Number.isFinite(cuts[1]) ? cuts[1].toFixed(0) : '\u2013'}).
      </p>
    </>
  );
}

export function BaselineNote() {
  return (
    <>
      <h2>Baseline and normal range</h2>
      <p>This is HRV4Training&apos;s logic applied to the Garmin nights (<Ref k="altini2021">Altini &amp; Plews 2021</Ref>, <Ref k="plews2014">Plews 2013</Ref>):</p>
      <ul>
        <li><b>Tonight</b> is ln rMSSD of the night.</li>
        <li><b>Baseline</b> is its 7-night rolling mean, needing at least 4 nights in the window. Single nights move with what you ate, drank and did the evening before; the weekly mean is what responds to training (<Ref k="plews2014">Plews 2012</Ref>: in an athlete heading into overreaching every single day fell outside the normal band but only one weekly mean in five did).</li>
        <li><b>Normal range</b> is built from the nights before tonight: their mean plus or minus a multiple of their standard deviation. HRV4Training uses 60 nights so that the range neither chases the baseline nor gets stuck in last season; the HRV-guided training trials used 4 weeks (<Ref k="vesterinen2016">Vesterinen 2016</Ref>, <Ref k="nuuttila2022">Nuuttila 2022</Ref>).</li>
        <li><b>Two bands.</b> A night is judged against the wide band (HRV4Training&apos;s 1.5 SD); the baseline against the narrow one, the smallest worthwhile change of 0.5 SD used in the training trials.</li>
      </ul>
      <p>The advice per night follows the trials&apos; rule (baseline outside the narrow band means train easy until it returns): <b>back off</b> when the baseline is below its band and tonight is also low, <b>keep it easy</b> when the baseline alone is low, <b>watch</b> when only tonight is low, <b>high</b> when tonight is above its range, otherwise <b>normal</b>. A high value is not automatically good: in functional overreaching HRV can rise (<Ref k="lemeur2013">Le Meur 2013</Ref>), so a run of highs during a hard block deserves the same suspicion as a run of lows.</p>
    </>
  );
}

export function TrendsNote() {
  return (
    <>
      <h2>Trends</h2>
      <p>A baseline that drifts is only interesting together with heart rate and with how much the nights scatter around it. HRV4Training reads four 7-day series together (<Ref k="plews2014">Plews 2012, 2013</Ref>, <Ref k="buchheit2014">Buchheit 2014</Ref>):</p>
      <ul>
        <li><b>ln rMSSD baseline</b>, the vagal signal itself.</li>
        <li><b>Sleeping heart rate</b>, which should move the other way.</li>
        <li><b>CV</b>, the coefficient of variation of ln rMSSD over the week. A falling CV with a rising baseline means you are absorbing the training; a falling CV with a falling baseline was the warning sign before non-functional overreaching in Plews&apos; triathlete.</li>
        <li><b>rMSSD normalised by heart rate</b> (rMSSD divided by the mean RR interval). HRV depends on heart rate mathematically, so this separates genuine changes in vagal tone from changes in heart rate (<Ref k="sacha2013">Sacha 2013</Ref>).</li>
      </ul>
      <p>Each series&apos; 14-night slope is compared with all of its earlier slopes; a slope more than one standard deviation from their mean counts as a real move. The combinations give HRV4Training&apos;s patterns: <b>coping well</b> (HRV steady or up, heart rate steady or down, CV down), <b>accumulated fatigue</b> (HRV down, heart rate up), <b>saturation</b> (both down, typical of very fit athletes or heavy blocks), <b>maladaptation</b> (heart rate and CV both up). Patterns need 60 nights of history before they start.</p>
    </>
  );
}

export function AcuteNote({ hard, rest }: { hard: number | null; rest: number | null }) {
  return (
    <>
      <h2>Training response</h2>
      <p>HRV4Training&apos;s acute analysis asks how far each night lands from your baseline, sorted by what you did the day before. Here a night&apos;s deviation is its ln rMSSD minus the 7-night baseline that ended the night before, so the baseline cannot contain the night it judges.</p>
      <p>
        {hard !== null && rest !== null && <>After a <b>hard</b> day your next night sits {num(Math.abs(hard) * 100)} % {hard < 0 ? 'below' : 'above'} baseline on average; after a <b>rest</b> day {num(Math.abs(rest) * 100)} % {rest > 0 ? 'above' : 'below'}. </>}
        That size is typical: across HRV4Training users a training day moved HRV by about 4.6 %, heavy drinking by 12 % and illness by 10 % (<Ref k="altini2021">Altini &amp; Plews 2021</Ref>). The second and third nights show how fast it recovers: parasympathetic activity is usually back within 24 h after easy work, 24 to 48 h after threshold work and 48 h or more after high intensity (<Ref k="stanley2013">Stanley 2013</Ref>).
      </p>
    </>
  );
}

export function NightNote({ whole, first4 }: { whole: number | null; first4: number | null }) {
  return (
    <>
      <h2>Inside the night</h2>
      <p>Garmin&apos;s number is the mean of 80 to 110 five-minute values. They are not interchangeable: HRV is highest and most reproducible in deep sleep, lower and noisier in REM and around awakenings (<Ref k="herzig2017">Herzig 2017</Ref>), and the night drifts as the evening&apos;s food, drink and training wear off. In runners whose training was deliberately overloaded, the most load-sensitive part of the night was the first four hours starting 30 minutes after sleep onset, and nightly and morning responses to the overload disagreed more than they agreed (<Ref k="nuuttila2024">Nuuttila 2024</Ref>). Under steady training, nightly and morning RMSSD agree closely (r around 0.9, <Ref k="mishica2022">Mishica 2022</Ref>).</p>
      <p>
        The barcode puts every night on one row, time since falling asleep across. On the right, the same test as in Training response, done three ways: how far a night after a hard day falls below one after a rest day, in units of that measure&apos;s night-to-night standard deviation.
        {whole !== null && first4 !== null && <> Here the whole-night average moves {num(whole, 2)} SD and the first four hours {num(first4, 2)} SD, so {first4 > whole ? 'the early-night window is the more sensitive signal for you' : 'the whole-night average is at least as sensitive for you, and steadier'}.</>}
      </p>
    </>
  );
}

export interface ModelNums { nights: number; pct_per_100: number; half_life_days: number; sigma_eta: number; sigma_eps: number; lr: number }
export function ModelNote({ m }: { m: ModelNums | null }) {
  return (
    <>
      <h2>Model</h2>
      <p>Rolling means and bands treat every night alike. A state-space model separates the slow level of your HRV from the effect of yesterday&apos;s training and from night-to-night noise, and turns each night into a surprise score. The impulse-response idea (training load pushes HRV down, the effect decays) is old: Banister&apos;s model has been fitted to nightly HF power in swimmers (<Ref k="chalencon2012">Chalencon 2012</Ref>), to daily lnRMSSD in rugby players (<Ref k="williams2018">Williams 2018</Ref>) and to HRV4Training readings in swimmers (<Ref k="piatrikova2021">Piatrikova 2021</Ref>), and a Kalman filter has been used to update a fitness-fatigue model (<Ref k="kolossa2017">Kolossa 2017</Ref>). Combining the two for nightly HRV is this notebook&apos;s own construction:</p>
      <div className="formula">{'y(t) = level(t) + beta * g(t) + noise(t)\nlevel(t) = level(t-1) + step(t)\ng(t) = exp(-1/tau) * g(t-1) + load(t-1) / 100'}</div>
      <p>y is ln rMSSD, level a slowly wandering level (random walk, step SD sigma_eta), load(t-1) the previous day&apos;s training load and noise the night&apos;s own noise (SD sigma_eps). A Kalman filter gives each night&apos;s prediction before it is seen, and the parameters maximise the likelihood of those predictions. The predictive band is an adaptive reference range in the sense of <Ref k="roshan2021">Roshan 2021</Ref>: it widens when nights are missing and narrows as evidence accumulates.</p>
      {m && (
        <p>
          Fitted on {big(m.nights)} nights: each 100 load units cost {num(Math.abs(m.pct_per_100))} % of the next night&apos;s HRV, {m.half_life_days < 0.5 ? 'and the effect is confined to that night (the fitted decay is under half a day, matching the second-night column in Training response)' : `and the effect decays with a half-life of ${num(m.half_life_days)} days`}.
          The level drifts {num(m.sigma_eta * 100)} % per night and a single night scatters {num(m.sigma_eps * 100, 0)} % around it. The load term improves the log-likelihood by {num(m.lr)} over a level alone (a likelihood-ratio statistic on 2 degrees of freedom; above 6 is significant at 5 %).
        </p>
      )}
    </>
  );
}

export function A1Note() {
  return (
    <>
      <h2>Thresholds from DFA {A1}</h2>
      <p>During exercise, the beat-to-beat series itself says how hard you are working. Detrended fluctuation analysis measures how strongly each interval is correlated with the ones before it: integrate the series, cut it into boxes of n beats, remove a straight line from each box and measure the leftover fluctuation F(n). Over short boxes (n = 4 to 16 beats) F(n) is proportional to n^{A1} (<Ref k="peng1995">Peng 1995</Ref>).</p>
      <p>At easy intensity the rhythm is fractal and {A1} sits around 1. As intensity rises it falls through 0.75 near the first ventilatory threshold (aerobic threshold) and reaches 0.5, uncorrelated noise, near the second (anaerobic) threshold (<Ref k="gronwald2020">Gronwald 2020</Ref>, <Ref k="rogers2021aet">Rogers 2021 aerobic</Ref>, <Ref k="rogers2021ant">Rogers 2021 anaerobic</Ref>, <Ref k="rogers2022">Rogers 2022 update</Ref>). In the validation studies the heart rate at {A1} = 0.75 matched VT1 within a few beats; at 0.5 the agreement with VT2 was looser (<Ref k="schaffarczyk2023">Schaffarczyk 2023</Ref>).</p>
      <p>How it is computed here, following Rogers&apos; method:</p>
      <ul>
        <li><b>Windows</b>: 2 minutes of beats, every 30 seconds through every chest-strap recording.</li>
        <li><b>Artefacts</b>: beats are checked against thresholds of 5.2 quartile deviations of the successive differences and of the distance from an 11-beat median (<Ref k="lipponen2019">Lipponen &amp; Tarvainen 2019</Ref>, simplified); missed beats are split, extra beats merged, ectopic beats replaced. Windows with more than 5 % corrected beats are dropped: up to about 6 % shifts {A1} but moves the threshold heart rate by under a beat (<Ref k="rogers2021sensors">Rogers 2021 Sensors</Ref>).</li>
        <li><b>Device</b>: some recordings carry RR intervals that the watch interpolated rather than measured (most consecutive intervals identical, typical of an optical sensor); they read as {A1} near 1.5 at any heart rate, so windows where more than 30 % of intervals repeat are dropped.</li>
        <li><b>Fresh only</b>: thresholds use each activity&apos;s first hour, because fatigue lowers {A1} at a given heart rate and breaks the link with VT1 (<Ref k="vanhooren2025">Van Hooren 2025</Ref>).</li>
        <li><b>Thresholds</b> come from all fresh windows of one sport over six weeks: median {A1} in 2-bpm bins, a weighted straight line through the transition zone (0.35 to 1.1), and the heart rates where it crosses 0.75 and 0.5. Day-to-day, {A1} at a fixed effort varies by about 0.06 when fresh (<Ref k="vanhooren2023">Van Hooren 2023</Ref>).</li>
      </ul>
    </>
  );
}

export function DurabilityNote() {
  return (
    <>
      <h2>Durability</h2>
      <p>Late in a long effort {A1} drops at the same pace while heart rate barely moves: after a 6-hour run it fell from 0.71 to 0.32 at a fixed 3 m/s (<Ref k="rogers2021ultra">Rogers 2021 ultra</Ref>), and the size of that drift is repeatable from session to session (<Ref k="rogers2025">Rogers 2025</Ref>). Without a fixed pace, the comparison here is at matched heart rate: each half hour of every activity of two hours or more is compared with that activity&apos;s own first hour. Matched heart rate is not matched pace (cardiac drift raises heart rate at a given pace, and long days include easy terrain), so read the curve as a description of your long days rather than a fatigue test.</p>
    </>
  );
}

export function RrNote() {
  return (
    <>
      <h2>From heartbeats to RMSSD</h2>
      <p>Every heartbeat is an R peak on the ECG. The time between two consecutive peaks is an <b>RR interval</b>, a few hundred to about a thousand milliseconds. Heart rate is just its reciprocal (60 000 / RR beats per minute); variability is how much consecutive intervals differ from each other.</p>
      <p>Garmin&apos;s HRV number is <b>RMSSD</b>, the root mean square of successive differences:</p>
      <div className="formula">{'RMSSD = sqrt( 1/(N-1) * sum over i of (RR(i+1) - RR(i))^2 )'}</div>
      <p>It only looks at beat-to-beat changes, so it tracks the fast, breath-linked swings that the vagus nerve drives and ignores slow drifts in heart rate. That makes it the standard short-window marker of parasympathetic (rest-and-digest) activity.</p>
      <p>The watch records RR intervals during activities when HRV logging is on. Overnight it computes RMSSD on the watch and only exports the 5-minute results, so this section works from activity recordings; a calm one (yoga, sauna, an ice bath) is closest to rest.</p>
    </>
  );
}

export function CleaningNote({ counts, total, rep }: { counts: Record<string, number>; total: number; rep: number }) {
  const kept = counts.kept ?? 0;
  return (
    <>
      <h2>Cleaning the beat stream</h2>
      <p>RMSSD squares every difference, so a single missed or doubled beat (an RR of 1,400 ms between two of 700 ms) can swamp a whole window. Before computing anything, each interval is checked:</p>
      <ul>
        <li><b>Out of range</b>: shorter than 300 ms (over 200 bpm) or longer than 2,000 ms (under 30 bpm) cannot be a real interval. {big(counts['out of range'] ?? 0)} here.</li>
        <li><b>Artifact</b>: more than 20% away from the median of its 11 neighbours, the classic signature of a missed beat, an extra beat, or an ectopic beat. {big(counts.artifact ?? 0)} here.</li>
        <li><b>Smoothing</b>: a true RR series almost never repeats the same value twice in a row. Here {num(rep * 100)}% of consecutive intervals are identical. Above roughly 8% the stream looks interpolated (typical of the wrist optical sensor when it loses the beat), which flattens the differences and understates RMSSD; compare a &quot;smoothed&quot; recording in the picker.</li>
      </ul>
      <p>That leaves <b>{big(kept)} of {big(total)} intervals</b> ({num((kept / Math.max(total, 1)) * 100)}%). Differences are only taken between two consecutive kept beats, so a rejected beat removes two differences rather than inventing one.</p>
    </>
  );
}

export function WindowNote() {
  return (
    <>
      <h2>One minute, step by step</h2>
      <p>The window opens on the most variable minute of the recording; slide it anywhere. The three panels are the three steps of the formula: the intervals themselves, the difference between each one and the next, and that difference squared. RMSSD is the square root of the mean of the bottom panel.</p>
    </>
  );
}

export function WorkedNote({ usable, meanSq, winRmssd, sessionRmssd, hr }: { usable: number; meanSq: number; winRmssd: number; sessionRmssd: number; hr: number }) {
  return (
    <p>
      In numbers, for this window: {usable} usable differences, mean squared difference <b>{big(Math.round(meanSq))} ms&sup2;</b>, square root <b>{num(winRmssd)} ms</b>. Over the whole recording, every pair of consecutive kept beats gives <b>RMSSD = {num(sessionRmssd)} ms</b> at an average heart rate of {num(hr, 0)} bpm. The table shows the first beats of the window the way you would work them by hand.
    </p>
  );
}

export function PoincareNote() {
  return (
    <>
      <h2>Poincare plot and spectrum</h2>
      <p>Plot each interval against the next and the cloud&apos;s width across the identity line, SD1, is the beat-to-beat variability: SD1 is exactly RMSSD divided by root 2. Its length along the line, SD2, adds the slower swings. Both are linear summaries of the same intervals, not nonlinear measures as was once thought (<Ref k="brennan2001">Brennan 2001</Ref>).</p>
      <p>The spectrum splits the variability by frequency: the high-frequency band (0.15 to 0.4 Hz) is breathing-linked and vagal, the low-frequency band (0.04 to 0.15 Hz) mixes vagal and sympathetic influences and baroreflex rhythms (<Ref k="taskforce1996">Task Force 1996</Ref>, <Ref k="berntson1997">Berntson 1997</Ref>). It is computed with a Lomb-Scargle periodogram, which handles the unevenly spaced beats directly. During exercise breathing speeds up, so the breathing peak can move above 0.4 Hz; that is one reason the exercise analysis uses DFA {A1} rather than spectral bands.</p>
    </>
  );
}

