const MAX_ROOMS = 1000;
const MAX_EXITS = 256;
const MAX_DIAGNOSTICS = 100;
const MAX_RECORDS = 100;
const MAX_PREVIEW_OUTPUT = 65536;

const state = {
  snapshot: null,
  selectedRoomId: null,
  dirty: false,
  validation: null,
  artifacts: [],
  selectedArtifact: null,
  preview: null,
  publications: [],
  pendingPublication: null,
  previewPoll: null,
};

const element = (id) => document.getElementById(id);

class ApiError extends Error {
  constructor(status, payload) {
    const safeMessage =
      payload && typeof payload.message === "string"
        ? payload.message.slice(0, 256)
        : `Request failed (${status})`;
    super(safeMessage);
    this.status = status;
    this.payload = payload || {};
  }
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    method: options.method || "GET",
    headers: options.headers || {},
    body: options.body,
    credentials: "same-origin",
  });
  if (response.status === 204) {
    if (!response.ok) {
      throw new ApiError(response.status, {});
    }
    return null;
  }
  let payload = null;
  try {
    payload = await response.json();
  } catch {
    if (response.ok) {
      throw new ApiError(502, { message: "The server returned an invalid response." });
    }
  }
  if (!response.ok) {
    throw new ApiError(response.status, payload);
  }
  return payload;
}

function jsonRequest(path, method, value) {
  return api(path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(value),
  });
}

function announce(message) {
  const safe = String(message).slice(0, 300);
  element("live-status").textContent = "";
  window.setTimeout(() => {
    element("live-status").textContent = safe;
  }, 20);
}

function shortDigest(value) {
  return typeof value === "string" && value.length >= 12 ? value.slice(0, 12) : "—";
}

function currentRoom() {
  return state.snapshot?.draft.rooms.find((room) => room.id === state.selectedRoomId) || null;
}

function selectedArtifact() {
  return (
    state.artifacts.find(
      (artifact) => artifact.artifact_digest === state.selectedArtifact,
    ) || null
  );
}

function markDirty(message = "Unsaved draft changes") {
  state.dirty = true;
  renderStatus();
  renderWorkflowAvailability();
  announce(message);
}

function validationMatchesSnapshot() {
  return Boolean(
    state.validation &&
      !state.dirty &&
      state.validation.revision === state.snapshot?.revision &&
      state.validation.snapshot_sha256 === state.snapshot?.snapshot_sha256,
  );
}

function validationIsCurrent() {
  return validationMatchesSnapshot() && state.validation.valid;
}

function previewMatchesArtifact(artifact) {
  return Boolean(
    artifact &&
      state.preview?.state === "succeeded" &&
      state.preview.artifact_digest === artifact.artifact_digest,
  );
}

function renderStatus() {
  if (!state.snapshot) {
    return;
  }
  element("revision-status").textContent =
    `Revision ${state.snapshot.revision} · ${shortDigest(state.snapshot.snapshot_sha256)}`;
  element("dirty-status").textContent = state.dirty ? "● Unsaved changes" : "✓ Draft saved";
  if (!state.validation) {
    element("validation-status").textContent = "◇ Not validated";
  } else if (!validationMatchesSnapshot()) {
    element("validation-status").textContent = "△ Validation stale";
  } else if (!state.validation.valid) {
    element("validation-status").textContent = "✕ Validation errors";
  } else {
    element("validation-status").textContent = "✓ Snapshot valid";
  }
  const previewState = state.preview?.state || "idle";
  const stale = state.preview?.stale ? " · STALE" : "";
  element("preview-chip").textContent = `□ Preview ${previewState}${stale}`;
}

