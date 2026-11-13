import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { DEFAULT_SETTINGS, type Settings as S } from '@wl/schema';
import { useApp } from '../lib/app';
import { useH12, useTheme } from '../lib/prefs';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';

const SECTIONS: [string, string][] = [['display', 'Display'], ['questions', 'Questions in the Inbox'], ['baselines', 'Baselines and sleep'], ['notifications', 'Notifications']];
const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

function Field({ id, label, help, children }: { id: string; label: string; help: ReactNode; children: ReactNode }) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'minmax(180px, 260px) 1fr', gap: '4px 18px', padding: '10px 0', borderBottom: '1px solid var(--grid)' }}>
      <label htmlFor={id} style={{ fontWeight: 500 }}>{label}</label>
      <div>
        <div>{children}</div>
        <p className="muted" style={{ margin: '4px 0 0', fontSize: 12.5 }}>{help}</p>
      </div>
    </div>
  );
}

/** A number box that may be empty (null). */
function NumBox({ id, value, onChange, min, max, step, placeholder, unit }: { id: string; value: number | null; onChange(v: number | null): void; min?: number; max?: number; step?: number; placeholder?: string; unit?: string }) {
  return (
    <span className="row" style={{ gap: 6, display: 'inline-flex' }}>
      <input id={id} type="number" className="mono" style={{ width: 110 }} value={value ?? ''} min={min} max={max} step={step} placeholder={placeholder}
        onChange={(e) => onChange(e.target.value === '' ? null : Number(e.target.value))} />
      {unit && <span className="muted">{unit}</span>}
    </span>
  );
}

