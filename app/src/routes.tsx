import { lazy, type ComponentType, type LazyExoticComponent } from 'react';
import { Activity, Database, FlaskConical, HeartPulse, Home, Inbox, Moon, Settings } from 'lucide-react';

export type WsId = 'today' | 'sleep' | 'recovery' | 'training' | 'journal' | 'lab' | 'data' | 'settings';
export interface Workspace { id: WsId; name: string; icon: ComponentType<{ size?: number }>; foot?: boolean }
export const WORKSPACES: Workspace[] = [
  { id: 'today', name: 'Today', icon: Home },
  { id: 'sleep', name: 'Sleep', icon: Moon },
  { id: 'recovery', name: 'Recovery', icon: HeartPulse },
  { id: 'training', name: 'Training', icon: Activity },
  { id: 'journal', name: 'Journal', icon: Inbox },
  { id: 'lab', name: 'Lab', icon: FlaskConical },
  { id: 'data', name: 'Data', icon: Database, foot: true },
  { id: 'settings', name: 'Settings', icon: Settings, foot: true },
];

export interface RouteDef { path: string; match: RegExp; name: string; key: string; ws: WsId; range: boolean; view: LazyExoticComponent<ComponentType> }

const r = (path: string, name: string, key: string, ws: WsId, range: boolean, view: RouteDef['view']): RouteDef => ({
  path, name, key, ws, range, view, match: new RegExp('^' + path.replace(/:[a-z]+/g, '[^/]+') + '/?$'),
});

export const ROUTES: RouteDef[] = [
  r('/', 'Today', 't', 'today', false, lazy(() => import('./views/Today'))),
  r('/nights', 'Nights', 'r', 'sleep', true, lazy(() => import('./views/Nights'))),
  r('/night/:day', 'Night', 'n', 'sleep', false, lazy(() => import('./views/Night'))),
  r('/typical', 'Typical', 'y', 'sleep', true, lazy(() => import('./views/Typical'))),
  r('/compare', 'Compare', 'm', 'sleep', true, lazy(() => import('./views/Compare'))),
  r('/hrv', 'HRV', 'h', 'recovery', true, lazy(() => import('./views/Hrv'))),
  r('/trends', 'Trends', 'd', 'recovery', true, lazy(() => import('./views/Trends'))),
  r('/circadian', 'Circadian', 'c', 'recovery', true, lazy(() => import('./views/Circadian'))),
  r('/models', 'Models', 'o', 'recovery', true, lazy(() => import('./views/Models'))),
  r('/thresholds', 'Thresholds', 'x', 'training', false, lazy(() => import('./views/Thresholds'))),
  r('/beats', 'Beats', 'b', 'training', false, lazy(() => import('./views/Beats'))),
  r('/inbox', 'Inbox', 'i', 'journal', false, lazy(() => import('./views/Inbox'))),
  r('/events', 'Events', 'e', 'journal', false, lazy(() => import('./views/Events'))),
  r('/experiments', 'Experiments', 'p', 'journal', false, lazy(() => import('./views/Experiments'))),
  r('/bench', 'Bench', 'k', 'lab', false, lazy(() => import('./views/Bench'))),
  r('/garmin', 'Garmin', 'a', 'lab', false, lazy(() => import('./views/Garmin'))),
  r('/sql', 'SQL', 's', 'lab', false, lazy(() => import('./views/Sql'))),
  r('/methods', 'Methods', 'f', 'lab', false, lazy(() => import('./views/Methods'))),
  r('/data', 'Data', 'u', 'data', false, lazy(() => import('./views/Data'))),
  r('/settings', 'Settings', ',', 'settings', false, lazy(() => import('./views/Settings'))),
];

export const routeFor = (path: string) => ROUTES.find((x) => x.match.test(path));