function renderRoomList() {
  const list = element("room-list");
  list.replaceChildren();
  const rooms = state.snapshot?.draft.rooms.slice(0, MAX_ROOMS) || [];
  const query = element("room-search").value.trim().toLocaleLowerCase();
  for (const room of rooms) {
    if (
      query &&
      !room.id.toLocaleLowerCase().includes(query) &&
      !room.title.toLocaleLowerCase().includes(query)
    ) {
      continue;
    }
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = `${room.title || "Untitled"} — ${room.id}`;
    button.setAttribute(
      "aria-current",
      room.id === state.selectedRoomId ? "true" : "false",
    );
    button.addEventListener("click", () => {
      state.selectedRoomId = room.id;
      renderEditor();
      element("room-title").focus();
    });
    item.append(button);
    list.append(item);
  }
}

function renderRoomOptions() {
  const entry = element("entry-room");
  const previous = state.snapshot.draft.entry_room;
  entry.replaceChildren();
  for (const room of state.snapshot.draft.rooms.slice(0, MAX_ROOMS)) {
    const option = document.createElement("option");
    option.value = room.id;
    option.textContent = `${room.title || "Untitled"} — ${room.id}`;
    option.selected = room.id === previous;
    entry.append(option);
  }
}

function renderExits() {
  const container = element("exit-list");
  container.replaceChildren();
  const room = currentRoom();
  if (!room) {
    return;
  }
  const entries = Object.entries(room.exits).slice(0, MAX_EXITS);
  entries.forEach(([label, target], index) => {
    const row = document.createElement("div");
    row.className = "exit-row";

    const labelField = document.createElement("div");
    labelField.className = "field";
    const labelLabel = document.createElement("label");
    labelLabel.htmlFor = `exit-label-${index}`;
    labelLabel.textContent = "Exit label";
    const labelInput = document.createElement("input");
    labelInput.id = `exit-label-${index}`;
    labelInput.value = label;
    labelInput.maxLength = 64;
    labelInput.addEventListener("input", () => {
      const replacement = labelInput.value;
      const duplicate = replacement !== label && Object.hasOwn(room.exits, replacement);
      labelInput.setCustomValidity(
        !replacement ? "Exit label is required." : duplicate ? "Exit labels must be unique." : "",
      );
      if (!replacement || duplicate) {
        markDirty();
        return;
      }
      const rebuilt = {};
      for (const [existingLabel, existingTarget] of Object.entries(room.exits)) {
        rebuilt[existingLabel === label ? replacement : existingLabel] = existingTarget;
      }
      room.exits = rebuilt;
      label = replacement;
      markDirty();
    });
    labelField.append(labelLabel, labelInput);

    const targetField = document.createElement("div");
    targetField.className = "field";
    const targetLabel = document.createElement("label");
    targetLabel.htmlFor = `exit-target-${index}`;
    targetLabel.textContent = "Destination";
    const targetSelect = document.createElement("select");
    targetSelect.id = `exit-target-${index}`;
    for (const destination of state.snapshot.draft.rooms.slice(0, MAX_ROOMS)) {
      const option = document.createElement("option");
      option.value = destination.id;
      option.textContent = `${destination.title || "Untitled"} — ${destination.id}`;
      option.selected = destination.id === target;
      targetSelect.append(option);
    }
    if (!state.snapshot.draft.rooms.some((candidate) => candidate.id === target)) {
      const missing = document.createElement("option");
      missing.value = target;
      missing.textContent = `Missing — ${target}`;
      missing.selected = true;
      targetSelect.prepend(missing);
    }
    targetSelect.addEventListener("change", () => {
      room.exits[label] = targetSelect.value;
      markDirty();
    });
    targetField.append(targetLabel, targetSelect);

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "danger-outline";
    remove.textContent = "Remove exit";
    remove.setAttribute("aria-label", `Remove exit ${label}`);
    remove.addEventListener("click", () => {
      delete room.exits[label];
      markDirty(`Removed exit ${label}`);
      renderExits();
    });
    row.append(labelField, targetField, remove);
    container.append(row);
  });
}

