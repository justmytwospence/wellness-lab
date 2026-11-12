import { lazy, type ComponentType, type LazyExoticComponent } from 'react';
import { Activity, BarChart3, BedDouble, Beaker, Brain, Calendar, Database, FlaskConical, GitCompare, Home, Inbox, LineChart, Moon, Radar, Search, Settings, Sun, Target, Terminal, Waves, BookOpen, HeartPulse } from 'lucide-react';

export interface RouteDef { path: string; match: RegExp; name: string; icon: ComponentType<{ size?: number }>; key: string; view: LazyExoticComponent<ComponentType>; rail: boolean; mobile?: boolean }

const r = (path: string, name: string, icon: RouteDef['icon'], key: string, view: RouteDef['view'], rail = true, mobile = false): RouteDef => ({
  path, name, icon, key, view, rail, mobile, match: new RegExp('^' + path.replace(/:[a-z]+/g, '[^/]+') + '/?$'),
});

export const ROUTES: RouteDef[] = [
  r('/', 'Today', Home, 't', lazy(() => import('./views/Today')), true, true),
  r('/night/:day', 'Night', Moon, 'n', lazy(() => import('./views/Night')), false),
  r('/nights', 'Nights', BedDouble, 'r', lazy(() => import('./views/Nights')), true, true),
  r('/typical', 'Typical', Waves, 'y', lazy(() => import('./views/Typical'))),
  r('/trends', 'Trends', LineChart, 'd', lazy(() => import('./views/Trends')), true, true),
  r('/hrv', 'HRV', HeartPulse, 'h', lazy(() => import('./views/Hrv'))),
  r('/thresholds', 'Thresholds', Target, 'x', lazy(() => import('./views/Thresholds'))),
  r('/beats', 'Beats', Activity, 'b', lazy(() => import('./views/Beats'))),
  r('/circadian', 'Circadian', Sun, 'c', lazy(() => import('./views/Circadian'))),
  r('/inbox', 'Inbox', Inbox, 'i', lazy(() => import('./views/Inbox')), true, true),
  r('/events', 'Events', Calendar, 'e', lazy(() => import('./views/Events'))),
  r('/compare', 'Compare', GitCompare, 'm', lazy(() => import('./views/Compare'))),
  r('/bench', 'Bench', Beaker, 'k', lazy(() => import('./views/Bench'))),
  r('/models', 'Models', Brain, 'o', lazy(() => import('./views/Models'))),
  r('/garmin', 'Garmin', Radar, 'a', lazy(() => import('./views/Garmin'))),
  r('/experiments', 'Experiments', FlaskConical, 'p', lazy(() => import('./views/Experiments'))),
  r('/sql', 'SQL', Terminal, 's', lazy(() => import('./views/Sql'))),
  r('/methods', 'Methods', BookOpen, 'f', lazy(() => import('./views/Methods'))),
  r('/data', 'Data', Database, 'u', lazy(() => import('./views/Data'))),
  r('/settings', 'Settings', Settings, ',', lazy(() => import('./views/Settings'))),
];
void BarChart3; void Search;

export const routeFor = (path: string) => ROUTES.find((x) => x.match.test(path));
