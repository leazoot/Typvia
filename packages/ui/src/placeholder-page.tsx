interface PlaceholderPageProps {
  labelEn: string;
  labelCn: string;
}

/**
 * Unstyled stand-in body for screens that are not implemented yet.
 * Replaced page by page from STAGE-08 on; carries no design values on
 * purpose — styling arrives with the token system (TASK-026).
 */
export function PlaceholderPage({ labelEn, labelCn }: PlaceholderPageProps) {
  return (
    <main>
      <h1>{labelEn}</h1>
      <p lang="zh-Hans">{labelCn}</p>
      <p>This screen is not implemented yet.</p>
    </main>
  );
}
