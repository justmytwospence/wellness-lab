import { View } from '../../ui/View';
import { useSyncExternalStore, type ReactNode } from 'react';
import { Empty } from '../small';

const q = '(max-width: 767px)';
/** True below 768 px; follows the window. */
export const useNarrow = (): boolean =>
  useSyncExternalStore(
    (cb) => {
      const m = matchMedia(q);
      m.addEventListener('change', cb);
      return () => m.removeEventListener('change', cb);
    },
    () => matchMedia(q).matches,
  );

/** Views that need room: below 768 px they say so, with their address, instead of rendering. */
export function DesktopOnly({ title, children }: { title: string; children: ReactNode }) {
  const narrow = useNarrow();
  if (!narrow) return <>{children}</>;
  return (
    <View title={title}>
      <Empty>This view needs a larger screen. <div className="mono" style={{ marginTop: 6 }}>{location.pathname}{location.search}</div></Empty>
    </View>
  );
}