function renderEditor() {
  if (!state.snapshot) {
    return;
  }
  if (!currentRoom()) {
    state.selectedRoomId = state.snapshot.draft.rooms[0]?.id || null;
  }
  renderRoomList();
  renderRoomOptions();
  const room = currentRoom();
  const disabled = !room;
  for (const id of [
    "room-id",
    "room-title",
    "room-description",
    "allow-unreachable",
    "delete-room",
    "add-exit",
  ]) {
    element(id).disabled = disabled;
  }
  element("namespace").value = state.snapshot.draft.namespace;
  if (room) {
    element("room-id").value = room.id;
    element("room-title").value = room.title;
    element("room-description").value = room.description;
    element("allow-unreachable").checked = Boolean(room.allow_unreachable);
  } else {
    element("room-id").value = "";
    element("room-title").value = "";
    element("room-description").value = "";
    element("allow-unreachable").checked = false;
  }
  renderExits();
  renderStatus();
  renderWorkflowAvailability();
}

function renderDiagnostics(validation = state.validation) {
  const list = element("diagnostic-list");
  list.replaceChildren();
  if (!validation) {
    element("diagnostic-summary").textContent = "Not validated";
    return;
  }
  const diagnostics = Array.isArray(validation.diagnostics)
    ? validation.diagnostics.slice(0, MAX_DIAGNOSTICS)
    : [];
  element("diagnostic-summary").textContent = validation.valid
    ? `Valid snapshot · ${validation.room_count ?? 0} rooms · ${validation.exit_count ?? 0} exits`
    : `${diagnostics.length} error${diagnostics.length === 1 ? "" : "s"}`;
  for (const diagnostic of diagnostics) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent =
      `${diagnostic.severity || "error"} · ${diagnostic.code || "unknown"} · ` +
      `${diagnostic.location || "document"} — ${diagnostic.message || "Invalid content"}`;
    button.addEventListener("click", () => focusDiagnostic(diagnostic.location || ""));
    item.append(button);
    list.append(item);
  }
}

function focusDiagnostic(location) {
  const roomMatch =
    location.match(/rooms(?:\[|\/)(\d+)/) || location.match(/rooms\.(\d+)/);
  if (roomMatch) {
    const room = state.snapshot.draft.rooms[Number(roomMatch[1])];
    if (room) {
      state.selectedRoomId = room.id;
      renderEditor();
    }
  }
  let target = "diagnostics-heading";
  if (location.includes("namespace")) target = "namespace";
  else if (location.includes("entry_room")) target = "entry-room";
  else if (location.includes("title")) target = "room-title";
  else if (location.includes("description")) target = "room-description";
  else if (location.includes("exits")) target = "exit-list";
  element(target)?.focus();
}

function renderArtifacts() {
  const select = element("artifact-select");
  select.replaceChildren();
  const records = state.artifacts.slice(0, MAX_RECORDS);
  if (!records.length) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No packaged artifacts";
    select.append(option);
    state.selectedArtifact = null;
  } else {
    if (!records.some((item) => item.artifact_digest === state.selectedArtifact)) {
      state.selectedArtifact = records[0].artifact_digest;
    }
    for (const record of records) {
      const option = document.createElement("option");
      option.value = record.artifact_digest;
      option.textContent =
        `r${record.draft_revision} · ${shortDigest(record.artifact_digest)} · ${record.release_id}`;
      option.selected = option.value === state.selectedArtifact;
      select.append(option);
    }
  }
  const artifact = selectedArtifact();
  element("artifact-identity").textContent = artifact
    ? `Artifact ${artifact.artifact_digest} · pinned revision ${artifact.draft_revision}`
    : "No artifact selected";
  element("publish-artifact").textContent = artifact
    ? `Exact artifact ${shortDigest(artifact.artifact_digest)}, revision ${artifact.draft_revision}.`
    : "A successful preview of the selected artifact is required.";
  renderWorkflowAvailability();
}

