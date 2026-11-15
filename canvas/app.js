/* OmaPaper — the canvas client.
   No framework, no build step: the daemon serves this file, the theme comes
   from /theme.css, and frames are plain iframes of /api/frames/<id>/html. */

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

const store = {
  get(key, fallback) {
    try {
      const value = localStorage.getItem(`omapaper.${key}`);
      return value === null ? fallback : value;
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(`omapaper.${key}`, value);
    } catch {
      /* private mode: ignore */
    }
  },
};

const state = {
  canvases: [],
  canvasId: store.get("canvas", ""),
  canvas: null,
  frames: [],
  comments: [],
  peers: [],
  themes: [],
  zoom: Number(store.get("zoom", "1")) || 1,
  viewTheme: store.get("theme", ""),
  selectedFrameId: store.get("frame", ""),
  selectedNode: null,
  selectedNodes: [],
  frameFilter: "",
  tab: "dashboard",
  peerId: store.get("peer", "") || crypto.randomUUID(),
  name: store.get("name", "You"),
  ws: null,
  wsRetry: null,
  playStack: [],
  playFrameId: null,
  undoStacks: new Map(),
  redoStacks: new Map(),
  readonly: new URLSearchParams(location.search).has("readonly"),
  lastCursor: 0,
};

store.set("peer", state.peerId);

// ---- network --------------------------------------------------------------

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: { "content-type": "application/json" },
    ...options,
  });
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const body = await response.json();
      if (body.error) message = body.error;
    } catch {
      /* not JSON */
    }
    throw new Error(message);
  }
  if (response.status === 204) return null;
  const type = response.headers.get("content-type") || "";
  return type.includes("application/json") ? response.json() : response.text();
}

function toast(message, isError = false) {
  const element = $("#toast");
  element.textContent = message;
  element.classList.toggle("error", isError);
  element.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => element.classList.remove("show"), 2600);
}

// ---- boot -----------------------------------------------------------------

async function boot() {
  try {
    const [info, themes] = await Promise.all([api("/api/info"), api("/api/themes")]);
    state.themes = themes.themes;
    $("#daemonInfo").textContent = `v${info.version} · ${info.renderer || "no renderer"}`;
    fillThemeSelect(themes.live);
  } catch (error) {
    $("#daemonInfo").textContent = "daemon offline";
    toast(`Cannot reach the daemon: ${error.message}`, true);
  }

  await loadCanvases();

  const wanted = new URLSearchParams(location.search).get("canvas");
  if (wanted && state.canvases.some((c) => c.id === wanted)) state.canvasId = wanted;
  if (!state.canvasId && state.canvases.length) state.canvasId = state.canvases[0].id;
  if (!state.canvasId) {
    const canvas = await api("/api/canvases", {
      method: "POST",
      body: JSON.stringify({ name: "Untitled canvas" }),
    });
    state.canvasId = canvas.id;
    await loadCanvases();
  }
  await loadCanvas(state.canvasId);
  bind();
  applyZoom();
  connect();
  setInterval(() => {
    if (!state.ws || state.ws.readyState > 1) refreshPresence();
  }, 15000);
}

async function loadCanvases() {
  state.canvases = await api("/api/canvases");
  const select = $("#canvasSelect");
  select.innerHTML = "";
  for (const canvas of state.canvases) {
    const option = document.createElement("option");
    option.value = canvas.id;
    option.textContent = canvas.name;
    select.appendChild(option);
  }
  select.value = state.canvasId;
}

async function loadCanvas(id) {
  if (!id) return;
  state.canvasId = id;
  store.set("canvas", id);
  const data = await api(`/api/canvases/${id}`);
  state.canvas = data.canvas;
  state.frames = data.frames;
  state.comments = data.comments || [];
  if (!state.frames.some((f) => f.id === state.selectedFrameId)) {
    state.selectedFrameId = state.frames[0]?.id || "";
  }
  store.set("frame", state.selectedFrameId);
  resetStage();
  renderAll();
  renderDashboard();
  if (state.ws && state.ws.readyState === 1) state.ws.close();
  connect();
}

// ---- theme ----------------------------------------------------------------

function fillThemeSelect(live) {
  const select = $("#themeSelect");
  select.innerHTML = "";
  const follow = document.createElement("option");
  follow.value = "";
  follow.textContent = `Omarchy · ${live}`;
  select.appendChild(follow);
  const groups = new Map();
  for (const theme of state.themes) {
    const key = theme.source === "ab-nextjs-theme" ? "ab-nextjs-theme" : theme.source === "omarchy" ? "omarchy" : "built-in";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(theme);
  }
  for (const [label, themes] of groups) {
    const group = document.createElement("optgroup");
    group.label = label;
    for (const theme of themes) {
      const option = document.createElement("option");
      option.value = theme.name;
      option.textContent = theme.name;
      group.appendChild(option);
    }
    select.appendChild(group);
  }
  select.value = state.viewTheme;
}

function frameSrc(frame) {
  const params = new URLSearchParams();
  if (state.viewTheme) params.set("name", state.viewTheme);
  params.set("r", String(frame.revision));
  return `/api/frames/${frame.id}/html?${params}`;
}

// ---- rendering ------------------------------------------------------------

function resetStage() {
  $("#stage").innerHTML = "";
  frameElements.clear();
  pageBlocks.clear();
}

const frameElements = new Map();
const pageBlocks = new Map();

function renderAll() {
  renderRail();
  renderStage();
  renderPanel();
  renderPresence();
  renderCursors();
}

function renderRail() {
  const canvas = state.canvas;
  if (!canvas) return;
  $("#canvasName").textContent = canvas.name;
  $("#frameCount").textContent = String(state.frames.length);

  const pageNav = $("#pageNav");
  pageNav.innerHTML = "";
  for (const page of canvas.pages) {
    const count = state.frames.filter((f) => f.pageId === page.id).length;
    const button = document.createElement("button");
    button.className = "nav-item page-item";
    button.innerHTML = `<span class="glyph">▤</span><span class="label"></span><span class="count">${count}</span>${state.readonly ? "" : `<span class="page-actions"><button class="page-action" title="Rename page">✎</button><button class="page-action danger" title="Delete page">×</button></span>`}`;
    button.querySelector(".label").textContent = page.name;
    button.onclick = () => {
      const block = pageBlocks.get(page.id);
      block?.scrollIntoView({ behavior: "smooth", block: "start" });
    };
    if (!state.readonly) {
      const actions = button.querySelector(".page-actions");
      const [renameBtn, deleteBtn] = actions.querySelectorAll("button");
      renameBtn.onclick = (event) => {
        event.stopPropagation();
        renamePage(page.id, page.name);
      };
      deleteBtn.onclick = (event) => {
        event.stopPropagation();
        deletePage(page.id);
      };
    }
    pageNav.appendChild(button);
  }

  const nav = $("#frameNav");
  nav.innerHTML = "";
  const filter = state.frameFilter || "";
  const frames = filter
    ? state.frames.filter((frame) => (frame.name || "").toLowerCase().includes(filter))
    : state.frames;
  $("#frameCount").textContent = filter && frames.length !== state.frames.length
    ? `${frames.length}/${state.frames.length}`
    : String(state.frames.length);
  for (const frame of frames) {
    const button = document.createElement("button");
    button.className = `nav-item${frame.id === state.selectedFrameId ? " active" : ""}`;
    button.innerHTML = `<span class="glyph">▢</span><span class="label"></span><span class="meta"></span>`;
    button.querySelector(".label").textContent = frame.name;
    button.querySelector(".meta").textContent = `${Math.round(frame.width)}×${Math.round(frame.height)}`;
    button.onclick = () => {
      selectFrame(frame.id);
      frameElements.get(frame.id)?.scrollIntoView({ behavior: "smooth", block: "center" });
    };
    nav.appendChild(button);
  }
}

