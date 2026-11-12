let MAX_ROOMS = 0;
let MAX_EXITS = 0;
let capabilities = null;
const utf8Bytes = (value) => new TextEncoder().encode(value).length;

function checkByteLimit(input, limit) {
  input.setCustomValidity(utf8Bytes(input.value) > limit ? `Maximum ${limit} UTF-8 bytes.` : "");
}

async function loadCapabilities() {
  const value = await api("/api/v1/capabilities");
  if (value?.schema_version !== 1 ||
      !["max_rooms", "max_exits_per_room", "max_text_bytes", "max_definition_bytes", "max_exit_label_bytes", "max_input_bytes"]
        .every((key) => Number.isSafeInteger(value[key]) && value[key] > 0)) {
    throw new Error("Unsupported Studio capabilities; editor disabled.");
  }
  capabilities = value;
  MAX_ROOMS = value.max_rooms;
  MAX_EXITS = value.max_exits_per_room;
  for (const [id, limit] of [
    ["namespace", value.max_definition_bytes], ["new-room-id", value.max_definition_bytes],
    ["room-title", value.max_text_bytes], ["new-room-title", value.max_text_bytes],
    ["room-description", value.max_text_bytes],
    ["builder-identity", 128], ["publication-reason", 512], ["change-summary", 1024],
    ["preview-input", 1024],
  ]) {
    const input = element(id);
    input.maxLength = limit;
    input.addEventListener("input", () => checkByteLimit(input, limit));
  }
  element("exit-help").textContent = `At most ${MAX_EXITS} exits per room; labels at most ${value.max_exit_label_bytes} UTF-8 bytes. Destinations must be distinct, existing, non-self rooms.`;
}
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
  editEpoch: 0,
  snapshotEpoch: 0,
  tickets: new Map(),
  busy: new Set(),
  previewReading: false,
  artifactIdentity: null,
  publicationAuth: null,
  publicationNotice: null,
  recovery: [],
};

function issue(name) {
  const value = (state.tickets.get(name) || 0) + 1;
  state.tickets.set(name, value);
  return value;
}
function currentTicket(name, ticket) { return state.tickets.get(name) === ticket; }
function stamp() {
  return { edit: state.editEpoch, snapshot: state.snapshotEpoch,
    revision: state.snapshot?.revision, digest: state.snapshot?.snapshot_sha256 };
}
function sameStamp(value) {
  const now = stamp();
  return Object.keys(now).every(key => now[key] === value[key]);
}
function beginOperation(name) {
  if (state.busy.has(name)) return false;
  state.busy.add(name);
  renderWorkflowAvailability();
  return true;
}
function endOperation(name) {
  state.busy.delete(name);
  renderWorkflowAvailability();
}
function validSnapshot(value) {
  return value && Number.isSafeInteger(value.revision) && value.revision > 0 &&
    /^[0-9a-f]{64}$/.test(value.snapshot_sha256) && value.draft &&
    Array.isArray(value.draft.rooms) && value.draft.rooms.length <= MAX_ROOMS;
}


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
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 30000);
  try {
    const response = await fetch(path, {
      method: options.method || "GET", headers: options.headers || {}, body: options.body,
      credentials: "same-origin", redirect: "error", signal: controller.signal,
    });
    if (response.status === 204 && response.ok) return null;
    const maximum = path === "/api/v1/workspace" || path === "/api/v1/draft" || path === "/api/v1/import"
      ? (capabilities?.max_input_bytes || 32 * 1024 * 1024) + 65536 : 1024 * 1024;
    let payload = null;
    try {
      if (response.body?.getReader) {
        const reader = response.body.getReader();
        const decoder = new TextDecoder("utf-8", { fatal: true });
        let total = 0; let text = "";
        for (;;) {
          const part = await reader.read();
          if (part.done) break;
          total += part.value.byteLength;
          if (total > maximum) { await reader.cancel(); throw new Error("Response exceeds bounds"); }
          text += decoder.decode(part.value, { stream: true });
        }
        text += decoder.decode(); payload = JSON.parse(text);
      } else {
        payload = await response.json();
        if (utf8Bytes(JSON.stringify(payload)) > maximum) throw new Error("Response exceeds bounds");
      }
    } catch (error) {
      if (controller.signal.aborted) throw error;
      if (response.ok) throw new ApiError(502, { message: "Invalid or oversized server response." });
    }
    if (!response.ok) throw new ApiError(response.status, payload);
    return payload;
  } catch (error) {
    if (controller.signal.aborted) {
      throw new ApiError(408, { message: "Request timed out. The operation may have completed; refresh or reconcile before retrying." });
    }
    throw error;
  } finally { clearTimeout(timer); }
}

