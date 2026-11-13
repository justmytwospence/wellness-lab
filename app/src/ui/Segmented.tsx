export function Segmented<T extends string>({ label, value, options, onChange, role = 'group' }: { label: string; value: T; options: [T, string][]; onChange(v: T): void; role?: 'group' | 'tablist' | 'radiogroup' }) {
  const item = role === 'tablist' ? 'tab' : role === 'radiogroup' ? 'radio' : undefined;
  return (
    <div className="seg" role={role} aria-label={label}>
      {options.map(([v, t]) => (
        <button
          key={v}
          className={v === value ? 'on' : ''}
          role={item}
          {...(item === 'tab' ? { 'aria-selected': v === value } : item === 'radio' ? { 'aria-checked': v === value } : { 'aria-pressed': v === value })}
          onClick={() => onChange(v)}
        >{t}</button>
      ))}
    </div>
  );
}