function renderPreview() {
  const preview = state.preview;
  if (!preview) {
    return;
  }
  element("preview-output").textContent = String(preview.output || "").slice(
    0,
    MAX_PREVIEW_OUTPUT,
  );
  const identity = preview.artifact_digest
    ? `${shortDigest(preview.artifact_digest)} · revision ${preview.draft_revision}`
    : "No artifact is running.";
  element("preview-pinned").textContent =
    `${identity} · ${preview.state}${preview.stale ? " · STALE SNAPSHOT" : ""}` +
    `${preview.truncated ? " · output truncated" : ""}`;
  renderStatus();
  renderWorkflowAvailability();
  if (preview.state === "running") {
    startPreviewPolling();
  } else {
    stopPreviewPolling();
  }
}

function statusLabel(status) {
  const labels = {
    requested: "Requested — not yet activated",
    approved: "Approved — awaiting activation",
    rejected: "Rejected",
    activating: "Activation in progress",
    activated: "Activated",
    failed: "Activation failed",
  };
  return labels[status] || `Unknown status: ${status}`;
}

function renderPublications() {
  const list = element("publication-list");
  list.replaceChildren();
  for (const record of state.publications.slice(0, MAX_RECORDS)) {
    const item = document.createElement("li");
    const status = document.createElement("p");
    status.textContent = statusLabel(record.status);
    const identity = document.createElement("p");
    identity.className = "digest";
    identity.textContent =
      `${record.target} · ${shortDigest(record.artifact_digest)} · request ${record.request_id}`;
    const refresh = document.createElement("button");
    refresh.type = "button";
    refresh.className = "quiet-button";
    refresh.textContent = "Refresh status";
    refresh.addEventListener("click", () => refreshPublication(record.request_id, refresh));
    item.append(status, identity, refresh);
    list.append(item);
  }
}

function renderWorkflowAvailability() {
  const cleanValidation = validationIsCurrent();
  element("package-artifact").disabled = !cleanValidation;
  element("package-note").textContent = cleanValidation
    ? `Validated revision ${state.validation.revision} is ready to package.`
    : "Packaging requires a clean, valid saved revision.";
  const artifact = selectedArtifact();
  element("preview-start").disabled = !artifact || state.preview?.state === "running";
  element("preview-stop").disabled = state.preview?.state !== "running";
  element("preview-send").disabled = state.preview?.state !== "running";
  const canPublish = previewMatchesArtifact(artifact);
  element("submit-publication").disabled = !canPublish;
  element("publication-note").textContent = canPublish
    ? "Ready to submit a review request. Submission is not activation."
    : "Preview the selected artifact successfully before requesting publication.";
}

async function loadWorkspace({ discard = false } = {}) {
  const snapshot = await api("/api/v1/workspace");
  if (!snapshot?.draft || !Array.isArray(snapshot.draft.rooms)) {
    throw new Error("Workspace response is invalid.");
  }
  state.snapshot = snapshot;
  state.selectedRoomId =
    snapshot.draft.rooms.find((room) => room.id === state.selectedRoomId)?.id ||
    snapshot.draft.entry_room ||
    snapshot.draft.rooms[0]?.id ||
    null;
  state.dirty = false;
  state.validation = null;
  element("conflict-panel").hidden = true;
  renderDiagnostics();
  renderEditor();
  if (discard) announce(`Reloaded saved revision ${snapshot.revision}`);
}

async function loadArtifacts() {
  try {
    const records = await api("/api/v1/artifacts");
    state.artifacts = Array.isArray(records)
      ? records.slice(0, MAX_RECORDS).sort((a, b) => b.created_at - a.created_at)
      : [];
    renderArtifacts();
  } catch (error) {
    announce(error.message);
  }
}

async function loadPreview() {
  try {
    state.preview = await api("/api/v1/preview");
    renderPreview();
  } catch (error) {
    announce(error.message);
  }
}

async function loadPublications() {
  try {
    const records = await api("/api/v1/publications");
    state.publications = Array.isArray(records) ? records.slice(0, MAX_RECORDS) : [];
    renderPublications();
  } catch (error) {
    announce(error.message);
  }
}

function openDialog(dialog, initialFocus, opener) {
  dialog.dataset.opener = opener?.id || "";
  dialog.showModal();
  window.setTimeout(() => initialFocus?.focus(), 0);
}