function renderStage() {
  const canvas = state.canvas;
  const stage = $("#stage");
  if (!canvas) return;
  const empty = state.frames.length === 0;
  $("#empty").hidden = !empty;
  stage.hidden = empty;

  const wanted = new Set();
  for (const page of canvas.pages) {
    let block = pageBlocks.get(page.id);
    if (!block) {
      block = document.createElement("section");
      block.className = "page-block";
      block.innerHTML = `
        <div class="page-head">
          <h2></h2><span class="rule"></span><span class="count"></span>
        </div>
        <div class="frames"></div>`;
      pageBlocks.set(page.id, block);
      stage.appendChild(block);
    }
    block.querySelector("h2").textContent = page.name;
    const frames = state.frames.filter((f) => f.pageId === page.id);
    block.querySelector(".count").textContent = `${frames.length} ${frames.length === 1 ? "frame" : "frames"}`;
    const wrap = block.querySelector(".frames");
    const placeholder = wrap.querySelector(".placeholder");
    if (frames.length && placeholder) placeholder.remove();
    if (!frames.length && !placeholder) {
      const note = document.createElement("p");
      note.className = "muted placeholder";
      note.style.fontSize = "10px";
      note.textContent = "No frames on this page yet.";
      wrap.appendChild(note);
    }
    for (const frame of frames) {
      wanted.add(frame.id);
      const element = frameElement(frame);
      if (element.parentElement !== wrap) wrap.appendChild(element);
    }
  }

  for (const [id, element] of frameElements) {
    if (!wanted.has(id)) {
      element.remove();
      frameElements.delete(id);
    }
  }
  for (const [id, block] of pageBlocks) {
    if (!canvas.pages.some((p) => p.id === id)) {
      block.remove();
      pageBlocks.delete(id);
    }
  }
}

function frameElement(frame) {
  let element = frameElements.get(frame.id);
  if (!element) {
    element = document.createElement("article");
    element.className = "frame";
    element.dataset.id = frame.id;
    element.innerHTML = `
      <div class="frame-head">
        <span class="fname"></span>
        <span class="size"></span>
        <span class="badge"></span>
        <span class="pin" hidden></span>
      </div>
      <div class="frame-tools">
        <button data-act="comment" title="Pin a comment">note</button>
        <button data-act="html" title="Inspect HTML">html</button>
        <button data-act="png" title="Export PNG">png</button>
      </div>
      <div class="canvas-box"><iframe title="${escapeAttr(frame.name)}"></iframe><div class="resize-handle" title="Resize frame" aria-label="Resize frame"></div></div>`;
    frameElements.set(frame.id, element);
    element.addEventListener("click", (event) => {
      if (event.target.closest(".frame-tools")) return;
      selectFrame(frame.id);
    });
    element.querySelector(".frame-tools").addEventListener("click", async (event) => {
      const action = event.target.dataset.act;
      if (!action) return;
      event.stopPropagation();
      selectFrame(frame.id);
      if (action === "comment") {
        setTab("comments");
        $("#commentBody").focus();
      } else if (action === "html") {
        setTab("inspect");
        $("#htmlEditor").focus();
      } else if (action === "png") {
        window.open(`/api/frames/${frame.id}/export?format=png`, "_blank");
      }
    });
    element.querySelector(".resize-handle").addEventListener("mousedown", (event) => beginResize(event, frame.id));
    element.querySelector("iframe").addEventListener("load", () => wireFrame(frame.id));
  }

  element.classList.toggle("selected", frame.id === state.selectedFrameId);
  element.querySelector(".fname").textContent = frame.name;
  element.querySelector(".size").textContent = `${Math.round(frame.width)}×${Math.round(frame.height)}`;
  const badge = element.querySelector(".badge");
  badge.textContent =
    frame.approval === "ready_for_dev" ? "ready" : frame.approval === "needs_review" ? "review" : "";
  badge.className = `badge ${frame.approval}`;
  if (frame.locked) {
    badge.textContent = badge.textContent ? `${badge.textContent} · locked` : "locked";
  }
  const open = state.comments.filter((c) => c.frameId === frame.id && !c.resolved).length;
  const pin = element.querySelector(".pin");
  pin.hidden = open === 0;
  pin.textContent = `${open} note${open === 1 ? "" : "s"}`;

  element.style.setProperty("--w", `${frame.width}px`);
  element.style.setProperty("--h", `${frame.height}px`);
  const iframe = element.querySelector("iframe");
  const src = frameSrc(frame);
  if (iframe.dataset.src !== src) {
    iframe.dataset.src = src;
    iframe.src = src;
  }
  return element;
}

