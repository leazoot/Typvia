// Shared TypeScript types and the typed IPC layer. Package entry point.
export * from './ipc';
export { APP_ROUTES } from './routes';
export { SNAPSHOT_REFRESH_DEBOUNCE_MS, createSnapshotRefresher } from './snapshot-refresh';
export type { SnapshotRefresher } from './snapshot-refresh';
export type { AppRoute } from './routes';