function restoreDialogFocus(dialog) {
  const opener = dialog.dataset.opener ? element(dialog.dataset.opener) : null;
  opener?.focus();
}

function selectRoomAndExit(roomId, label) {
  state.selectedRoomId = roomId;
  renderEditor();
  const entries = Object.keys(currentRoom()?.exits || {});
  const index = entries.indexOf(label);
  (index >= 0 ? element(`exit-target-${index}`) : element("room-title"))?.focus();
}

function openDeleteDialog() {
  const room = currentRoom();
  if (!room) return;
  const incoming = [];
  for (const source of state.snapshot.draft.rooms) {
    for (const [label, target] of Object.entries(source.exits)) {
      if (target === room.id) incoming.push({ source: source.id, label });
    }
  }
  const outgoing = Object.entries(room.exits);
  const isEntry = state.snapshot.draft.entry_room === room.id;
  const impact = element("delete-impact");
  impact.replaceChildren();
  const summary = document.createElement("p");
  summary.textContent =
    `${incoming.length} incoming reference${incoming.length === 1 ? "" : "s"}, ` +
    `${outgoing.length} outgoing exit${outgoing.length === 1 ? "" : "s"}, ` +
    `${isEntry ? "and this is the entry room" : "not the entry room"}.`;
  impact.append(summary);
  if (incoming.length) {
    const heading = document.createElement("strong");
    heading.textContent = "Fix incoming references before deletion:";
    const list = document.createElement("ul");
    for (const reference of incoming) {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = `${reference.source} via “${reference.label}”`;
      button.addEventListener("click", () => {
        element("delete-room-dialog").close();
        selectRoomAndExit(reference.source, reference.label);
      });
      item.append(button);
      list.append(item);
    }
    impact.append(heading, list);
  }
  if (outgoing.length) {
    const outgoingText = document.createElement("p");
    outgoingText.textContent =
      `Outgoing exits removed with this room: ${outgoing
        .map(([label, target]) => `${label} → ${target}`)
        .join("; ")}`;
    impact.append(outgoingText);
  }
  if (isEntry) {
    const entryWarning = document.createElement("p");
    entryWarning.textContent = "Choose a different entry room before deleting this room.";
    impact.append(entryWarning);
  }
  const blocked = incoming.length > 0 || isEntry;
  element("delete-room-confirmation").value = "";
  element("delete-room-confirmation").disabled = blocked;
  element("confirm-delete-room").disabled = true;
  element("delete-room-error").textContent = blocked
    ? "Deletion is blocked until entry and incoming references are resolved."
    : `Type ${room.id} exactly to enable deletion.`;
  const dialog = element("delete-room-dialog");
  dialog.dataset.roomId = room.id;
  openDialog(
    dialog,
    blocked ? dialog.querySelector("[data-dialog-cancel]") : element("delete-room-confirmation"),
    element("delete-room"),
  );
}

async function submitPublication(data) {
  try {
    element("submit-publication").disabled = true;
    const view = await jsonRequest("/api/v1/publications", "POST", data);
    state.publications = [
      view,
      ...state.publications.filter((item) => item.request_id !== view.request_id),
    ].slice(0, MAX_RECORDS);
    renderPublications();
    element("publication-note").textContent = "Requested — not yet approved or activated.";
    announce("Publication review requested; activation has not occurred.");
  } catch (error) {
    element("publication-note").textContent = error.message;
    announce(error.message);
  } finally {
    state.pendingPublication = null;
    renderWorkflowAvailability();
  }
}

function continuePublication() {
  const pending = state.pendingPublication;
  if (!pending) return;
  if (pending.target === "production") {
    const phrase = `production:${pending.artifact_digest.slice(0, 12)}`;
    element("production-phrase").textContent = phrase;
    element("production-confirmation").value = "";
    element("production-error").textContent = "";
    element("production-copy").textContent =
      `Target production with artifact ${pending.artifact_digest}.`;
    openDialog(
      element("production-dialog"),
      element("production-confirmation"),
      element("submit-publication"),
    );
  } else {
    pending.production_confirmation = null;
    void submitPublication(pending);
  }
}

