import { getItem } from "./api";

export function App() {
  function onLoad() {
    getItem(1);
  }

  return (
    <main>
      <button onClick={onLoad}>Load</button>
    </main>
  );
}
