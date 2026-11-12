import { DEFAULT_SETTINGS, type ExperimentDef, type Label, type LabelKind, type Proposal, type Settings, type State } from '@wl/schema';
import { kindInfo } from '@wl/schema';
import { readFile, writeFile } from './opfs';

export type Answer = 'confirm' | 'deny' | 'unsure' | 'snooze';

/** Where labels, proposals and settings live: the browser (local mode) or the server (server mode). */
export interface StateStore {
  load(): Promise<State>;
  createLabel(l: Partial<Label> & Pick<Label, 'kind' | 'start_day' | 'end_day'>): Promise<Label>;
  updateLabel(l: Label): Promise<Label>;
  deleteLabel(id: string): Promise<void>;
  /** Confirm creates a label from the proposal (or `label`), deny and unsure only record the answer. Undo with `undo`. */
  answer(id: string, answer: Answer, label?: Partial<Label>): Promise<{ proposal: Proposal; label?: Label }>;
  undo(id: string, labelId?: string): Promise<void>;
  saveProposals(p: Proposal[]): Promise<void>;
  saveSettings(s: Settings): Promise<void>;
  saveExperiment(e: ExperimentDef): Promise<void>;
  deleteExperiment(id: string): Promise<void>;
  setAdherence(id: string, day: number, adherent: boolean): Promise<void>;
}