async function refreshPublication(requestId, button) {
  button.disabled = true;
  try {
    const view = await jsonRequest(
      `/api/v1/publications/${encodeURIComponent(requestId)}`,
      "POST",
      {},
    );
    state.publications = state.publications.map((record) =>
      record.request_id === view.request_id ? view : record,
    );
    renderPublications();
    announce(statusLabel(view.status));
  } catch (error) {
    announce(error.message);
  } finally {
    button.disabled = false;
  }
}

function startPreviewPolling() {
  if (state.previewPoll !== null) return;
  state.previewPoll = window.setInterval(() => void loadPreview(), 1000);
}

function stopPreviewPolling() {
  if (state.previewPoll !== null) {
    window.clearInterval(state.previewPoll);
    state.previewPoll = null;
  }
}

element("room-search").addEventListener("input", renderRoomList);

element("namespace").addEventListener("input", () => {
  state.snapshot.draft.namespace = element("namespace").value;
  markDirty();
});

element("entry-room").addEventListener("change", () => {
  state.snapshot.draft.entry_room = element("entry-room").value;
  markDirty();
});

element("room-title").addEventListener("input", () => {
  const room = currentRoom();
  if (room) {
    room.title = element("room-title").value;
    markDirty();
    renderRoomList();
    renderRoomOptions();
  }
});

element("room-description").addEventListener("input", () => {
  const room = currentRoom();
  if (room) {
    room.description = element("room-description").value;
    markDirty();
  }
});

element("allow-unreachable").addEventListener("change", () => {
  const room = currentRoom();
  if (room) {
    room.allow_unreachable = element("allow-unreachable").checked;
    markDirty();
  }
});

element("add-exit").addEventListener("click", () => {
  const room = currentRoom();
  if (!room || Object.keys(room.exits).length >= MAX_EXITS) {
    announce("Exit limit reached.");
    return;
  }
  let label = "new-exit";
  for (let suffix = 2; Object.hasOwn(room.exits, label); suffix += 1) {
    label = `new-exit-${suffix}`;
  }
  room.exits[label] = room.id;
  markDirty(`Added exit ${label}`);
  renderExits();
  element(`exit-label-${Object.keys(room.exits).length - 1}`)?.focus();
});

element("add-room").addEventListener("click", () => {
  element("new-room-id").value = "";
  element("new-room-title").value = "";
  element("add-room-error").textContent = "";
  openDialog(element("add-room-dialog"), element("new-room-id"), element("add-room"));
});

element("add-room-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const id = element("new-room-id").value.trim();
  const title = element("new-room-title").value.trim();
  if (!id || !title || id.length > 256) {
    element("add-room-error").textContent = "A bounded stable ID and title are required.";
    return;
  }
  if (state.snapshot.draft.rooms.some((room) => room.id === id)) {
    element("add-room-error").textContent = "That stable room ID already exists.";
    return;
  }
  if (state.snapshot.draft.rooms.length >= MAX_ROOMS) {
    element("add-room-error").textContent = "Room limit reached.";
    return;
  }
  state.snapshot.draft.rooms.push({
    id,
    title,
    description: "",
    exits: {},
    allow_unreachable: false,
  });
  state.selectedRoomId = id;
  markDirty(`Added room ${id}`);
  element("add-room-dialog").close();
  renderEditor();
  element("room-description").focus();
});

element("delete-room").addEventListener("click", openDeleteDialog);
element("delete-room-confirmation").addEventListener("input", () => {
  const dialog = element("delete-room-dialog");
  const matches = element("delete-room-confirmation").value === dialog.dataset.roomId;
  element("confirm-delete-room").disabled = !matches;
  element("delete-room-error").textContent = matches
    ? "Exact identity confirmed. This still affects only the unsaved draft."
    : `Type ${dialog.dataset.roomId} exactly to enable deletion.`;
});

