import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ReactNode } from 'react';
import { t } from '../i18n';
import type { Theme } from '../theme';
import { IconClose, IconDay, IconGear, IconInfo, IconMax, IconMin, IconMoon, IconReports, IconStats, IconSun } from './Icons';

export type Tab = 'day' | 'stats' | 'reports';

interface Props {
  tab: Tab;
  onTab: (t: Tab) => void;
  onSettings: () => void;
  onAbout: () => void;
  theme: Theme;
  onToggleTheme: () => void;
}

const win = () => getCurrentWindow();

export function TitleBar({ tab, onTab, onSettings, onAbout, theme, onToggleTheme }: Props) {
  const tabs: { id: Tab; label: string; icon: ReactNode }[] = [
    { id: 'day', label: t.tabDay, icon: <IconDay /> },
    { id: 'stats', label: t.tabStats, icon: <IconStats /> },
    { id: 'reports', label: t.tabReports, icon: <IconReports /> },
  ];
  return (
    <header className="titlebar" data-tauri-drag-region>
      <nav className="tabs">
        {tabs.map((x) => (
          <button key={x.id} className={`tab ${tab === x.id ? 'active' : ''}`} onClick={() => onTab(x.id)}>
            {x.icon}
            {x.label}
          </button>
        ))}
      </nav>
      <div className="title-spacer" data-tauri-drag-region />
      <button
        className="icon-btn"
        title={theme === 'dark' ? t.themeToLight : t.themeToDark}
        onClick={onToggleTheme}
      >
        {theme === 'dark' ? <IconSun /> : <IconMoon />}
      </button>
      <button className="icon-btn" title={t.settings} onClick={onSettings}>
        <IconGear />
      </button>
      <button className="icon-btn" title={t.about} onClick={onAbout}>
        <IconInfo />
      </button>
      <span className="sep" />
      <button className="icon-btn win" title={t.minimize} onClick={() => win().minimize()}>
        <IconMin />
      </button>
      <button className="icon-btn win" title={t.maximize} onClick={() => win().toggleMaximize()}>
        <IconMax />
      </button>
      <button className="icon-btn win close" title={t.close} onClick={() => win().close()}>
        <IconClose />
      </button>
    </header>
  );
}