function jsonRequest(path, method, value) {
  return api(path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(value),
  });
}

let announcement = 0;
function announce(message) {
  const ticket = ++announcement;
  const safe = String(message).slice(0, 300);
  element("live-status").textContent = "";
  window.setTimeout(() => {
    if (ticket === announcement) element("live-status").textContent = safe;
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
  state.editEpoch += 1;
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
      state.preview.artifact_digest === artifact.artifact_digest &&
      state.preview.draft_revision === artifact.draft_revision &&
      state.preview.snapshot_sha256 === artifact.payload_digest,
  );
}

function previewIsStale() {
  return Boolean(state.preview?.artifact_digest && (state.preview.stale || state.dirty ||
    state.preview.draft_revision !== state.snapshot?.revision || state.preview.snapshot_sha256 !== state.snapshot?.snapshot_sha256));
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
  const stale = previewIsStale() ? " · STALE" : "";
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
    labelInput.maxLength = capabilities.max_exit_label_bytes;
    labelInput.addEventListener("input", () => {
      const replacement = labelInput.value;
      const duplicate = replacement !== label && Object.hasOwn(room.exits, replacement);
      labelInput.setCustomValidity(
        !replacement ? "Exit label is required." : duplicate ? "Exit labels must be unique." :
          utf8Bytes(replacement) > capabilities.max_exit_label_bytes ? "Exit label exceeds the UTF-8 byte limit." : "",
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
  for (const id of ["room-title", "room-description"]) {
    checkByteLimit(element(id), capabilities.max_text_bytes);
  }
  checkByteLimit(element("namespace"), capabilities.max_definition_bytes);
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
  const identity = state.artifactIdentity?.artifact_digest === artifact?.artifact_digest ? state.artifactIdentity : null;
  element("artifact-identity").textContent = artifact
    ? `Artifact ${artifact.artifact_digest} · pinned revision ${artifact.draft_revision} · game ${identity?.game || "not yet verified"}`
    : "No artifact selected";
  element("publish-artifact").textContent = artifact
    ? `Exact artifact ${shortDigest(artifact.artifact_digest)}, revision ${artifact.draft_revision}, game ${identity?.game || "not yet verified"}.`
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
    `${identity} · ${preview.state}${previewIsStale() ? " · STALE SNAPSHOT" : ""}` +
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
  const cleanValidation = validationIsCurrent() && !state.busy.has("draft");
  element("package-artifact").disabled = !cleanValidation || state.busy.has("package");
  element("package-note").textContent = cleanValidation
    ? `Validated revision ${state.validation.revision} is ready to package.`
    : "Packaging requires a clean, valid saved revision.";
  const artifact = selectedArtifact();
  const confirmed = artifact && state.artifactIdentity?.artifact_digest === artifact.artifact_digest;
  element("preview-start").disabled = !confirmed || state.preview?.state === "running" || state.busy.has("preview");
  element("preview-stop").disabled = state.preview?.state !== "running" || state.busy.has("preview");
  element("preview-send").disabled = state.preview?.state !== "running" || state.busy.has("preview") || state.busy.has("preview-input");
  const canPublish = confirmed && previewMatchesArtifact(artifact) &&
    state.publicationAuth?.configured === true && !state.busy.has("publication");
  element("submit-publication").disabled = !canPublish;
  element("reconcile-publication").disabled = state.publicationAuth?.configured !== true || state.busy.has("publication");
  element("reconcile-form").setAttribute("aria-busy", String(state.busy.has("publication")));
  element("validate-draft").disabled = state.busy.has("validation") || state.busy.has("draft") || !state.snapshot || state.dirty;
  element("reload-draft").disabled = state.busy.has("draft");
  element("import-file").disabled = state.busy.has("draft");
  for (const [id, operation] of [["world-form", "draft"], ["package-form", "package"], ["publish-form", "publication"], ["preview-input-form", "preview-input"]]) {
    element(id).setAttribute("aria-busy", String(state.busy.has(operation)));
  }
  element("publication-note").textContent = state.publicationNotice || (canPublish
    ? "Ready to submit a review request. Submission is not activation."
    : state.publicationAuth?.configured === false ? "Publication is not configured in this Studio process."
    : "Verify and preview the selected artifact successfully before requesting publication.");
}

async function loadWorkspace({ discard = false } = {}) {
  if (!beginOperation("draft")) return;
  const before = stamp();
  try {
    const snapshot = await api("/api/v1/workspace");
    if (!validSnapshot(snapshot)) throw new Error("Workspace response is invalid.");
    if (!sameStamp(before)) { announce("Reload ignored because you edited while it was loading."); return; }
    state.snapshot = snapshot;
    state.snapshotEpoch += 1;
    state.selectedRoomId = snapshot.draft.rooms.find(room => room.id === state.selectedRoomId)?.id || snapshot.draft.entry_room || snapshot.draft.rooms[0]?.id || null;
    state.dirty = false; state.validation = null;
    element("conflict-panel").hidden = true;
    renderDiagnostics(); renderEditor();
    if (discard) announce(`Reloaded saved revision ${snapshot.revision}`);
  } finally { endOperation("draft"); }
}

async function loadArtifacts() {
  const ticket = issue("artifacts");
  try {
    const records = await api("/api/v1/artifacts");
    if (!currentTicket("artifacts", ticket)) return;
    if (!Array.isArray(records) || records.length > MAX_RECORDS) throw new Error("Artifact list is invalid or oversized.");
    state.artifacts = records.sort((a, b) => b.created_at - a.created_at);
    renderArtifacts(); await loadArtifactIdentity();
  } catch (error) { if (currentTicket("artifacts", ticket)) announce(error.message); }
}

async function loadArtifactIdentity() {
  const ticket = issue("artifact-identity");
  const artifact = selectedArtifact();
  state.artifactIdentity = null; renderArtifacts();
  if (!artifact) return;
  try {
    const identity = await api(`/api/v1/artifacts/${encodeURIComponent(artifact.artifact_digest)}/identity`);
    if (!currentTicket("artifact-identity", ticket) || state.selectedArtifact !== artifact.artifact_digest) return;
    if (identity?.schema_version !== 1 || !["reference", "expedition"].includes(identity.game) ||
        identity.artifact_digest !== artifact.artifact_digest || identity.payload_digest !== artifact.payload_digest || identity.draft_revision !== artifact.draft_revision) {
      throw new Error("Artifact game identity does not match the immutable record.");
    }
    state.artifactIdentity = identity; renderArtifacts();
  } catch (error) { if (currentTicket("artifact-identity", ticket)) announce(error.message); }
}

async function loadPreview() {
  if (state.previewReading || state.busy.has("preview")) return;
  state.previewReading = true;
  const ticket = issue("preview");
  try {
    const preview = await api("/api/v1/preview");
    if (!currentTicket("preview", ticket)) return;
    state.preview = preview; renderPreview();
  } catch (error) { if (currentTicket("preview", ticket)) announce(error.message); }
  finally { state.previewReading = false; }
}

async function loadPublications() {
  const ticket = issue("publications");
  try {
    const records = await api("/api/v1/publications");
    if (!currentTicket("publications", ticket)) return;
    if (!Array.isArray(records) || records.length > MAX_RECORDS) throw new Error("Publication list is invalid or oversized.");
    state.publications = records; renderPublications();
  } catch (error) { if (currentTicket("publications", ticket)) announce(error.message); }
}

async function loadPublicationSupport() {
  const ticket = issue("publication-support");
  try {
    const auth = await api("/api/v1/publication-auth");
    if (!currentTicket("publication-support", ticket)) return;
    if (auth?.schema_version !== 1 || typeof auth.configured !== "boolean" || auth.automatic_retry !== false || !["token_file", "restart_with_token_env", "disabled"].includes(auth.renewal)) throw new Error("Invalid publication configuration response.");
    state.publicationAuth = auth;
    element("publication-auth-note").textContent = auth.renewal === "token_file"
      ? "Token-file renewal is configured. An authorized operator replaces the private file; each explicit request reads it afresh. Authorization is checked by the server."
      : auth.configured ? "Token comes from the startup environment. Renew it and restart Studio if it expires. The browser never receives credentials."
      : "Publication is disabled; configure the trusted loopback endpoint and a server-side token source.";
    const recovery = await api("/api/v1/publication-recovery");
    if (!currentTicket("publication-support", ticket)) return;
    if (!Array.isArray(recovery) || recovery.length > MAX_RECORDS || recovery.some(id => typeof id !== "string" || !/^[0-9a-f]{32}$/.test(id))) throw new Error("Invalid publication recovery inventory.");
    state.recovery = recovery;
    const list = element("publication-recovery-list"); list.replaceChildren();
    for (const id of recovery) {
      const item = document.createElement("li"); const button = document.createElement("button");
      button.type = "button"; button.textContent = `Recover local record ${id}`;
      button.addEventListener("click", () => void recoverPublication(id));
      item.append(button); list.append(item);
    }
  } catch (error) { if (currentTicket("publication-support", ticket)) announce(error.message); }
  finally { renderWorkflowAvailability(); }
}

async function recoverPublication(id) {
  if (!beginOperation("publication")) return;
  issue("publications");
  try {
    await jsonRequest(`/api/v1/publications/${encodeURIComponent(id)}/recover`, "POST", {});
    state.publicationNotice = "Local record recovered. This does not submit, approve or activate anything.";
    await Promise.all([loadPublications(), loadPublicationSupport()]);
  } catch (error) { state.publicationNotice = `${error.message} Recovery did not complete; preserve the workspace and inspect its state before retrying.`; }
  finally { endOperation("publication"); }
}

async function reconcilePublication() {
  const id = element("reconcile-request").value.trim();
  if (!/^[0-9a-f]{32}$/.test(id) || /^0+$/.test(id)) {
    announce("Enter the canonical request ID confirmed by an authorized operator."); return;
  }
  if (state.publicationAuth?.configured !== true || !beginOperation("publication")) return;
  issue("publications");
  try {
    const view = await jsonRequest(`/api/v1/publications/${encodeURIComponent(id)}/reconcile`, "POST", {});
    if (view?.request_id !== id) throw new Error("Reconciled request identity mismatch.");
    issue("publications");
    state.publications = [view, ...state.publications.filter(record => record.request_id !== id)].slice(0, MAX_RECORDS);
    renderPublications();
    state.publicationNotice = `Existing request reconciled: ${statusLabel(view.status)}. No submission, review or activation was performed.`;
    if (element("reconcile-request").value.trim() === id) element("reconcile-request").value = "";
    announce(state.publicationNotice);
  } catch (error) {
    state.publicationNotice = publicationFailure(error); announce(state.publicationNotice);
  } finally { endOperation("publication"); }
}

function publicationFailure(error) {
  if (error.status === 401 || error.status === 403) {
    return state.publicationAuth?.renewal === "token_file"
      ? "Publication credentials expired or permission was denied. Have an authorized operator renew the private token file or grants, then explicitly retry the desired operation."
      : "Publication credentials expired or permission was denied. Renew server-side credentials/grants and restart Studio if using a token environment. No automatic retry occurred.";
  }
  return `${error.message || "Publication failed."} If submission outcome is uncertain, reconcile with the operator before resubmitting.`;
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
  if (!data || !beginOperation("publication")) return;
  issue("publications");
  state.publicationNotice = null;
  try {
    element("submit-publication").disabled = true;
    const view = await jsonRequest("/api/v1/publications", "POST", data);
    issue("publications");
    state.publications = [
      view,
      ...state.publications.filter((item) => item.request_id !== view.request_id),
    ].slice(0, MAX_RECORDS);
    renderPublications();
    state.publicationNotice = "Requested — not yet approved or activated.";
    element("publication-note").textContent = state.publicationNotice;
    announce("Publication review requested; activation has not occurred.");
  } catch (error) {
    state.publicationNotice = publicationFailure(error);
    element("publication-note").textContent = state.publicationNotice;
    announce(state.publicationNotice);
  } finally {
    state.pendingPublication = null;
    endOperation("publication");
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
  if (!beginOperation("publication")) return;
  issue("publications");
  button.disabled = true;
  try {
    const view = await jsonRequest(
      `/api/v1/publications/${encodeURIComponent(requestId)}`,
      "POST",
      {},
    );
    issue("publications");
    state.publications = state.publications.map((record) =>
      record.request_id === view.request_id ? view : record,
    );
    renderPublications();
    state.publicationNotice = statusLabel(view.status);
    announce(state.publicationNotice);
  } catch (error) {
    state.publicationNotice = publicationFailure(error);
    announce(state.publicationNotice);
  } finally {
    button.disabled = false;
    endOperation("publication");
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
  if (!state.snapshot) return;
  state.snapshot.draft.namespace = element("namespace").value;
  markDirty();
});

element("entry-room").addEventListener("change", () => {
  if (!state.snapshot) return;
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
  if (!state.snapshot || !capabilities) { announce("Wait for the workspace to load."); return; }
  const id = element("new-room-id").value.trim();
  const title = element("new-room-title").value.trim();
  if (!capabilities || !id || !title || utf8Bytes(id) > capabilities.max_definition_bytes || utf8Bytes(title) > capabilities.max_text_bytes) {
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
  cancel.addEventListener("click", () => {
    if (["stale-artifact-dialog", "production-dialog"].includes(cancel.dataset.dialogCancel)) state.pendingPublication = null;
    element(cancel.dataset.dialogCancel).close();
  });
}
for (const dialog of document.querySelectorAll("dialog")) {
  dialog.addEventListener("close", () => restoreDialogFocus(dialog));
  dialog.addEventListener("cancel", () => {
    if (["stale-artifact-dialog", "production-dialog"].includes(dialog.id)) state.pendingPublication = null;
  });
}

element("world-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!state.snapshot || !event.currentTarget.reportValidity() || !beginOperation("draft")) return;
  const before = stamp();
  try {
    const saved = await api("/api/v1/draft", {
      method: "PUT",
      headers: {
        "Content-Type": "application/vnd.worldwright.world+json",
        "X-Worldwright-Draft-Revision": String(state.snapshot.revision),
      },
      body: JSON.stringify(state.snapshot.draft),
    });
    if (!validSnapshot(saved) || saved.revision <= before.revision) throw new Error("Invalid save acknowledgement; reconcile the saved revision.");
    const editedWhileSaving = !sameStamp(before);
    const selected = state.selectedRoomId;
    state.snapshot = editedWhileSaving ? { ...saved, draft: state.snapshot.draft } : saved;
    state.snapshotEpoch += 1;
    state.selectedRoomId =
      state.snapshot.draft.rooms.find((room) => room.id === selected)?.id ||
      state.snapshot.draft.entry_room ||
      state.snapshot.draft.rooms[0]?.id ||
      null;
    state.dirty = editedWhileSaving;
    state.validation = null;
    element("conflict-panel").hidden = true;
    renderDiagnostics();
    if (!editedWhileSaving) renderEditor(); else renderStatus();
    announce(editedWhileSaving ? `Saved revision ${saved.revision}; newer local edits were preserved and still need saving.` : `Saved revision ${saved.revision}`);
  } catch (error) {
    if (error.status === 409) {
      element("conflict-panel").hidden = false;
      element("conflict-text").textContent =
        `The workspace is now revision ${error.payload.current_revision ?? "unknown"}.`;
      element("conflict-panel").scrollIntoView({ block: "nearest" });
    }
    announce(error.message);
  } finally { endOperation("draft"); }
});

element("reload-draft").addEventListener("click", () => {
  void loadWorkspace({ discard: true }).catch(error => announce(error.message));
});

element("import-file").addEventListener("change", async () => {
  const file = element("import-file").files?.[0];
  if (!file || !state.snapshot) return;
  if (!beginOperation("draft")) { element("import-file").value = ""; return; }
  const before = stamp();
  try {
    if (!capabilities || file.size > capabilities.max_input_bytes) throw new Error("Import exceeds the input byte limit.");
    const text = await file.text();
    const result = await api("/api/v1/import", {
      method: "POST",
      headers: { "Content-Type": "application/vnd.worldwright.world+json" },
      body: text,
    });
    if (!sameStamp(before)) { announce("Import ignored because you edited while it was loading."); return; }
    if (!result?.draft || !Array.isArray(result.draft.rooms) || result.draft.rooms.length > MAX_ROOMS) throw new Error("Invalid import response.");
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
    endOperation("draft");
  }
});

element("export-draft").addEventListener("click", () => {
  if (state.snapshot) {
    window.location.assign(`/api/v1/revisions/${state.snapshot.revision}/export`);
  }
});

element("validate-draft").addEventListener("click", async () => {
  if (!state.snapshot || state.dirty || state.busy.has("draft") || !beginOperation("validation")) return;
  const before = stamp();
  try {
    const result = await jsonRequest(
      `/api/v1/revisions/${state.snapshot.revision}/validate`,
      "POST",
      {},
    );
    if (!sameStamp(before)) { announce("Validation result is stale; save and validate your current edits."); return; }
    if (result.snapshot_sha256 !== before.digest) throw new Error("Validation snapshot mismatch.");
    state.validation = { ...result, revision: before.revision };
    renderDiagnostics(state.validation);
    renderStatus();
    renderWorkflowAvailability();
    announce(result.valid ? "Saved snapshot is valid." : "Validation found errors.");
  } catch (error) {
    announce(error.message);
  } finally { endOperation("validation"); }
});

element("package-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!validationIsCurrent() || state.busy.has("draft") || !event.currentTarget.reportValidity() || !beginOperation("package")) return;
  const before = stamp();
  const requestedGame = element("package-game").value;
  issue("artifacts");
  try {
    const record = await jsonRequest("/api/v1/package", "POST", {
      revision: state.snapshot.revision,
      snapshot_sha256: state.snapshot.snapshot_sha256,
      release_id: element("release-id").value,
      builder_identity: element("builder-identity").value,
      game: requestedGame,
    });
    if (record.draft_revision !== before.revision || record.payload_digest !== before.digest) throw new Error("Packaged artifact snapshot mismatch.");
    issue("artifacts");
    state.artifacts = [
      record,
      ...state.artifacts.filter(
        (artifact) => artifact.artifact_digest !== record.artifact_digest,
      ),
    ].slice(0, MAX_RECORDS);
    if (sameStamp(before) && element("package-game").value === requestedGame) state.selectedArtifact = record.artifact_digest;
    renderArtifacts(); await loadArtifactIdentity();
    announce(`Packaged ${requestedGame} artifact ${shortDigest(record.artifact_digest)} from saved revision ${before.revision}`);
  } catch (error) {
    element("package-note").textContent = error.message;
    announce(error.message);
  } finally { endOperation("package"); }
});

element("artifact-select").addEventListener("change", () => {
  state.selectedArtifact = element("artifact-select").value || null;
  state.artifactIdentity = null;
  renderArtifacts();
  renderPreview();
  void loadArtifactIdentity();
});

element("preview-start").addEventListener("click", async () => {
  const artifact = selectedArtifact();
  if (!artifact || state.artifactIdentity?.artifact_digest !== artifact.artifact_digest || !beginOperation("preview")) return;
  const ticket = issue("preview");
  try {
    const preview = await jsonRequest("/api/v1/preview/start", "POST", {
      artifact_digest: artifact.artifact_digest,
      max_seconds: Number(element("preview-seconds").value),
    });
    if (!currentTicket("preview", ticket)) return;
    state.preview = preview;
    renderPreview();
    announce("Preview started with the selected immutable artifact.");
  } catch (error) {
    announce(error.message);
  } finally { endOperation("preview"); }
});

element("preview-input-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = element("preview-input").value;
  if (!text || utf8Bytes(text) > 1024 || state.busy.has("preview") || !beginOperation("preview-input")) return;
  try {
    await jsonRequest("/api/v1/preview/input", "POST", { text });
    if (element("preview-input").value === text) element("preview-input").value = "";
  } catch (error) {
    announce(error.message);
  } finally { endOperation("preview-input"); }
});

element("preview-stop").addEventListener("click", async () => {
  if (!beginOperation("preview")) return;
  const ticket = issue("preview");
  try {
    const preview = await jsonRequest("/api/v1/preview/stop", "POST", {});
    if (!currentTicket("preview", ticket)) return;
    state.preview = preview;
    renderPreview();
    announce(`Preview ${state.preview.state}.`);
  } catch (error) {
    announce(error.message);
  } finally { endOperation("preview"); }
});

element("publish-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const artifact = selectedArtifact();
  if (!artifact || !previewMatchesArtifact(artifact) || state.artifactIdentity?.artifact_digest !== artifact.artifact_digest || state.publicationAuth?.configured !== true || state.busy.has("publication") || state.pendingPublication || !event.currentTarget.reportValidity()) {
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
    state.dirty || artifact.draft_revision !== state.snapshot.revision ||
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
  if (!state.pendingPublication) return;
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
  if (!state.pendingPublication) return;
  state.pendingPublication.production_confirmation = expected;
  element("production-dialog").close();
  void submitPublication(state.pendingPublication);
});

element("reconcile-form").addEventListener("submit", (event) => {
  event.preventDefault();
  if (event.currentTarget.reportValidity()) void reconcilePublication();
});
element("refresh-publication-support").addEventListener("click", () => void loadPublicationSupport());
window.addEventListener("pagehide", stopPreviewPolling);
window.addEventListener("beforeunload", (event) => {
  if (state.dirty) {
    event.preventDefault();
    event.returnValue = "";
  }
});

async function initialize() {
  try {
    await loadCapabilities();
    await loadWorkspace();
    await Promise.all([loadArtifacts(), loadPreview(), loadPublications(), loadPublicationSupport()]);
    renderStatus();
    renderWorkflowAvailability();
    announce("Worldwright Studio workspace loaded.");
  } catch (error) {
    announce(error.message || "Studio failed to load.");
  }
}

void initialize();
