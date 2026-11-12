import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './theme.css';
import { AppProvider } from './lib/app';
import { Shell } from './layout/Shell';

const theme = localStorage.getItem('wl.theme');
if (theme) document.documentElement.dataset.theme = theme;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <AppProvider>
      <Shell />
    </AppProvider>
  </StrictMode>,
);
