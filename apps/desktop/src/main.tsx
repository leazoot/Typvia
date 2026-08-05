import { followSystemTheme } from '@typvia/ui';
import '@typvia/ui/base.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router';
import { App } from './App';

followSystemTheme(document.documentElement);

const container = document.getElementById('root');
if (container === null) {
  throw new Error('missing #root element in index.html');
}

createRoot(container).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>,
);
