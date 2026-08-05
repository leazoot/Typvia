import type { ReactNode } from 'react';
import './page.css';

interface PageProps {
  children: ReactNode;
}

/** Desktop page container: the 40px page padding on the paper surface. */
export function Page({ children }: PageProps) {
  return <main className="tv-page">{children}</main>;
}
