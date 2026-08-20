/**
 * The anti-relay check of the pairing handshake. Both devices compute these
 * characters independently; a server sitting in the middle cannot make them
 * agree, so this comparison is the only thing between the user and a relaying
 * server.
 *
 * It lives here rather than in either app because it must exist exactly once:
 * a second copy is a second place the gate could be weakened, and both hosts
 * have to show the same four groups with the same warning.
 *
 * The class names are the shared vocabulary; each app's stylesheet gives them
 * its own metrics (desktop page rhythm vs. mobile touch targets).
 */
import { useTr } from '../i18n';

interface SasCompareProps {
  /** The short authentication string, already grouped for reading. */
  sas: string;
  /** What the user is deciding, e.g. "Admit Pixel 8?". */
  heading: string;
  /** Who or what is on the other end, e.g. "Pixel 8 · android". */
  detail: string;
}

export function SasCompare({ sas, heading, detail }: SasCompareProps) {
  const tr = useTr();
  return (
    <>
      <div className="tv-sync-rail-label">{tr('Compare the characters', '核对短码')}</div>
      <h2 className="tv-sync-conflict-title">{heading}</h2>
      <p className="tv-sync-lead">{detail}</p>
      <p className="tv-sync-sas" aria-label={tr(`Verification code ${sas}`, `验证码 ${sas}`)}>
        {sas}
      </p>
      <p className="tv-sync-lead">
        {tr(
          'The four groups above must be identical on both devices. If they differ, something is relaying this pairing — stop here and nothing will have been exchanged.',
          '两台设备上的四组字符必须完全一致。如果不一致，说明有什么在中转这次配对——就此停止，不会有任何内容被交换。',
        )}
      </p>
    </>
  );
}
