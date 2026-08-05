import { APP_ROUTES } from '@typvia/shared';
import { Page, PlaceholderPage, TopNav } from '@typvia/ui';
import { Route, Routes, useLocation, useNavigate } from 'react-router';
import { EditorPage } from './pages/editor';
import { HomePage } from './pages/home';
import { LibraryPage } from './pages/library';
import { TrashPage } from './pages/trash';

const NAV_ITEMS = APP_ROUTES.map((route) => ({ key: route.path, label: route.labelEn }));

// App shell: top text navigation (TopNav) over the routed pages. Page bodies
// are placeholders until their screens land (STAGE-08 on).
export function App() {
  const navigate = useNavigate();
  const location = useLocation();
  return (
    <>
      <TopNav
        items={NAV_ITEMS}
        activeKey={location.pathname}
        onNavigate={(path) => void navigate(path)}
      />
      <Routes>
        <Route path="/editor/:id" element={<EditorPage />} />
        {APP_ROUTES.map((route) =>
          route.path === '/library' ? (
            <Route key={route.path} path={route.path} element={<LibraryPage />} />
          ) : route.path === '/editor' ? (
            <Route key={route.path} path={route.path} element={<EditorPage />} />
          ) : route.path === '/' ? (
            <Route key={route.path} path={route.path} element={<HomePage />} />
          ) : route.path === '/trash' ? (
            <Route key={route.path} path={route.path} element={<TrashPage />} />
          ) : (
            <Route
              key={route.path}
              path={route.path}
              element={
                <Page>
                  <PlaceholderPage labelEn={route.labelEn} labelCn={route.labelCn} />
                </Page>
              }
            />
          ),
        )}
      </Routes>
    </>
  );
}
