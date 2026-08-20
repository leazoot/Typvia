import { useTr } from './i18n';

interface PlaceholderPageProps {
  labelEn: string;
  labelCn: string;
}

/**
 * Unstyled stand-in body for screens that are not implemented yet.
 * Replaced page by page; carries no design values on purpose — styling
 * arrives with the token system.
 */
export function PlaceholderPage({ labelEn, labelCn }: PlaceholderPageProps) {
  const tr = useTr();
  return (
    <section>
      <h1>{tr(labelEn, labelCn)}</h1>
      <p>{tr('This screen is not implemented yet.', '该界面尚未实现。')}</p>
    </section>
  );
}
