import { APP_ROUTES } from '@typvia/shared';
import { PlaceholderPage } from '@typvia/ui';
import { NavLink, Route, Routes } from 'react-router';

// Skeleton shell: an unstyled text navigation over placeholder pages.
// Visual design lands with the token system (TASK-026) and the real
// navigation component (TASK-027); nothing here may hardcode design values.
export function App() {
  return (
    <>
      <nav aria-label="Main">
        <ul>
          {APP_ROUTES.map((route) => (
            <li key={route.path}>
              <NavLink to={route.path} end>
                {route.labelEn}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <Routes>
        {APP_ROUTES.map((route) => (
          <Route
            key={route.path}
            path={route.path}
            element={<PlaceholderPage labelEn={route.labelEn} labelCn={route.labelCn} />}
          />
        ))}
      </Routes>
    </>
  );
}
