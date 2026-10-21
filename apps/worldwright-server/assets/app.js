(() => {
  const loginForm = document.querySelector("#login-form");
  const commandForm = document.querySelector("#command-form");
  const commandInput = document.querySelector("#command");
  const commandSubmit = document.querySelector("#command-submit");
  const username = document.querySelector("#username");
  const password = document.querySelector("#password");
  const output = document.querySelector("#output");
  const status = document.querySelector("#status");
  const logout = document.querySelector("#logout");
  let socket = null;
  const heartbeatTimers = new WeakMap();
  let requestNumber = 0;

  function setStatus(value) {
    status.textContent = value;
  }

  function setCommandEnabled(enabled) {
    commandInput.disabled = !enabled;
    commandSubmit.disabled = !enabled;
  }

  function print(value) {
    const item = document.createElement("li");
    item.textContent = value;
    output.appendChild(item);
    while (output.childElementCount > 500) {
      output.removeChild(output.firstElementChild);
    }
  }

  function nextRequestId() {
    requestNumber += 1;
    return `browser-${requestNumber}`;
  }

  function send(current, message) {
    if (current && socket === current && current.readyState === WebSocket.OPEN) {
      current.send(JSON.stringify(message));
    }
  }

  function protocolFailure(current) {
    if (socket !== current) {
      return;
    }
    setCommandEnabled(false);
    print("Connection protocol error.");
    current.close();
  }

  function connect() {
    if (socket) {
      socket.close();
    }
    setCommandEnabled(false);
    const scheme = location.protocol === "https:" ? "wss:" : "ws:";
    const current = new WebSocket(`${scheme}//${location.host}/api/v1/play`);
    socket = current;
    let streamEpoch = null;
    let expectedSequence = 1;
    current.addEventListener("open", () => {
      if (socket !== current) {
        return;
      }
      setStatus("Connected; negotiating");
      send(current, { type: "hello", supported_versions: [1] });
      const timer = window.setInterval(() => send(current, { type: "ping", nonce: Date.now() }), 20000);
      heartbeatTimers.set(current, timer);
    });
    current.addEventListener("message", (event) => {
      if (socket !== current) {
        return;
      }
      let envelope;
      try {
        envelope = JSON.parse(event.data);
      } catch (_) {
        protocolFailure(current);
        return;
      }
      if (envelope.protocol_version !== 1
          || typeof envelope.stream_epoch !== "string"
          || !/^[0-9a-f]{32}$/.test(envelope.stream_epoch)
          || envelope.sequence !== expectedSequence
          || !envelope.message
          || typeof envelope.message.type !== "string") {
        protocolFailure(current);
        return;
      }
      if (streamEpoch === null) {
        streamEpoch = envelope.stream_epoch;
      } else if (streamEpoch !== envelope.stream_epoch) {
        protocolFailure(current);
        return;
      }
      expectedSequence += 1;
      const body = envelope.message;
      switch (body.type) {
        case "welcome":
          send(current, { type: "attach", request_id: nextRequestId() });
          break;
        case "accepted":
          break;
        case "attached":
          setCommandEnabled(true);
          setStatus(`Attached as ${body.character_name}`);
          print(`Attached as ${body.character_name}.`);
          break;
        case "output":
          print(body.speaker ? `${body.speaker}: ${body.text}` : body.text);
          break;
        case "exits":
          print(`Exits: ${body.exits.length ? body.exits.join(", ") : "(none)"}`);
          break;
        case "command_result":
          break;
        case "reward_result":
          print(`Reward ${body.outcome}; balance ${body.balance}${body.item_id ? `; item ${body.item_id}` : ""}${body.replayed ? " (replayed)" : ""}.`);
          break;
        case "transfer_result":
          print(`Transfer ${body.outcome}; item ${body.item_id}${body.recipient_name ? ` to ${body.recipient_name}` : ""}${body.replayed ? " (replayed)" : ""}.`);
          break;
        case "inventory":
          print(`Balance ${body.balance}; items ${body.items.length}.`);
          body.items.forEach((item) => print(`${item.item_id} ${item.definition_id} v${item.version}`));
          break;
        case "error":
          print(`${body.code}: ${body.message}`);
          if (body.fatal) {
            setCommandEnabled(false);
            current.close();
          }
          break;
        case "pong":
          break;
        default:
          protocolFailure(current);
          break;
      }
    });
    current.addEventListener("close", () => {
      const timer = heartbeatTimers.get(current);
      if (timer !== undefined) {
        window.clearInterval(timer);
        heartbeatTimers.delete(current);
      }
      if (socket !== current) {
        return;
      }
      setCommandEnabled(false);
      setStatus("Disconnected");
      socket = null;
    });
    current.addEventListener("error", () => {
      if (socket === current) {
        setStatus("Connection error");
      }
    });
  }

  loginForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const payload = { username: username.value, password: password.value };
    password.value = "";
    try {
      const response = await fetch("/api/v1/session", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      if (response.ok) {
        print("Logged in.");
        connect();
      } else {
        print("Login failed.");
      }
    } catch (_) {
      print("Network request failed.");
    }
  });

  commandForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const value = commandInput.value.trim();
    const requestId = nextRequestId();
    if (value === "look" || value === "l") {
      send(socket, { type: "command", request_id: requestId, command: { type: "look" } });
    } else if (value === "inventory") {
      send(socket, { type: "command", request_id: requestId, command: { type: "inventory" } });
    } else if (value === "claim welcome") {
      send(socket, { type: "command", request_id: requestId, command: { type: "claim_reward", reward: "welcome" } });
    } else if (value.startsWith("give ")) {
      const parts = value.slice(5).trim().split(/\s+/);
      if (parts.length === 2) {
        send(socket, { type: "command", request_id: requestId, command: { type: "transfer_item", item_id: parts[0], recipient: parts[1] } });
      } else {
        print("Commands: look, l, go <exit-label>, say <text>, inventory, claim welcome, give <item-id> <username>");
      }
    } else if (value.startsWith("go ")) {
      send(socket, { type: "command", request_id: requestId, command: { type: "move", exit: value.slice(3).trim() } });
    } else if (value.startsWith("say ")) {
      send(socket, { type: "command", request_id: requestId, command: { type: "say", text: value.slice(4) } });
    } else {
      print("Commands: look, l, go <exit-label>, say <text>");
    }
    commandInput.value = "";
  });

  logout.addEventListener("click", async () => {
    const current = socket;
    setCommandEnabled(false);
    if (current) {
      const timer = heartbeatTimers.get(current);
      if (timer !== undefined) {
        window.clearInterval(timer);
        heartbeatTimers.delete(current);
      }
      socket = null;
      current.close();
    }
    try {
      await fetch("/api/v1/session", { method: "DELETE", credentials: "same-origin" });
    } catch (_) {
      print("Network request failed.");
    }
    password.value = "";
    output.replaceChildren();
    setStatus("Logged out");
  });

  setCommandEnabled(false);
})();
