const elements = {
  status: document.querySelector("#status"),
  observers: document.querySelector("#observers"),
  creature: document.querySelector("#creature"),
  identity: document.querySelector("#identity"),
  lifetime: document.querySelector("#lifetime"),
  countdown: document.querySelector("#countdown"),
  graveyard: document.querySelector("#graveyard"),
};

let socket;
let state;
let frame = 0;
let heartbeat;
let deathEndsAt;

function connect() {
  if (document.hidden || socket?.readyState === WebSocket.OPEN) return;
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  socket = new WebSocket(`${protocol}//${location.host}/observe`);

  socket.addEventListener("open", () => {
    heartbeat = setInterval(() => socket.send("still-looking"), 10_000);
    refresh();
  });
  socket.addEventListener("message", ({ data }) => {
    try { render(JSON.parse(data)); } catch (_) {}
  });
  socket.addEventListener("close", () => {
    clearInterval(heartbeat);
    if (!document.hidden) setTimeout(connect, 1500);
  });
}

function disconnect() {
  clearInterval(heartbeat);
  socket?.close();
  socket = undefined;
}

async function refresh() {
  const response = await fetch("/api/state", { cache: "no-store" });
  render(await response.json());
}

function render(next) {
  state = next;
  deathEndsAt = next.death_in_seconds
    ? Date.now() + next.death_in_seconds * 1000
    : undefined;
  elements.observers.textContent = next.observers;

  if (next.alive && next.life) {
    const { creature, id, born_at } = next.life;
    elements.status.textContent = "ALIVE";
    elements.status.className = "status alive";
    elements.creature.textContent = creature.frames[frame % creature.frames.length].join("\n");
    elements.identity.textContent =
      `Life #${id} · ${creature.shiny ? "shiny " : ""}${creature.rarity} ${creature.species}`;
    elements.lifetime.dataset.born = born_at;
    elements.countdown.textContent = next.death_in_seconds
      ? `No observers remain. Decoherence in ${next.death_in_seconds}s.`
      : "Your attention is keeping it alive.";
  } else {
    elements.status.textContent = "DEAD";
    elements.status.className = "status dead";
    elements.creature.textContent = "        ┌──────────┐\n        │          │\n        │   ?      │\n        └──────────┘";
    elements.identity.textContent = "The box is empty.";
    elements.lifetime.textContent = "";
    elements.lifetime.dataset.born = "";
    elements.countdown.textContent = "Observation will begin a new life.";
  }

  elements.graveyard.innerHTML = next.graveyard.length
    ? next.graveyard.map(life => `
      <article>
        <strong>Life #${life.id}</strong>
        <span>${life.shiny ? "shiny " : ""}${life.rarity} ${life.species}</span>
        <small>Peak observation: ${life.peak_observers}</small>
      </article>`).join("")
    : '<p class="muted">No previous lives. Yet.</p>';
}

setInterval(() => {
  frame += 1;
  if (state?.life) {
    const frames = state.life.creature.frames;
    elements.creature.textContent = frames[frame % frames.length].join("\n");
  }
  const born = elements.lifetime.dataset.born;
  if (born) {
    const seconds = Math.max(0, Math.floor((Date.now() - new Date(born)) / 1000));
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    elements.lifetime.textContent = `Alive for ${hours}h ${minutes}m ${seconds % 60}s`;
  }
  if (deathEndsAt) {
    const seconds = Math.max(0, Math.ceil((deathEndsAt - Date.now()) / 1000));
    elements.countdown.textContent = `No observers remain. Decoherence in ${seconds}s.`;
  }
}, 500);

document.addEventListener("visibilitychange", () => {
  document.hidden ? disconnect() : connect();
});

connect();
