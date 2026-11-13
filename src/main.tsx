import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
import { applyTheme, loadTheme } from './theme';

// Režim se nastaví ještě před prvním vykreslením, aby okno neprobliklo v jiné barvě.
applyTheme(loadTheme());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
