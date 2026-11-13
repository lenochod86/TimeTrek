import type { Icons } from '../hooks';
import { t } from '../i18n';
import { colorFor } from '../utils/color';
import { fmtDuration } from '../utils/time';

export interface AppRow {
  id: number;
  name: string;
  ms: number;
}

interface Props {
  rows: AppRow[];
  total: number;
  icons: Icons;
  checked: Set<number>;
  onToggle: (id: number) => void;
}

export function AppRanking({ rows, total, icons, checked, onToggle }: Props) {
  return (
    <div className="ranking">
      <div className="ranking-list">
        {rows.map((r) => {
          const pct = total > 0 ? (r.ms / total) * 100 : 0;
          return (
            <div key={r.id} className={`rank-row ${checked.has(r.id) ? 'checked' : ''}`} onClick={() => onToggle(r.id)}>
              <input type="checkbox" checked={checked.has(r.id)} onChange={() => onToggle(r.id)} onClick={(e) => e.stopPropagation()} />
              <span className="cell-icon">{icons.apps[r.id] ? <img src={icons.apps[r.id]} alt="" width={16} height={16} /> : <span className="icon-ph" />}</span>
              <span className="rank-name" title={r.name}>
                {r.name}
              </span>
              <span className="bar">
                <span className="bar-fill" style={{ width: `${Math.max(pct, 1)}%`, background: colorFor(`app:${r.id}`) }} />
                <span className="bar-text">{pct.toFixed(1)}%</span>
              </span>
              <span className="rank-time">{fmtDuration(r.ms)}</span>
            </div>
          );
        })}
      </div>
      <div className="ranking-total">
        {t.total}: <strong>{fmtDuration(total)}</strong>
      </div>
    </div>
  );
}
