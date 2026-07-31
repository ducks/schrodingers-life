const elements = {
  status: document.querySelector("#status"),
  observers: document.querySelector("#observers"),
  creature: document.querySelector("#creature"),
  identity: document.querySelector("#identity"),
  lifetime: document.querySelector("#lifetime"),
  countdown: document.querySelector("#countdown"),
  graveyard: document.querySelector("#graveyard"),
  memorial: document.querySelector("#memorial"),
  memorialTitle: document.querySelector("#memorial-title"),
  memorialDetails: document.querySelector("#memorial-details"),
  openBox: document.querySelector("#open-box"),
  record: document.querySelector("#record"),
  share: document.querySelector("#share"),
};

const LAST_LIFE_KEY = "schrodingers-life:last-observed-life";
let socket;
let state;
let frame = 0;
let heartbeat;
let deathEndsAt;

function connect() {
  if (document.hidden || socket?.readyState === WebSocket.OPEN) return;
  elements.memorial.hidden = true;
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
  render(await fetchState());
}

async function fetchState() {
  const response = await fetch("/api/state", { cache: "no-store" });
  return response.json();
}

function rememberedLife() {
  try {
    return Number(localStorage.getItem(LAST_LIFE_KEY)) || undefined;
  } catch (_) {
    return undefined;
  }
}

function rememberLife(id) {
  try {
    localStorage.setItem(LAST_LIFE_KEY, String(id));
  } catch (_) {}
}

async function resumeObservation() {
  if (document.hidden) return;
  const snapshot = await fetchState();
  const priorLife = rememberedLife();
  const death = !snapshot.alive
    ? snapshot.graveyard.find((life) => life.id === priorLife)
    : undefined;

  render(snapshot);
  if (death) {
    elements.memorialTitle.textContent = `Life #${death.id} died while you were away.`;
    elements.memorialDetails.textContent =
      `${death.rarity} ${death.species} · peak observation ${death.peak_observers}`;
    elements.memorial.hidden = false;
    return;
  }
  connect();
}

function render(next) {
  state = next;
  deathEndsAt = next.death_in_seconds
    ? Date.now() + next.death_in_seconds * 1000
    : undefined;
  elements.observers.textContent = next.observers;
  elements.record.textContent = next.longest_life
    ? `Record: Life #${next.longest_life.id} · ${formatDuration(next.longest_life.duration_seconds)}`
    : "No lifetime record yet.";

  if (next.alive && next.life) {
    const { creature, id, born_at } = next.life;
    rememberLife(id);
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
        <small>${formatDuration(life.duration_seconds)} · peak ${life.peak_observers}</small>
      </article>`).join("")
    : '<p class="muted">No previous lives. Yet.</p>';
}

function formatDuration(totalSeconds) {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (days) return `${days}d ${hours}h`;
  if (hours) return `${hours}h ${minutes}m`;
  if (minutes) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

async function shareCurrentLife() {
  const life = state?.life;
  const text = life
    ? `Life #${life.id} is alive because ${state.observers} ${state.observers === 1 ? "person is" : "people are"} looking.`
    : "The box is empty. Your observation could begin a new life.";
  const share = {
    title: "Schrödinger's Life",
    text,
    url: "https://schrodingers.life/",
  };

  if (navigator.share) {
    await navigator.share(share);
  } else {
    await navigator.clipboard.writeText(`${text} ${share.url}`);
    elements.share.textContent = "Link copied";
    setTimeout(() => { elements.share.textContent = "Share this life"; }, 1500);
  }
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
  document.hidden ? disconnect() : resumeObservation();
});

elements.openBox.addEventListener("click", connect);
elements.share.addEventListener("click", () => {
  shareCurrentLife().catch(() => {});
});
resumeObservation();