export default function Settings() {
  const { state, saveSettings, mode, demo, toast } = useApp();
  const [theme, setTheme] = useTheme();
  const [h12, setH12] = useH12();
  const [draft, setDraft] = useState<S>(state.settings);
  const [testing, setTesting] = useState(false);
  const [testOut, setTestOut] = useState<string[] | null>(null);
  // follow the stored settings when they change underneath (load, server revision), not while editing
  const stored = JSON.stringify(state.settings);
  useEffect(() => setDraft(state.settings), [stored]); // eslint-disable-line react-hooks/exhaustive-deps
  const dirty = JSON.stringify(draft) !== stored;
  const set = <K extends keyof S>(k: K, v: S[K]) => setDraft((d) => ({ ...d, [k]: v }));
  const invalid = useMemo(() => {
    if (draft.ask_min > draft.ask_max) return 'The lowest probability to ask at cannot exceed the highest.';
    if (draft.baseline_window < 7) return 'The baseline window needs at least 7 nights.';
    if (draft.hrv_range_days < 14) return 'The HRV range needs at least 14 nights.';
    return null;
  }, [draft]);

  const save = async () => {
    try {
      await saveSettings(draft);
      toast('Settings saved');
    } catch (e) {
      toast(`Could not save settings: ${(e as Error).message}`);
    }
  };

  const sendTest = async () => {
    setTesting(true);
    setTestOut(null);
    try {
      const r = await fetch('/api/notify/test', { method: 'POST', credentials: 'same-origin' });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const j = (await r.json()) as Record<string, { ok: boolean; error?: string }>;
      const names = Object.keys(j);
      setTestOut(names.length ? names.map((n) => `${n}: ${j[n]!.ok ? 'sent' : `failed (${j[n]!.error ?? 'unknown error'})`}`) : ['No channel is configured on the server: set NTFY_URL or NOTIFY_WEBHOOK_URL.']);
    } catch (e) {
      setTestOut([`Could not send: ${(e as Error).message}`]);
    } finally {
      setTesting(false);
    }
  };

  const doc = useRef<HTMLDivElement>(null);
  const go = (id: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    doc.current?.querySelector(`#sec-${id}`)?.scrollIntoView({ block: 'start' });
  };

  return (
    <View title="Settings" desc="Analysis settings are stored with your labels.">
      <div className="vgrid" style={{ gridTemplateColumns: '200px minmax(0,1fr)' }}>
        <Panel testid="settings-nav">
          <nav className="vnav" aria-label="Sections">
            {SECTIONS.map(([id, t]) => <a key={id} href={`#sec-${id}`} onClick={go(id)}>{t}</a>)}
          </nav>
        </Panel>
        <Panel scroll testid="settings-form">
          <div ref={doc} style={{ maxWidth: 900 }}>
            <p className="muted" style={{ marginTop: 0 }}>
              Stored {mode === 'server' ? 'on the server' : 'in this browser'}; they change the numbers the engine computes.
              {demo && ' Demo data is showing: saved settings are real, the demo data is not.'}
            </p>
        <section className="method" id="sec-display">
        <h2 style={{ margin: '0 0 4px', fontSize: 14 }}>Display</h2>
        <Field id="theme" label="Theme" help="Dark is the default. This choice stays on this device.">
          <select id="theme" value={theme} onChange={(e) => setTheme(e.target.value as 'dark' | 'light')}>
            <option value="dark">Dark</option>
            <option value="light">Light</option>
          </select>
        </Field>
        <Field id="h12" label="Clock" help="How times of day are written, for example 22:30 or 10:30 pm. This choice stays on this device.">
          <select id="h12" value={h12 ? '12' : '24'} onChange={(e) => setH12(e.target.value === '12')}>
            <option value="24">24 hour</option>
            <option value="12">12 hour</option>
          </select>
        </Field>
        </section>

        <section className="method" id="sec-questions">
        <h2 style={{ margin: '0 0 4px', fontSize: 14 }}>Questions in the Inbox</h2>
        <Field id="question_budget_per_week" label="Questions per week" help="The most questions the Inbox asks (and a notification mentions) in any 7 days. A likely illness may add one more. 0 turns asking off.">
          <NumBox id="question_budget_per_week" value={draft.question_budget_per_week} min={0} max={14} step={1} onChange={(v) => set('question_budget_per_week', Math.max(0, Math.round(v ?? 0)))} />
        </Field>
        <Field id="ask_min" label="Ask from probability" help="Only ask about an event whose most likely explanation has at least this probability. Lower asks more, including weaker guesses.">
          <NumBox id="ask_min" value={draft.ask_min} min={0} max={1} step={0.05} onChange={(v) => set('ask_min', v ?? 0)} />
        </Field>
        <Field id="ask_max" label="Label by itself above" help="Above this probability the explanation is treated as certain: the event is labelled on its own, shown as inferred with a one-click Undo, and not asked about.">
          <NumBox id="ask_max" value={draft.ask_max} min={0} max={1} step={0.05} onChange={(v) => set('ask_max', v ?? 1)} />
        </Field>
        <Field id="auto_label_travel" label="Label time zone changes" help="Add a Travel label by itself when the time zone shifts. Every automatic label has a one-click Undo.">
          <input id="auto_label_travel" type="checkbox" checked={draft.auto_label_travel} onChange={(e) => set('auto_label_travel', e.target.checked)} />
        </Field>
        <Field id="auto_label_altitude" label="Label altitude stays" help="Add an Altitude label by itself for runs of days at least 700 m above your home altitude (and above 1500 m).">
          <input id="auto_label_altitude" type="checkbox" checked={draft.auto_label_altitude} onChange={(e) => set('auto_label_altitude', e.target.checked)} />
        </Field>
        <Field id="home_altitude_m" label="Home altitude" help="Metres above sea level where you usually sleep. Leave empty to use the median of your last 90 nights.">
          <NumBox id="home_altitude_m" value={draft.home_altitude_m} min={-100} max={5000} step={50} unit="m" placeholder="auto" onChange={(v) => set('home_altitude_m', v)} />
        </Field>
        <Field id="cycle_tracking" label="Cycle tracking" help="Turn on if you track a menstrual cycle and want to log it as an event kind. Detection does not use it yet.">
          <input id="cycle_tracking" type="checkbox" checked={draft.cycle_tracking} onChange={(e) => set('cycle_tracking', e.target.checked)} />
        </Field>
        </section>

        <section className="method" id="sec-baselines">
        <h2 style={{ margin: '0 0 4px', fontSize: 14 }}>Baselines and sleep</h2>
        <Field id="baseline_window" label="Baseline window" help="Nights behind each personal baseline (a robust median and spread). Shorter follows you faster and is noisier; the default is 28.">
          <NumBox id="baseline_window" value={draft.baseline_window} min={7} max={120} step={1} unit="nights" onChange={(v) => set('baseline_window', Math.round(v ?? DEFAULT_SETTINGS.baseline_window))} />
        </Field>
        <Field id="hrv_range_days" label="HRV range" help="Nights behind the HRV normal range and trend on the HRV page. The default is 60.">
          <NumBox id="hrv_range_days" value={draft.hrv_range_days} min={14} max={365} step={1} unit="nights" onChange={(v) => set('hrv_range_days', Math.round(v ?? DEFAULT_SETTINGS.hrv_range_days))} />
        </Field>
        <Field id="k_daily" label="Daily band width" help="How many spreads either side of your baseline count as a normal day on the HRV page. Larger is more forgiving; the default is 1.5.">
          <NumBox id="k_daily" value={draft.k_daily} min={0.5} max={4} step={0.1} onChange={(v) => set('k_daily', v ?? DEFAULT_SETTINGS.k_daily)} />
        </Field>
        <Field id="k_swc" label="Smallest worthwhile change" help="The smallest change worth noticing, as a fraction of your spread. It sets the band around the trend and the effect size experiments aim at; the default is 0.5.">
          <NumBox id="k_swc" value={draft.k_swc} min={0.1} max={2} step={0.1} onChange={(v) => set('k_swc', v ?? DEFAULT_SETTINGS.k_swc)} />
        </Field>
        <Field id="sleep_need_h" label="Sleep need" help="Hours of sleep you need per night, for sleep debt. Leave empty to estimate it from your median sleep on free days over the last 90 nights.">
          <NumBox id="sleep_need_h" value={draft.sleep_need_h} min={4} max={12} step={0.25} unit="h" placeholder="auto" onChange={(v) => set('sleep_need_h', v)} />
        </Field>
        <Field id="tst_bias_min" label="Sleep time bias" help="How many minutes Garmin overestimates your total sleep time. It is taken off before sleep debt is computed; the default is 30.">
          <NumBox id="tst_bias_min" value={draft.tst_bias_min} min={0} max={90} step={5} unit="min" onChange={(v) => set('tst_bias_min', v ?? DEFAULT_SETTINGS.tst_bias_min)} />
        </Field>
        <Field id="free_days" label="Free days" help="The weekdays you wake without an alarm. They set social jet lag and chronotype, and mark nights as free-day nights.">
          <span className="row" style={{ gap: 6, display: 'inline-flex' }} role="group" aria-label="Free days">
            {DAYS.map((d, i) => {
              const on = draft.free_days.includes(i);
              return (
                <button key={d} type="button" aria-pressed={on} className={on ? 'primary' : ''}
                  onClick={() => set('free_days', on ? draft.free_days.filter((x) => x !== i) : [...draft.free_days, i].sort())}>{d}</button>
              );
            })}
          </span>
        </Field>
        </section>

        <section className="method" id="sec-notifications">
        <h2 style={{ margin: '0 0 4px', fontSize: 14 }}>Notifications</h2>
        <Field id="notify_ntfy" label="ntfy" help="Send new questions to the ntfy topic configured on the server (NTFY_URL).">
          <input id="notify_ntfy" type="checkbox" checked={draft.notify_ntfy} onChange={(e) => set('notify_ntfy', e.target.checked)} />
        </Field>
        <Field id="notify_webhook" label="Webhook" help="Post new questions as JSON to the webhook configured on the server (NOTIFY_WEBHOOK_URL).">
          <input id="notify_webhook" type="checkbox" checked={draft.notify_webhook} onChange={(e) => set('notify_webhook', e.target.checked)} />
        </Field>
        <Field id="notify_test" label="Test" help={mode === 'server' ? 'Sends one test message to every channel the server has configured, whatever the switches above say.' : 'Notifications are sent by the server, so this needs server mode. In local mode the Inbox is the only place questions appear.'}>
          <button id="notify_test" aria-label="Send test notification" disabled={mode !== 'server' || testing} onClick={sendTest}>{testing ? 'Sending...' : 'Send test notification'}</button>
          {testOut && <ul style={{ margin: '8px 0 0', paddingLeft: 18 }}>{testOut.map((l) => <li key={l} className={l.includes('failed') || l.startsWith('Could not') ? 'err' : ''}>{l}</li>)}</ul>}
        </Field>
        </section>

          </div>
          <div style={{ position: 'sticky', bottom: 0, background: 'var(--bg)', borderTop: '1px solid var(--line)', padding: '10px 0', marginTop: 8 }}>
            {invalid && <p className="err" style={{ margin: '0 0 8px' }}>{invalid}</p>}
            <div className="row">
              <button className="primary" disabled={!dirty || !!invalid} onClick={save}>Save settings</button>
              <button disabled={JSON.stringify(draft) === JSON.stringify(DEFAULT_SETTINGS)} onClick={() => setDraft({ ...DEFAULT_SETTINGS })}>Reset to defaults</button>
              {dirty && <span className="muted">Unsaved changes</span>}
            </div>
          </div>
        </Panel>
      </div>
    </View>
  );
}