element("delete-room-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const dialog = element("delete-room-dialog");
  const roomId = dialog.dataset.roomId;
  if (element("delete-room-confirmation").value !== roomId) return;
  const index = state.snapshot.draft.rooms.findIndex((room) => room.id === roomId);
  if (index < 0) return;
  const blocked =
    state.snapshot.draft.entry_room === roomId ||
    state.snapshot.draft.rooms.some((room) =>
      Object.values(room.exits).some((target) => target === roomId),
    );
  if (blocked) {
    element("delete-room-error").textContent = "References changed; review impacts again.";
    return;
  }
  state.snapshot.draft.rooms.splice(index, 1);
  state.selectedRoomId =
    state.snapshot.draft.rooms[Math.min(index, state.snapshot.draft.rooms.length - 1)]?.id || null;
  markDirty(`Deleted room ${roomId} from the unsaved draft`);
  dialog.close();
  renderEditor();
});

for (const cancel of document.querySelectorAll("[data-dialog-cancel]")) {
  cancel.addEventListener("click", () => element(cancel.dataset.dialogCancel).close());
}
for (const dialog of document.querySelectorAll("dialog")) {
  dialog.addEventListener("close", () => restoreDialogFocus(dialog));
}

element("world-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!state.snapshot || !event.currentTarget.reportValidity()) return;
  try {
    const saved = await api("/api/v1/draft", {
      method: "PUT",
      headers: {
        "Content-Type": "application/vnd.worldwright.world+json",
        "X-Worldwright-Draft-Revision": String(state.snapshot.revision),
      },
      body: JSON.stringify(state.snapshot.draft),
    });
    const selected = state.selectedRoomId;
    state.snapshot = saved;
    state.selectedRoomId =
      saved.draft.rooms.find((room) => room.id === selected)?.id ||
      saved.draft.entry_room ||
      saved.draft.rooms[0]?.id ||
      null;
    state.dirty = false;
    state.validation = null;
    element("conflict-panel").hidden = true;
    renderDiagnostics();
    renderEditor();
    announce(`Saved revision ${saved.revision}`);
  } catch (error) {
    if (error.status === 409) {
      element("conflict-panel").hidden = false;
      element("conflict-text").textContent =
        `The workspace is now revision ${error.payload.current_revision ?? "unknown"}.`;
      element("conflict-panel").scrollIntoView({ block: "nearest" });
    }
    announce(error.message);
  }
});

element("reload-draft").addEventListener("click", () => {
  void loadWorkspace({ discard: true });
});

element("import-file").addEventListener("change", async () => {
  const file = element("import-file").files?.[0];
  if (!file) return;
  try {
    const text = await file.text();
    const result = await api("/api/v1/import", {
      method: "POST",
      headers: { "Content-Type": "application/vnd.worldwright.world+json" },
      body: text,
    });
    state.snapshot.draft = result.draft;
    state.selectedRoomId =
      result.draft.entry_room || result.draft.rooms[0]?.id || null;
    state.validation = null;
    markDirty(`Imported ${file.name}; save to create a revision`);
    renderDiagnostics(result.validation);
    renderEditor();
  } catch (error) {
    announce(error.message);
  } finally {
    element("import-file").value = "";
  }
});

element("export-draft").addEventListener("click", () => {
  if (state.snapshot) {
    window.location.assign(`/api/v1/revisions/${state.snapshot.revision}/export`);
  }
});

element("validate-draft").addEventListener("click", async () => {
  try {
    const result = await jsonRequest(
      `/api/v1/revisions/${state.snapshot.revision}/validate`,
      "POST",
      {},
    );
    state.validation = {
      ...result,
      revision: state.snapshot.revision,
    };
    renderDiagnostics(state.validation);
    renderStatus();
    renderWorkflowAvailability();
    announce(result.valid ? "Saved snapshot is valid." : "Validation found errors.");
  } catch (error) {
    announce(error.message);
  }
});

