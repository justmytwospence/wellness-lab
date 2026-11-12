import { isoDay, parseDay } from './format';

export type Range = [number, number];
const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

const monthRange = (y: number, m: number): Range => [parseDay(`${y}-${String(m + 1).padStart(2, '0')}-01`), Math.floor(Date.UTC(y, m + 1, 0) / 86_400_000)];

/**
 * "last 90 days", "90d", "2025", "summer 2024" (Jun 21 to Sep 22), "2024-03 to 2024-06", "Mar 2024", "since 2023-05-01",
 * "2024-03-01 to 2024-04-15", "all". `today` is a day number. Returns null when it cannot read the text.
 */
export function parseRange(text: string, today: number, first = today - 3650): Range | null {
  const s = text.trim().toLowerCase();
  let m: RegExpMatchArray | null;
  if (!s) return null;
  if (s === 'all' || s === 'everything') return [first, today];
  if ((m = s.match(/^(?:last\s+)?(\d+)\s*(d|days?|w|weeks?|m|months?|y|years?)$/))) {
    const n = +m[1]!;
    const u = m[2]![0]!;
    const days = u === 'd' ? n : u === 'w' ? n * 7 : u === 'm' ? n * 30 : n * 365;
    return [today - days + 1, today];
  }
  if ((m = s.match(/^since\s+(\d{4}-\d{2}-\d{2})$/))) return [parseDay(m[1]!), today];
  if ((m = s.match(/^(\d{4})$/))) return [parseDay(`${m[1]}-01-01`), parseDay(`${m[1]}-12-31`)];
  if ((m = s.match(/^(spring|summer|autumn|fall|winter)\s+(\d{4})$/))) {
    const y = +m[2]!;
    const season = m[1]!;
    if (season === 'summer') return [parseDay(`${y}-06-21`), parseDay(`${y}-09-22`)];
    if (season === 'spring') return [parseDay(`${y}-03-20`), parseDay(`${y}-06-20`)];
    if (season === 'winter') return [parseDay(`${y}-12-21`), parseDay(`${y + 1}-03-19`)];
    return [parseDay(`${y}-09-23`), parseDay(`${y}-12-20`)];
  }
  if ((m = s.match(/^([a-z]{3})[a-z]*\s+(\d{4})$/)) && MONTHS.includes(m[1]!)) return monthRange(+m[2]!, MONTHS.indexOf(m[1]!));
  if ((m = s.match(/^(\d{4})-(\d{2})$/))) return monthRange(+m[1]!, +m[2]! - 1);
  const parts = s.split(/\s+(?:to|-|\u2013)\s+/);
  if (parts.length === 2) {
    const a = parseRange(parts[0]!, today, first), b = parseRange(parts[1]!, today, first);
    if (a && b) return [a[0], b[1]];
  }
  if ((m = s.match(/^(\d{4}-\d{2}-\d{2})$/))) return [parseDay(m[1]!), parseDay(m[1]!)];
  return null;
}

export const formatRange = (r: Range): string => (r[0] === r[1] ? isoDay(r[0]) : `${isoDay(r[0])} to ${isoDay(r[1])}`);
