import { useState } from 'react';
import { AboutDialog, SettingsDialog } from './components/Dialogs';
import { TitleBar, type Tab } from './components/TitleBar';
import { DayPage } from './pages/DayPage';
import { ReportsPage } from './pages/ReportsPage';
import { StatsPage } from './pages/StatsPage';
import { useTheme } from './theme';

export default function App() {
  const [tab, setTab] = useState<Tab>('day');
  const [dialog, setDialog] = useState<'settings' | 'about' | null>(null);
  const { theme, toggle } = useTheme();

  return (
    <div className="app">
      <TitleBar
        tab={tab}
        onTab={setTab}
        onSettings={() => setDialog('settings')}
        onAbout={() => setDialog('about')}
        theme={theme}
        onToggleTheme={toggle}
      />
      <main className="content">
        {tab === 'day' && <DayPage />}
        {tab === 'stats' && <StatsPage />}
        {tab === 'reports' && <ReportsPage />}
      </main>
      {dialog === 'settings' && <SettingsDialog onClose={() => setDialog(null)} />}
      {dialog === 'about' && <AboutDialog onClose={() => setDialog(null)} />}
    </div>
  );
}