element("package-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!validationIsCurrent() || !event.currentTarget.reportValidity()) return;
  try {
    const record = await jsonRequest("/api/v1/package", "POST", {
      revision: state.snapshot.revision,
      snapshot_sha256: state.snapshot.snapshot_sha256,
      release_id: element("release-id").value,
      builder_identity: element("builder-identity").value,
      game: element("package-game").value,
    });
    state.artifacts = [
      record,
      ...state.artifacts.filter(
        (artifact) => artifact.artifact_digest !== record.artifact_digest,
      ),
    ].slice(0, MAX_RECORDS);
    state.selectedArtifact = record.artifact_digest;
    renderArtifacts();
    announce(`Packaged artifact ${shortDigest(record.artifact_digest)}`);
  } catch (error) {
    element("package-note").textContent = error.message;
    announce(error.message);
  }
});

element("artifact-select").addEventListener("change", () => {
  state.selectedArtifact = element("artifact-select").value || null;
  renderArtifacts();
  renderPreview();
});

element("preview-start").addEventListener("click", async () => {
  const artifact = selectedArtifact();
  if (!artifact) return;
  try {
    state.preview = await jsonRequest("/api/v1/preview/start", "POST", {
      artifact_digest: artifact.artifact_digest,
      max_seconds: Number(element("preview-seconds").value),
    });
    renderPreview();
    announce("Preview started with the selected immutable artifact.");
  } catch (error) {
    announce(error.message);
  }
});

element("preview-input-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = element("preview-input").value;
  if (!text) return;
  try {
    await jsonRequest("/api/v1/preview/input", "POST", { text });
    element("preview-input").value = "";
  } catch (error) {
    announce(error.message);
  }
});

element("preview-stop").addEventListener("click", async () => {
  try {
    state.preview = await jsonRequest("/api/v1/preview/stop", "POST", {});
    renderPreview();
    announce(`Preview ${state.preview.state}.`);
  } catch (error) {
    announce(error.message);
  }
});

element("publish-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const artifact = selectedArtifact();
  if (!artifact || !previewMatchesArtifact(artifact) || !event.currentTarget.reportValidity()) {
    announce("A successful preview of the selected artifact is required.");
    return;
  }
  const expected = element("expected-current").value.trim();
  state.pendingPublication = {
    artifact_digest: artifact.artifact_digest,
    kind: element("publication-kind").value,
    target: element("publication-target").value,
    expected_current: expected || null,
    reason: element("publication-reason").value,
    change_summary: element("change-summary").value,
    production_confirmation: null,
    allow_stale: false,
  };
  const stale =
    artifact.draft_revision !== state.snapshot.revision ||
    artifact.payload_digest !== state.snapshot.snapshot_sha256;
  if (stale) {
    element("stale-copy").textContent =
      `Artifact revision ${artifact.draft_revision} differs from current saved revision ` +
      `${state.snapshot.revision}.`;
    openDialog(
      element("stale-artifact-dialog"),
      element("confirm-stale-artifact"),
      element("submit-publication"),
    );
  } else {
    continuePublication();
  }
});

element("stale-artifact-form").addEventListener("submit", (event) => {
  event.preventDefault();
  state.pendingPublication.allow_stale = true;
  element("stale-artifact-dialog").close();
  continuePublication();
});

element("production-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const expected = element("production-phrase").textContent;
  if (element("production-confirmation").value !== expected) {
    element("production-error").textContent = "The confirmation phrase does not match.";
    return;
  }
  state.pendingPublication.production_confirmation = expected;
  element("production-dialog").close();
  void submitPublication(state.pendingPublication);
});

window.addEventListener("beforeunload", (event) => {
  if (state.dirty) {
    event.preventDefault();
    event.returnValue = "";
  }
});

async function initialize() {
  try {
    await loadWorkspace();
    await Promise.all([loadArtifacts(), loadPreview(), loadPublications()]);
    renderStatus();
    renderWorkflowAvailability();
    announce("Worldwright Studio workspace loaded.");
  } catch (error) {
    announce(error.message || "Studio failed to load.");
  }
}

void initialize();
