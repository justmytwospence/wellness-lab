import { useEffect, useState } from 'react';
import { formatRange, parseRange } from '@wl/views';
import { useApp } from '../lib/app';
import { Segmented } from './Segmented';

const PRESETS: [string, string][] = [['30d', '30d'], ['90d', '90d'], ['180d', '6m'], ['1y', '1y'], ['all', 'All']];

export function RangePicker() {
  const app = useApp();
  const [text, setText] = useState(app.rangeText);
  useEffect(() => setText(app.rangeText), [app.rangeText]);
  const apply = () => {
    if (parseRange(text, app.lastDay, app.firstDay)) app.setRangeText(text);
    else app.toast(`Could not read "${text}" as a date range`);
  };
  return (
    <>
      <Segmented label="Date range presets" value={app.rangeText} options={PRESETS} onChange={(v) => app.setRangeText(v)} />
      <input className="mono" style={{ width: 150 }} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && apply()} onBlur={apply} aria-label="Date range" placeholder="2025, summer 2024, last 45 days" />
      <span className="muted mono range-text" style={{ fontSize: 12 }}>{formatRange(app.range)}</span>
    </>
  );
}