const now = () => new Date().toISOString();
const uid = () => (crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`);
const EMPTY = (): State => ({ labels: [], proposals: [], settings: { ...DEFAULT_SETTINGS }, experiments: [], adherence: {}, revision: 0 });

export const mergeProposals = (old: Proposal[], fresh: Proposal[]): Proposal[] => {
  // answered proposals keep their answer; open ones are replaced by the new detection
  const keep = old.filter((p) => p.status !== 'open');
  const ids = new Set(keep.map((p) => p.id));
  return [...keep, ...fresh.filter((p) => !ids.has(p.id))];
};

/** State as a JSON file in OPFS (`lab/state.json`). The semantics of `answer` mirror the server's. */
export class LocalStateStore implements StateStore {
  private s: State = EMPTY();
  private loaded = false;
  constructor(
    private read: (p: string[]) => Promise<ArrayBuffer | null> = readFile,
    private write: (p: string[], d: string) => Promise<void> = writeFile,
  ) {}
  private path = ['lab', 'state.json'];
  private async save() {
    this.s.revision++;
    await this.write(this.path, JSON.stringify(this.s));
  }
  async load(): Promise<State> {
    if (!this.loaded) {
      const b = await this.read(this.path);
      if (b) this.s = { ...EMPTY(), ...(JSON.parse(new TextDecoder().decode(b)) as Partial<State>) };
      this.s.settings = { ...DEFAULT_SETTINGS, ...this.s.settings };
      this.loaded = true;
    }
    return structuredClone(this.s);
  }
  async createLabel(l: Partial<Label> & Pick<Label, 'kind' | 'start_day' | 'end_day'>): Promise<Label> {
    await this.load();
    const label: Label = {
      id: uid(), note: '', exclude_baseline: kindInfo(l.kind).exclude, source: 'user', created_at: now(), updated_at: now(), ...l,
    } as Label;
    this.s.labels.push(label);
    await this.save();
    return label;
  }
  async updateLabel(l: Label): Promise<Label> {
    await this.load();
    const i = this.s.labels.findIndex((x) => x.id === l.id);
    if (i < 0) throw new Error('no such label');
    this.s.labels[i] = { ...l, updated_at: now() };
    await this.save();
    return this.s.labels[i]!;
  }
  async deleteLabel(id: string): Promise<void> {
    await this.load();
    const l = this.s.labels.find((x) => x.id === id);
    if (l) {
      l.deleted_at = now();
      await this.save();
    }
  }
  async answer(id: string, answer: Answer, label?: Partial<Label>) {
    await this.load();
    const p = this.s.proposals.find((x) => x.id === id);
    if (!p) throw new Error('no such proposal');
    p.answered_at = now();
    let created: Label | undefined;
    if (answer === 'confirm') {
      p.status = 'confirmed';
      const kind: LabelKind = label?.kind ?? p.kind;
      created = {
        id: uid(), kind, start_day: p.start_day, end_day: p.end_day, onset_day: null, severity: null, note: '',
        exclude_baseline: kindInfo(kind).exclude, source: p.auto ? 'inferred' : 'confirmed', proposal_id: p.id,
        created_at: now(), updated_at: now(), ...label,
      } as Label;
      this.s.labels.push(created);
    } else if (answer === 'deny') p.status = 'denied';
    else if (answer === 'unsure') p.status = 'unsure';
    else {
      p.status = 'snoozed';
      p.snoozed_until = new Date(Date.now() + 7 * 86_400_000).toISOString();
    }
    await this.save();
    return { proposal: structuredClone(p), label: created };
  }
  async undo(id: string, labelId?: string): Promise<void> {
    await this.load();
    const p = this.s.proposals.find((x) => x.id === id);
    if (p) {
      p.status = 'open';
      p.answered_at = null;
      p.snoozed_until = null;
    }
    if (labelId) this.s.labels = this.s.labels.filter((l) => l.id !== labelId);
    await this.save();
  }
  async saveProposals(p: Proposal[]): Promise<void> {
    await this.load();
    this.s.proposals = mergeProposals(this.s.proposals, p);
    await this.save();
  }
  async saveSettings(s: Settings): Promise<void> {
    await this.load();
    this.s.settings = s;
    await this.save();
  }
  async saveExperiment(e: ExperimentDef): Promise<void> {
    await this.load();
    const i = this.s.experiments.findIndex((x) => x.id === e.id);
    if (i >= 0) this.s.experiments[i] = e;
    else this.s.experiments.push(e);
    await this.save();
  }
  async deleteExperiment(id: string): Promise<void> {
    await this.load();
    this.s.experiments = this.s.experiments.filter((e) => e.id !== id);
    delete this.s.adherence[id];
    await this.save();
  }
  async setAdherence(id: string, day: number, adherent: boolean): Promise<void> {
    await this.load();
    (this.s.adherence[id] ??= {})[day] = adherent;
    await this.save();
  }
}

/** The server's REST state (server mode). */
export class ServerStateStore implements StateStore {
  constructor(private base = '/api') {}
  private async j<T>(path: string, init?: RequestInit): Promise<T> {
    const r = await fetch(this.base + path, { credentials: 'same-origin', headers: { 'content-type': 'application/json' }, ...init });
    if (r.status === 401) throw Object.assign(new Error('unauthorized'), { status: 401 });
    if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
    return r.status === 204 ? (undefined as T) : ((await r.json()) as T);
  }
  load = () => this.j<State>('/state');
  createLabel = (l: Partial<Label>) => this.j<Label>('/labels', { method: 'POST', body: JSON.stringify(l) });
  updateLabel = (l: Label) => this.j<Label>(`/labels/${encodeURIComponent(l.id)}`, { method: 'PUT', body: JSON.stringify(l) });
  deleteLabel = (id: string) => this.j<void>(`/labels/${encodeURIComponent(id)}`, { method: 'DELETE' });
  answer = (id: string, answer: Answer, label?: Partial<Label>) =>
    this.j<{ proposal: Proposal; label?: Label }>(`/proposals/${encodeURIComponent(id)}/answer`, { method: 'POST', body: JSON.stringify({ answer, label }) });
  undo = (id: string, labelId?: string) => this.j<void>(`/proposals/${encodeURIComponent(id)}/undo`, { method: 'POST', body: JSON.stringify({ label_id: labelId }) });
  saveProposals = async () => {}; // the server detects and stores them itself
  saveSettings = (s: Settings) => this.j<void>('/settings', { method: 'PUT', body: JSON.stringify(s) });
  async saveExperiment(e: ExperimentDef): Promise<void> {
    try {
      await this.j<void>(`/experiments/${encodeURIComponent(e.id)}`, { method: 'PUT', body: JSON.stringify(e) });
    } catch {
      await this.j<void>('/experiments', { method: 'POST', body: JSON.stringify(e) });
    }
  }
  deleteExperiment = (id: string) => this.j<void>(`/experiments/${encodeURIComponent(id)}`, { method: 'DELETE' });
  setAdherence = (id: string, day: number, adherent: boolean) => this.j<void>(`/experiments/${encodeURIComponent(id)}/adherence/${day}`, { method: 'PUT', body: JSON.stringify({ adherent }) });
}