function escapeAttr(value) {
  return String(value).replace(/&/g, "&amp;").replace(/"/g, "&quot;");
}

function selectFrame(id) {
  state.selectedFrameId = id;
  state.selectedNode = null;
  state.selectedNodes = [];
  store.set("frame", id);
  for (const [frameId, element] of frameElements) {
    element.classList.toggle("selected", frameId === id);
  }
  renderRail();
  renderPanel();
  sendSelection();
}

function selectedFrame() {
  return state.frames.find((f) => f.id === state.selectedFrameId) || null;
}

// ---- frames inside the canvas ---------------------------------------------

function wireFrame(frameId) {
  const element = frameElements.get(frameId);
  const iframe = element?.querySelector("iframe");
  const doc = iframe?.contentDocument;
  if (!doc) return;
  doc.addEventListener("click", (event) => {
    event.preventDefault();
    if (state.selectedFrameId !== frameId) {
      state.selectedFrameId = frameId;
      store.set("frame", frameId);
      renderRail();
    }
    const target = event.target.closest("[data-op-id]") || event.target;
    if (!(target instanceof Element)) return;
    const node = {
      id: target.getAttribute("data-op-id") || "",
      tag: target.tagName.toLowerCase(),
      label: target.getAttribute("data-name") || target.getAttribute("data-op-id") || target.tagName.toLowerCase(),
      frameId,
    };
    if (event.shiftKey) {
      const existing = state.selectedNodes.findIndex((entry) => entry.id === node.id && entry.frameId === frameId);
      if (existing >= 0) state.selectedNodes.splice(existing, 1);
      else state.selectedNodes.push(node);
    } else {
      state.selectedNodes = [node];
    }
    state.selectedNode = state.selectedNodes[state.selectedNodes.length - 1] || null;
    for (const marked of doc.querySelectorAll("[data-op-selected]")) marked.removeAttribute("data-op-selected");
    for (const selected of state.selectedNodes) {
      const marked = doc.querySelector(`[data-op-id="${CSS.escape(selected.id)}"]`);
      if (marked) marked.setAttribute("data-op-selected", "");
    }
    if (!state.selectedNodes.length) target.setAttribute("data-op-selected", "");
    selectFrameNode(frameId);
  });
  doc.addEventListener("mousemove", (event) => {
    const now = Date.now();
    if (now - state.lastCursor < 140) return;
    state.lastCursor = now;
    const layer = $("#cursorLayer").getBoundingClientRect();
    sendSocket({
      type: "cursor",
      x: Math.round(event.clientX + iframe.getBoundingClientRect().left - layer.left),
      y: Math.round(event.clientY + iframe.getBoundingClientRect().top - layer.top),
      frameId,
    });
  });
  paintCommentPins(frameId);
}

function selectFrameNode(frameId) {
  if (state.selectedFrameId !== frameId) {
    state.selectedFrameId = frameId;
    store.set("frame", frameId);
    renderRail();
  }
  renderPanel();
  sendSelection();
  renderCursors();
}

function sendSelection() {
  if (!state.canvasId) return;
  api(`/api/canvases/${state.canvasId}/selection`, {
    method: "POST",
    body: JSON.stringify({
      peerId: state.peerId,
      frameId: state.selectedFrameId || null,
      pageId: state.canvas?.pages?.[0]?.id || null,
      nodeId: state.selectedNode?.id || null,
    }),
  }).catch(() => {});
}

function paintCommentPins(frameId) {
  const element = frameElements.get(frameId);
  const iframe = element?.querySelector("iframe");
  const doc = iframe?.contentDocument;
  if (!doc) return;
  for (const old of doc.querySelectorAll(".op-pin")) old.remove();
  const notes = state.comments.filter((c) => c.frameId === frameId && !c.resolved && c.elementId);
  for (const note of notes) {
    const target = doc.querySelector(`[data-op-id="${CSS.escape(note.elementId)}"]`);
    if (!target) continue;
    const rect = target.getBoundingClientRect();
    const pin = doc.createElement("button");
    pin.className = "op-pin";
    pin.textContent = "●";
    pin.title = note.body;
    pin.style.cssText = `position:fixed;left:${rect.left + 8}px;top:${rect.top + 8}px;z-index:2147483647;width:14px;height:14px;border-radius:50%;border:2px solid #111;background:#f38d70;color:#111;font-size:7px;line-height:10px;padding:0;cursor:pointer;`;
    pin.onclick = (event) => {
      event.stopPropagation();
      setTab("comments");
      $(`[data-comment="${note.id}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" });
    };
    doc.body.appendChild(pin);
  }
}

// ---- inspector ------------------------------------------------------------

function renderPanel() {
  const frame = selectedFrame();
  $("#inspectEmpty").hidden = Boolean(frame);
  $("#inspectBody").hidden = !frame;
  if (frame) {
    $("#frameName").value = frame.name;
    $("#frameWidth").value = Math.round(frame.width);
    $("#frameHeight").value = Math.round(frame.height);
    $("#frameBg").value = frame.background || "";
    $("#frameTheme").value = frame.theme || "";
    $("#frameApproval").value = frame.approval || "draft";
    const editor = $("#htmlEditor");
    if (document.activeElement !== editor) editor.value = frame.html || "";
    if (!frame.html) loadFrameHtml(frame);
    const node = state.selectedNode;
    const selectedCount = state.selectedNodes.length;
    $("#selectionInfo").innerHTML = selectedCount > 1
      ? `${selectedCount} elements selected`
      : node
        ? `element <b>${escapeHtml(node.label)}</b> · ${escapeHtml(node.tag)}${node.id ? ` · data-op-id <b>${escapeHtml(node.id)}</b>` : ""}`
        : `frame <b>${escapeHtml(frame.name)}</b> · ${frame.elements} elements · revision ${frame.revision}`;
    $("#exportHtml").href = `/api/frames/${frame.id}/export?format=html`;
    $("#exportPng").href = `/api/frames/${frame.id}/export?format=png`;
    $("#exportPdf").href = `/api/frames/${frame.id}/export?format=pdf`;
    $("#exportHtml").download = `${frame.name}.html`;
    $("#exportPng").download = `${frame.name}.png`;
    $("#exportPdf").download = `${frame.name}.pdf`;
    loadVersions(frame.id);
  }
  renderTokens();
  renderComments();
  $("#commentCount").textContent = state.comments.filter((c) => !c.resolved).length || "";
  const anchor = state.selectedNode
    ? `on <b>${escapeHtml(state.selectedNode.label)}</b>`
    : frame
      ? `on <b>${escapeHtml(frame.name)}</b>`
      : "Select a frame to pin a note.";
  $("#commentAnchor").innerHTML = anchor;
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function renderDashboard() {
  const target = $("#dashboardBody");
  if (!target) return;
  target.innerHTML = "";
  const canvas = state.canvas || {};
  const frames = state.frames || [];
  const comments = state.comments || [];
  const pages = Array.isArray(canvas.pages) ? canvas.pages.length : 0;
  const tokens = Array.isArray(canvas.tokens) ? canvas.tokens.length : 0;

  const card = document.createElement("div");
  card.className = "dashboard-card";
  card.innerHTML = `<h3>${escapeHtml(canvas.name || "Untitled canvas")}</h3><p class="muted">${pages} page${pages === 1 ? "" : "s"} · ${frames.length} frame${frames.length === 1 ? "" : "s"} · ${comments.length} comment${comments.length === 1 ? "" : "s"} · ${tokens} token${tokens === 1 ? "" : "s"}</p>`;
  target.appendChild(card);

  const actions = document.createElement("div");
  actions.className = "actions";
  if (!state.readonly) {
    const newBtn = document.createElement("button");
    newBtn.className = "primary small";
    newBtn.textContent = "+ Frame";
    newBtn.onclick = newFrame;
    actions.appendChild(newBtn);
  }
  const playBtn = document.createElement("button");
  playBtn.className = "ghost small";
  playBtn.textContent = "▶ Play";
  playBtn.onclick = () => openPlay();
  actions.appendChild(playBtn);
  target.appendChild(actions);

  const frameTitle = document.createElement("div");
  frameTitle.className = "rail-label";
  frameTitle.textContent = "frames";
  target.appendChild(frameTitle);
  const frameList = document.createElement("div");
  frameList.className = "dashboard-list";
  if (!frames.length) {
    frameList.innerHTML = `<span class="muted small">No frames yet.</span>`;
  } else {
    for (const frame of frames.slice(0, 8)) {
      const row = document.createElement("div");
      row.className = "dashboard-row";
      row.innerHTML = `<span class="name">${escapeHtml(frame.name || "Untitled")}</span><span class="meta">${frame.width}×${frame.height} · ${escapeHtml(frame.approval || "draft")}</span>`;
      row.onclick = () => {
        selectFrame(frame.id);
        setTab("inspect");
      };
      frameList.appendChild(row);
    }
  }
  target.appendChild(frameList);

  if (comments.length) {
    const commentTitle = document.createElement("div");
    commentTitle.className = "rail-label";
    commentTitle.textContent = "recent comments";
    target.appendChild(commentTitle);
    const commentList = document.createElement("div");
    commentList.className = "comment-list";
    for (const comment of comments.slice(0, 5)) {
      const item = document.createElement("div");
      item.className = "comment";
      item.innerHTML = `<div class="cbody">${escapeHtml(comment.body || "")}</div>`;
      commentList.appendChild(item);
    }
    target.appendChild(commentList);
  }
}

async function loadFrameHtml(frame) {
  try {
    const data = await api(`/api/frames/${frame.id}`);
    const index = state.frames.findIndex((f) => f.id === frame.id);
    if (index >= 0) state.frames[index] = { ...state.frames[index], ...data.frame };
    if (state.selectedFrameId === frame.id && !state.selectedNode) {
      $("#htmlEditor").value = data.frame.html;
    }
  } catch {
    /* the next event will bring it */
  }
}

async function loadVersions(frameId) {
  const target = $("#versions");
  target.innerHTML = `<span class="muted small">loading…</span>`;
  try {
    const versions = await api(`/api/frames/${frameId}/versions`);
    target.innerHTML = "";
    if (!versions.length) {
      target.innerHTML = `<span class="muted small">No versions yet. Every replace keeps one.</span>`;
      return;
    }
    for (const version of versions.slice(0, 8)) {
      const row = document.createElement("div");
      row.className = "version";
      row.innerHTML = `<span>${escapeHtml(version.label || "version")}</span><span class="when">${timeAgo(version.createdAt)}</span><button class="ghost" data-id="${version.id}">restore</button>`;
      row.querySelector("button").onclick = async () => {
        try {
          await api(`/api/frames/${frameId}/versions/${version.id}/restore`, { method: "POST" });
          toast("Version restored");
          await reloadFrame(frameId);
        } catch (error) {
          toast(error.message, true);
        }
      };
      target.appendChild(row);
    }
  } catch (error) {
    target.innerHTML = `<span class="muted small">${escapeHtml(error.message)}</span>`;
  }
}

function timeAgo(ms) {
  const seconds = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

// ---- tokens ---------------------------------------------------------------

function renderTokens() {
  const canvas = state.canvas;
  const list = $("#tokenList");
  const themes = $("#tokenThemes");
  if (!canvas) return;
  themes.innerHTML = "";
  for (const theme of canvas.tokenThemes || []) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = theme;
    themes.appendChild(chip);
  }
  list.innerHTML = "";
  if (!canvas.tokens?.length) {
    list.innerHTML = `<p class="muted small">No tokens yet. An agent can create them with <code>create_tokens</code>; they become var(--name) in every frame.</p>`;
    $("#tokenCss").textContent = "";
    return;
  }
  for (const token of canvas.tokens) {
    const row = document.createElement("div");
    row.className = "token";
    const value = token.values?.default || Object.values(token.values || {})[0] || "";
    const swatch = /^#|rgb|hsl/.test(value)
      ? `<span class="swatch" style="background:${escapeAttr(value)}"></span>`
      : `<span class="swatch"></span>`;
    row.innerHTML = `${swatch}<span class="tname" title="${escapeAttr(token.description || "")}">${escapeHtml(token.name)}</span><span class="tvalue">${escapeHtml(value)}</span>`;
    list.appendChild(row);
  }
  $("#tokenCss").textContent = cssForTokens(canvas);
}

function cssForTokens(canvas) {
  const lines = [":root {"];
  for (const token of canvas.tokens || []) {
    const theme = canvas.tokenThemes?.[0] || "default";
    const value = token.values?.[theme] ?? token.values?.default ?? Object.values(token.values || {})[0] ?? "";
    lines.push(`  --${token.name}: ${value};`);
  }
  lines.push("}");
  return lines.join("\n");
}

// ---- comments -------------------------------------------------------------

function renderComments() {
  const list = $("#commentList");
  list.innerHTML = "";
  if (!state.comments.length) {
    list.innerHTML = `<p class="muted small">No comments yet.</p>`;
    return;
  }
  const sorted = [...state.comments].sort((a, b) => Number(a.resolved) - Number(b.resolved) || b.createdAt - a.createdAt);
  for (const comment of sorted) {
    const card = document.createElement("article");
    card.className = `comment${comment.resolved ? " resolved" : ""}`;
    card.dataset.comment = comment.id;
    const frame = state.frames.find((f) => f.id === comment.frameId);
    const anchor = comment.elementId
      ? `${frame?.name || "frame"} · ${comment.elementId}`
      : comment.frameId
        ? frame?.name || "frame"
        : "page";
    card.innerHTML = `
      <div class="chead"><b>${escapeHtml(comment.author)}</b><span>${timeAgo(comment.createdAt)}</span><span class="anchor">${escapeHtml(anchor)}</span></div>
      <p class="cbody">${escapeHtml(comment.body)}</p>
      <div class="replies"></div>
      <div class="cactions">
        <button class="ghost" data-act="reply">Reply</button>
        <button class="ghost" data-act="resolve">${comment.resolved ? "Reopen" : "Resolve"}</button>
      </div>`;
    const replies = card.querySelector(".replies");
    for (const reply of comment.replies || []) {
      const line = document.createElement("div");
      line.className = "reply";
      line.innerHTML = `<b>${escapeHtml(reply.author)}</b> ${escapeHtml(reply.body)}`;
      replies.appendChild(line);
    }
    card.querySelector('[data-act="reply"]').onclick = async () => {
      const body = prompt("Reply");
      if (!body) return;
      try {
        await api(`/api/comments/${comment.id}/replies`, {
          method: "POST",
          body: JSON.stringify({ body, author: state.name }),
        });
        await refreshComments();
      } catch (error) {
        toast(error.message, true);
      }
    };
    card.querySelector('[data-act="resolve"]').onclick = async () => {
      try {
        await api(`/api/comments/${comment.id}`, {
          method: "PATCH",
          body: JSON.stringify({ resolved: !comment.resolved }),
        });
        await refreshComments();
      } catch (error) {
        toast(error.message, true);
      }
    };
    list.appendChild(card);
  }
}

async function refreshComments() {
  if (!state.canvasId) return;
  state.comments = await api(`/api/canvases/${state.canvasId}/comments`);
  renderComments();
  renderPanel();
  for (const frame of state.frames) paintCommentPins(frame.id);
}

async function addComment() {
  const body = $("#commentBody").value.trim();
  if (!body) return;
  if (!state.selectedFrameId) {
    toast("Select a frame first", true);
    return;
  }
  try {
    await api(`/api/canvases/${state.canvasId}/comments`, {
      method: "POST",
      body: JSON.stringify({
        frameId: state.selectedFrameId,
        elementId: state.selectedNode?.id || null,
        body,
        author: state.name,
      }),
    });
    $("#commentBody").value = "";
    await refreshComments();
    toast("Comment pinned");
  } catch (error) {
    toast(error.message, true);
  }
}

// ---- presence -------------------------------------------------------------

function initials(name) {
  return name
    .split(/[\s_-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0].toUpperCase())
    .join("");
}

function renderPresence() {
  const target = $("#presence");
  target.innerHTML = "";
  for (const peer of state.peers.slice(0, 6)) {
    const dot = document.createElement("span");
    dot.className = `peer${peer.kind === "agent" ? " agent" : ""}`;
    dot.style.background = peer.color || "var(--op-accent)";
    dot.textContent = initials(peer.name);
    dot.title = `${peer.name}${peer.status ? ` — ${peer.status}` : ""}`;
    target.appendChild(dot);
  }
  if (state.peers.length > 6) {
    const more = document.createElement("span");
    more.className = "peer";
    more.textContent = `+${state.peers.length - 6}`;
    target.appendChild(more);
  }
}

function renderCursors() {
  const layer = $("#cursorLayer");
  layer.innerHTML = "";
  for (const peer of state.peers) {
    if (peer.id === state.peerId) continue;
    if (!peer.x && !peer.y) continue;
    const cursor = document.createElement("div");
    cursor.className = "cursor";
    cursor.style.left = `${peer.x}px`;
    cursor.style.top = `${peer.y}px`;
    cursor.style.setProperty("--cursor-color", peer.color || "var(--op-accent)");
    cursor.innerHTML = `<span>${escapeHtml(peer.name)}</span>`;
    layer.appendChild(cursor);
  }
}

async function refreshPresence() {
  if (!state.canvasId) return;
  try {
    state.peers = await api(`/api/canvases/${state.canvasId}/presence`);
    renderPresence();
    renderCursors();
  } catch {
    /* offline */
  }
}

// ---- websocket ------------------------------------------------------------

function connect() {
  if (!state.canvasId) return;
  if (state.ws && state.ws.readyState <= 1) return;
  const protocol = location.protocol === "https:" ? "wss" : "ws";
  const socket = new WebSocket(`${protocol}://${location.host}/ws?canvasId=${encodeURIComponent(state.canvasId)}`);
  state.ws = socket;
  socket.onopen = () => {
    state.wsRetry = null;
    sendSocket({ type: "hello", peerId: state.peerId, name: state.name, kind: "human" });
    sendSelection();
    refreshPresence();
  };
  socket.onmessage = (event) => {
    try {
      handleEvent(JSON.parse(event.data));
    } catch {
      /* ignore malformed */
    }
  };
  socket.onclose = () => {
    state.ws = null;
    if (!state.wsRetry) {
      state.wsRetry = setTimeout(() => {
        state.wsRetry = null;
        connect();
      }, 1500);
    }
  };
}

function sendSocket(message) {
  if (state.ws && state.ws.readyState === 1) state.ws.send(JSON.stringify(message));
}

function handleEvent(event) {
  switch (event.type) {
    case "presence":
      state.peers = event.peers || [];
      renderPresence();
      renderCursors();
      break;
    case "selection": {
      const selection = event.selection;
      if (!selection) break;
      break;
    }
    case "frame.updated": {
      const index = state.frames.findIndex((f) => f.id === event.frame.id);
      if (index >= 0) {
        const wasSelected = state.selectedFrameId === event.frame.id;
        state.frames[index] = {
          ...state.frames[index],
          ...event.frame,
          elements: event.frame.elements ?? state.frames[index].elements,
        };
        const summary = state.frames[index];
        frameElement(summary);
        renderRail();
        if (wasSelected) {
          renderPanel();
          if (!state.selectedNode) $("#htmlEditor").value = event.frame.html;
        }
      }
      break;
    }
    case "frame.created":
      if (!state.frames.some((f) => f.id === event.frame.id)) {
        state.frames.push({ ...event.frame, elements: event.frame.elements || 0 });
      }
      renderStage();
      renderRail();
      break;
    case "frame.deleted":
      state.frames = state.frames.filter((f) => f.id !== event.frameId);
      frameElements.get(event.frameId)?.remove();
      frameElements.delete(event.frameId);
      if (state.selectedFrameId === event.frameId) {
        state.selectedFrameId = state.frames[0]?.id || "";
        store.set("frame", state.selectedFrameId);
      }
      renderAll();
      break;
    case "canvas.updated":
      if (event.canvas.id === state.canvasId) {
        state.canvas = event.canvas;
        reloadCanvasData().catch(() => {});
      }
      break;
    case "canvas.created":
      loadCanvases().catch(() => {});
      break;
    case "comment.created":
    case "comment.updated":
      refreshComments().catch(() => {});
      break;
    case "theme.changed":
      if (!state.viewTheme) {
        for (const frame of state.frames) {
          const element = frameElements.get(frame.id);
          const iframe = element?.querySelector("iframe");
          if (iframe) {
            iframe.dataset.src = "";
            iframe.src = frameSrc(frame);
          }
        }
      }
      break;
    default:
      break;
  }
}

// ---- actions --------------------------------------------------------------

async function reloadFrame(frameId) {
  const data = await api(`/api/frames/${frameId}`);
  const index = state.frames.findIndex((f) => f.id === frameId);
  if (index >= 0) state.frames[index] = { ...state.frames[index], ...data.frame };
  frameElement(state.frames[index]);
  renderRail();
  renderPanel();
}

let resizeState = null;

function beginResize(event, frameId) {
  if (state.readonly) return;
  const frame = state.frames.find((f) => f.id === frameId);
  if (!frame) return;
  event.stopPropagation();
  event.preventDefault();
  pushHistory(frameId);
  resizeState = {
    frameId,
    startX: event.clientX,
    startY: event.clientY,
    width: frame.width,
    height: frame.height,
    currentWidth: frame.width,
    currentHeight: frame.height,
  };
  document.body.style.cursor = "nwse-resize";
}

function applyResize(event) {
  if (!resizeState) return;
  const scale = state.zoom || 1;
  const width = Math.max(120, resizeState.width + (event.clientX - resizeState.startX) / scale);
  const height = Math.max(120, resizeState.height + (event.clientY - resizeState.startY) / scale);
  resizeState.currentWidth = width;
  resizeState.currentHeight = height;
  const element = frameElements.get(resizeState.frameId);
  if (!element) return;
  element.style.setProperty("--w", `${width}px`);
  element.style.setProperty("--h", `${height}px`);
  const size = element.querySelector(".size");
  if (size) size.textContent = `${Math.round(width)}×${Math.round(height)}`;
}

async function finishResize() {
  if (!resizeState) return;
  const { frameId, currentWidth, currentHeight } = resizeState;
  resizeState = null;
  document.body.style.cursor = "";
  const frame = state.frames.find((f) => f.id === frameId);
  if (!frame) return;
  try {
    const updated = await api(`/api/frames/${frameId}`, {
      method: "PATCH",
      body: JSON.stringify({ width: currentWidth, height: currentHeight }),
    });
    const index = state.frames.findIndex((f) => f.id === frameId);
    if (index >= 0) state.frames[index] = { ...state.frames[index], ...updated };
    frameElement(state.frames[index]);
    renderRail();
    renderPanel();
    toast("Frame resized");
  } catch (error) {
    toast(error.message, true);
    await reloadFrame(frameId);
  }
}

function frameSnapshot(frame) {
  return {
    html: frame.html || "",
    name: frame.name,
    width: frame.width,
    height: frame.height,
    x: frame.x,
    y: frame.y,
    background: frame.background,
    theme: frame.theme,
    approval: frame.approval,
  };
}

function pushHistory(frameId) {
  const frame = state.frames.find((f) => f.id === frameId);
  if (!frame) return;
  if (!state.undoStacks.has(frameId)) state.undoStacks.set(frameId, []);
  if (!state.redoStacks.has(frameId)) state.redoStacks.set(frameId, []);
  state.undoStacks.get(frameId).push(frameSnapshot(frame));
  state.redoStacks.set(frameId, []);
}

async function applySnapshot(frameId, snapshot) {
  try {
    const updated = await api(`/api/frames/${frameId}`, {
      method: "PATCH",
      body: JSON.stringify({
        html: snapshot.html,
        name: snapshot.name,
        width: snapshot.width,
        height: snapshot.height,
        x: snapshot.x,
        y: snapshot.y,
        background: snapshot.background,
        theme: snapshot.theme,
        approval: snapshot.approval,
      }),
    });
    const index = state.frames.findIndex((f) => f.id === frameId);
    if (index >= 0) state.frames[index] = { ...state.frames[index], ...updated };
    frameElement(state.frames[index]);
    renderRail();
    renderPanel();
  } catch (error) {
    toast(error.message, true);
  }
}

async function undoFrame() {
  const frame = selectedFrame();
  if (!frame) return;
  const stack = state.undoStacks.get(frame.id) || [];
  if (!stack.length) {
    toast("Nothing to undo", true);
    return;
  }
  if (!state.redoStacks.has(frame.id)) state.redoStacks.set(frame.id, []);
  state.redoStacks.get(frame.id).push(frameSnapshot(frame));
  const previous = stack.pop();
  await applySnapshot(frame.id, previous);
  toast("Undone");
}

async function redoFrame() {
  const frame = selectedFrame();
  if (!frame) return;
  const stack = state.redoStacks.get(frame.id) || [];
  if (!stack.length) {
    toast("Nothing to redo", true);
    return;
  }
  if (!state.undoStacks.has(frame.id)) state.undoStacks.set(frame.id, []);
  state.undoStacks.get(frame.id).push(frameSnapshot(frame));
  const next = stack.pop();
  await applySnapshot(frame.id, next);
  toast("Redone");
}

async function saveFrame() {
  const frame = selectedFrame();
  if (!frame) return;
  pushHistory(frame.id);
  const body = {
    name: $("#frameName").value.trim() || frame.name,
    width: Number($("#frameWidth").value) || frame.width,
    height: Number($("#frameHeight").value) || frame.height,
    background: $("#frameBg").value.trim() || null,
    theme: $("#frameTheme").value || null,
    approval: $("#frameApproval").value,
  };
  try {
    const updated = await api(`/api/frames/${frame.id}`, { method: "PATCH", body: JSON.stringify(body) });
    const index = state.frames.findIndex((f) => f.id === frame.id);
    if (index >= 0) state.frames[index] = { ...state.frames[index], ...updated };
    frameElement(state.frames[index]);
    renderRail();
    renderPanel();
    toast("Frame saved");
  } catch (error) {
    toast(error.message, true);
  }
}

async function saveHtml() {
  const frame = selectedFrame();
  if (!frame) return;
  pushHistory(frame.id);
  const html = $("#htmlEditor").value;
  try {
    const updated = await api(`/api/frames/${frame.id}`, {
      method: "PATCH",
      body: JSON.stringify({ html }),
    });
    const index = state.frames.findIndex((f) => f.id === frame.id);
    if (index >= 0) state.frames[index] = { ...state.frames[index], ...updated };
    frameElement(state.frames[index]);
    $("#htmlStatus").textContent = `saved · ${updated.revision}`;
    toast("HTML written");
  } catch (error) {
    toast(error.message, true);
  }
}

async function newFrame() {
  if (!state.canvasId) return;
  try {
    const frame = await api(`/api/canvases/${state.canvasId}/frames`, {
      method: "POST",
      body: JSON.stringify({ name: `Frame ${state.frames.length + 1}` }),
    });
    if (!state.frames.some((f) => f.id === frame.id)) {
      state.frames.push({ ...frame, elements: 0 });
    }
    selectFrame(frame.id);
    renderStage();
    renderRail();
    frameElements.get(frame.id)?.scrollIntoView({ behavior: "smooth", block: "center" });
  } catch (error) {
    toast(error.message, true);
  }
}

async function reloadCanvasData() {
  if (!state.canvasId) return;
  try {
    const data = await api(`/api/canvases/${state.canvasId}`);
    state.canvas = data.canvas;
    state.frames = data.frames;
    state.comments = data.comments || [];
    if (!state.frames.some((f) => f.id === state.selectedFrameId)) {
      state.selectedFrameId = state.frames[0]?.id || "";
    }
    store.set("frame", state.selectedFrameId);
    resetStage();
    renderAll();
    renderDashboard();
  } catch (error) {
    toast(error.message, true);
  }
}

async function newPage() {
  if (!state.canvasId || state.readonly) return;
  const name = prompt("Page name", `Page ${(state.canvas?.pages?.length || 0) + 1}`);
  if (!name || !name.trim()) return;
  try {
    await api(`/api/canvases/${state.canvasId}/pages`, {
      method: "POST",
      body: JSON.stringify({ name: name.trim() }),
    });
    toast("Page created");
    await reloadCanvasData();
  } catch (error) {
    toast(error.message, true);
  }
}

async function duplicateCanvas() {
  if (!state.canvasId || state.readonly) return;
  try {
    const canvas = await api(`/api/canvases/${state.canvasId}/duplicate`, { method: "POST" });
    toast("Canvas duplicated");
    await loadCanvases();
    if (canvas?.id) {
      state.canvasId = canvas.id;
      await loadCanvas(canvas.id);
    }
  } catch (error) {
    toast(error.message, true);
  }
}

async function renamePage(pageId, current) {
  if (state.readonly) return;
  const name = prompt("Rename page", current);
  if (!name || !name.trim() || name.trim() === current) return;
  try {
    await api(`/api/canvases/${state.canvasId}/pages/${pageId}`, {
      method: "PATCH",
      body: JSON.stringify({ name: name.trim() }),
    });
    await reloadCanvasData();
  } catch (error) {
    toast(error.message, true);
  }
}

async function deletePage(pageId) {
  if (state.readonly) return;
  if (!confirm("Delete this page? Its frames will move to another page.")) return;
  try {
    await api(`/api/canvases/${state.canvasId}/pages/${pageId}`, { method: "DELETE" });
    toast("Page deleted");
    await reloadCanvasData();
  } catch (error) {
    toast(error.message, true);
  }
}

async function duplicateFrame() {
  const frame = selectedFrame();
  if (!frame) return;
  try {
    const created = await api(`/api/canvases/${state.canvasId}/frames`, {
      method: "POST",
      body: JSON.stringify({
        name: `${frame.name} copy`,
        pageId: frame.pageId,
        width: frame.width,
        height: frame.height,
        html: frame.html || "",
      }),
    });
    state.frames.push({ ...created, elements: frame.elements });
    renderStage();
    renderRail();
    selectFrame(created.id);
  } catch (error) {
    toast(error.message, true);
  }
}

async function deleteFrame() {
  const frame = selectedFrame();
  if (!frame) return;
  if (!confirm(`Delete “${frame.name}”? Its versions go too.`)) return;
  try {
    await api(`/api/frames/${frame.id}`, { method: "DELETE" });
    state.frames = state.frames.filter((f) => f.id !== frame.id);
    frameElements.get(frame.id)?.remove();
    frameElements.delete(frame.id);
    state.selectedFrameId = state.frames[0]?.id || "";
    renderAll();
    toast("Frame deleted");
  } catch (error) {
    toast(error.message, true);
  }
}

async function newCanvas() {
  const name = prompt("Canvas name", "Untitled canvas");
  if (!name) return;
  try {
    const canvas = await api("/api/canvases", { method: "POST", body: JSON.stringify({ name }) });
    state.canvasId = canvas.id;
    await loadCanvases();
    await loadCanvas(canvas.id);
  } catch (error) {
    toast(error.message, true);
  }
}

async function renameCanvas() {
  if (!state.canvas) return;
  const name = prompt("Canvas name", state.canvas.name);
  if (!name) return;
  try {
    state.canvas = await api(`/api/canvases/${state.canvas.id}`, {
      method: "PATCH",
      body: JSON.stringify({ name }),
    });
    await loadCanvases();
    renderRail();
  } catch (error) {
    toast(error.message, true);
  }
}

// ---- zoom and play --------------------------------------------------------

function applyZoom() {
  state.zoom = Math.min(2, Math.max(0.25, state.zoom));
  $("#stage").style.zoom = String(state.zoom);
  $("#zoomLabel").textContent = `${Math.round(state.zoom * 100)}%`;
  store.set("zoom", String(state.zoom));
}

function setTab(tab) {
  state.tab = tab;
  for (const button of $$(".tab")) button.classList.toggle("active", button.dataset.tab === tab);
  for (const pane of ["dashboard", "inspect", "tokens", "comments"]) {
    const element = $(`#pane-${pane}`);
    if (element) element.hidden = pane !== tab;
  }
  if (tab === "dashboard") renderDashboard();
}

function openPlay(frameId = state.selectedFrameId) {
  const frame = state.frames.find((f) => f.id === frameId);
  if (!frame) {
    toast("Add a frame first", true);
    return;
  }
  state.playFrameId = frame.id;
  state.playStack = [];
  $("#playOverlay").hidden = false;
  playShow(frame);
}

function playShow(frame) {
  const iframe = $("#playFrame");
  iframe.dataset.frame = frame.id;
  iframe.dataset.src = frameSrc(frame);
  iframe.src = frameSrc(frame);
  $("#playTitle").textContent = `${state.canvas?.name || "prototype"} · ${frame.name}`;
  const stage = $(".play-stage").getBoundingClientRect();
  const scale = Math.min(1, (stage.width - 80) / frame.width, (stage.height - 80) / frame.height);
  iframe.style.width = `${Math.round(frame.width * scale)}px`;
  iframe.style.height = `${Math.round(frame.height * scale)}px`;
  iframe.onload = () => wirePlay(frame);
}

function wirePlay(frame) {
  const doc = $("#playFrame").contentDocument;
  if (!doc) return;
  doc.addEventListener("click", (event) => {
    const target = event.target.closest("[data-omapaper-on]");
    if (!target) return;
    event.preventDefault();
    const action = target.getAttribute("data-omapaper-action") || "";
    if (action.startsWith("frame:")) {
      const id = action.slice(6).trim();
      const next = state.frames.find((f) => f.id === id);
      if (next) {
        state.playStack.push(frame.id);
        playShow(next);
      }
    } else if (action === "back") {
      playBack();
    } else if (action.startsWith("url:")) {
      window.open(action.slice(4), "_blank");
    }
  });
}

function playBack() {
  const previous = state.playStack.pop();
  if (!previous) return;
  const frame = state.frames.find((f) => f.id === previous);
  if (frame) playShow(frame);
}

function closePlay() {
  $("#playOverlay").hidden = true;
  $("#playFrame").src = "about:blank";
}

// ---- bindings -------------------------------------------------------------

function bind() {
  $("#canvasSelect").onchange = (event) => loadCanvas(event.target.value);
  $("#newCanvasBtn").onclick = newCanvas;
  $("#renameCanvas").onclick = renameCanvas;
  $("#newFrameBtn").onclick = newFrame;
  $("#duplicateCanvasBtn").onclick = duplicateCanvas;
  const closeDrawers = () => { document.body.classList.remove("rail-open", "panel-open"); const b = $("#drawerBackdrop"); if (b) b.hidden = true; };
  const openDrawer = (name) => {
    closeDrawers();
    document.body.classList.add(name === "rail" ? "rail-open" : "panel-open");
    const b = $("#drawerBackdrop"); if (b) b.hidden = false;
  };
  const railToggle = $("#railToggle");
  if (railToggle) railToggle.onclick = () => document.body.classList.contains("rail-open") ? closeDrawers() : openDrawer("rail");
  const panelToggle = $("#panelToggle");
  if (panelToggle) panelToggle.onclick = () => document.body.classList.contains("panel-open") ? closeDrawers() : openDrawer("panel");
  const backdrop = $("#drawerBackdrop");
  if (backdrop) backdrop.onclick = closeDrawers;
  $("#stageWrap").addEventListener("click", closeDrawers);
  $("#newPageBtn").onclick = newPage;
  $("#importFrameBtn").onclick = () => $("#importFile").click();
  $("#frameSearch").oninput = (event) => {
    state.frameFilter = event.target.value.trim().toLowerCase();
    renderRail();
  };
  $("#importFile").onchange = async (event) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !state.canvasId) return;
    try {
      const html = await file.text();
      await api(`/api/canvases/${state.canvasId}/frames`, {
        method: "POST",
        body: JSON.stringify({ name: file.name.replace(/\.[^.]+$/, ""), html }),
      });
      toast("Imported");
      await loadCanvas(state.canvasId);
    } catch (error) {
      toast(error.message, true);
    }
  };
  $("#saveFrame").onclick = saveFrame;
  $("#saveHtml").onclick = saveHtml;
  $("#undoFrame").onclick = undoFrame;
  $("#redoFrame").onclick = redoFrame;
  $("#duplicateFrame").onclick = duplicateFrame;
  $("#deleteFrame").onclick = deleteFrame;
  $("#addComment").onclick = addComment;
  $("#playBtn").onclick = () => openPlay();
  $("#playClose").onclick = closePlay;
  $("#playBack").onclick = playBack;

  $("#zoomIn").onclick = () => {
    state.zoom += 0.1;
    applyZoom();
  };
  $("#zoomOut").onclick = () => {
    state.zoom -= 0.1;
    applyZoom();
  };
  $("#zoomLabel").onclick = () => {
    state.zoom = 1;
    applyZoom();
  };

  $("#themeSelect").onchange = (event) => {
    state.viewTheme = event.target.value;
    store.set("theme", state.viewTheme);
    for (const frame of state.frames) {
      const element = frameElements.get(frame.id);
      const iframe = element?.querySelector("iframe");
      if (iframe) {
        iframe.dataset.src = "";
        frameElement(frame);
      }
    }
    toast(state.viewTheme ? `Previewing ${state.viewTheme}` : "Following Omarchy");
  };

  for (const button of $$(".tab")) {
    button.onclick = () => setTab(button.dataset.tab);
  }

  $("#commentBody").addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) addComment();
  });

  document.addEventListener("keydown", (event) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || "");
    if (event.key === "Escape") {
      if (!$("#playOverlay").hidden) closePlay();
      else if (state.selectedNode) {
        state.selectedNode = null;
        renderPanel();
      }
      return;
    }
    if (typing) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" && !event.shiftKey) {
      event.preventDefault();
      undoFrame();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "y" || (event.shiftKey && event.key.toLowerCase() === "z"))) {
      event.preventDefault();
      redoFrame();
      return;
    }
    if (event.key === "n") {
      event.preventDefault();
      newFrame();
    } else if (event.key === "p") {
      event.preventDefault();
      openPlay();
    } else if (event.key === "0") {
      state.zoom = 1;
      applyZoom();
    } else if (event.key === "+" || event.key === "=") {
      state.zoom += 0.1;
      applyZoom();
    } else if (event.key === "-") {
      state.zoom -= 0.1;
      applyZoom();
    }
  });

  // Cursor over the empty stage still says "here".
  $("#stageWrap").addEventListener("mousemove", (event) => {
    const now = Date.now();
    if (now - state.lastCursor < 200) return;
    state.lastCursor = now;
    const layer = $("#cursorLayer").getBoundingClientRect();
    sendSocket({ type: "cursor", x: Math.round(event.clientX - layer.left), y: Math.round(event.clientY - layer.top) });
  });

  $("#shareBtn").onclick = async () => {
    const info = await api("/api/network").catch(() => null);
    const address = info?.addresses?.[0];
    const port = info?.port || location.port || 4310;
    const base = address ? `http://${address}:${port}` : `${location.protocol}//${location.host}`;
    const url = new URL(base);
    url.pathname = location.pathname;
    if (state.canvasId) url.searchParams.set("canvas", state.canvasId);
    url.searchParams.set("readonly", "1");
    try {
      await navigator.clipboard.writeText(url.toString());
      toast(address ? `LAN link copied (${address})` : "View-only link copied");
    } catch {
      toast(url.toString());
    }
  };

  if (state.readonly) {
    document.body.classList.add("readonly");
    for (const selector of ["#newCanvasBtn", "#renameCanvas", "#newFrameBtn", "#importFrameBtn", "#newPageBtn", "#duplicateCanvasBtn", "#saveFrame", "#saveHtml", "#undoFrame", "#redoFrame", "#duplicateFrame", "#deleteFrame", "#addComment"]) {
      const element = $(selector);
      if (element) element.hidden = true;
    }
    const editor = $("#htmlEditor");
    if (editor) editor.readOnly = true;
    for (const input of ["#frameName", "#frameWidth", "#frameHeight", "#frameBg"]) {
      const element = $(input);
      if (element) element.disabled = true;
    }
    const tag = document.querySelector(".tag");
    if (tag) tag.textContent = "read-only";
  }

  document.addEventListener("click", (event) => {
    if (event.target.closest(".nav-item, .page-nav button")) closeDrawers();
  });
  document.addEventListener("mousemove", applyResize);
  document.addEventListener("mouseup", finishResize);

  window.addEventListener("beforeunload", () => {
    if (state.ws) state.ws.close();
  });
}

boot().catch((error) => {
  console.error(error);
  toast(`OmaPaper failed to start: ${error.message}`, true);
});
