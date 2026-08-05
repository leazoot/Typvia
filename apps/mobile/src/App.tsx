// SPIKE (TASK-009): minimal page proving the React frontend renders inside
// the Tauri mobile WebView. Reports the WebView user agent so simulator
// screenshots double as OS/WebView version evidence.
export function App() {
  return (
    <main style={{ fontFamily: 'sans-serif', padding: '4rem 1.5rem' }}>
      <h1>Typvia mobile shell spike</h1>
      <p>React page rendered inside Tauri 2 mobile WebView.</p>
      <p style={{ fontSize: '0.8rem', overflowWrap: 'break-word' }}>{navigator.userAgent}</p>
    </main>
  );
}
