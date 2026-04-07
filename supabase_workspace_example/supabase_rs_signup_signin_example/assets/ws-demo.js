const logEl = document.getElementById("log");
const statusPill = document.getElementById("status-pill");
const authPill = document.getElementById("auth-pill");
const protocolPill = document.getElementById("protocol-pill");
const emailInput = document.getElementById("email");
const passwordInput = document.getElementById("password");
const tokenInput = document.getElementById("token");
const signinButton = document.getElementById("signin");
const connectButton = document.getElementById("connect");
const disconnectButton = document.getElementById("disconnect");
const sendJsonButton = document.getElementById("send-json");
const sendBinaryButton = document.getElementById("send-binary");

let socket = null;

function appendLog(message) {
  logEl.textContent += `${message}\n`;
  logEl.scrollTop = logEl.scrollHeight;
}

function setStatus(message) {
  statusPill.textContent = `Socket: ${message}`;
}

function setAuth(message) {
  authPill.textContent = `Auth: ${message}`;
}

function currentSocketUrl() {
  const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const token = tokenInput.value.trim();
  const url = new URL("/ws", window.location.href);
  url.protocol = wsProtocol;
  url.searchParams.set("access_token", token);
  return url;
}

function ensureSocketOpen() {
  if (!socket || socket.readyState !== WebSocket.OPEN) {
    appendLog("[warn] socket is not open");
    return false;
  }
  return true;
}

async function signInWithSupabase() {
  const email = emailInput.value.trim();
  const password = passwordInput.value;

  if (!email || !password) {
    appendLog("[warn] email and password are required before sign-in");
    return;
  }

  setAuth("signing in");

  try {
    const response = await fetch("/auth/supabase/signin", {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify({ email, password }),
    });
    const payload = await response.json();

    if (!response.ok) {
      const message = payload.error || payload.message || "sign-in failed";
      throw new Error(message);
    }

    if (!payload.access_token) {
      throw new Error("Supabase sign-in did not return an access token");
    }

    tokenInput.value = payload.access_token;
    setAuth(`ready for ${payload.email || email}`);
    appendLog(`[auth] signed in as ${payload.email || email}`);
  } catch (error) {
    setAuth("failed");
    appendLog(`[auth-error] ${error.message}`);
  }
}

function connectSocket() {
  const token = tokenInput.value.trim();

  if (!token) {
    appendLog("[warn] sign in first or paste a Supabase access token");
    return;
  }

  if (socket && socket.readyState === WebSocket.OPEN) {
    appendLog("[warn] socket is already open");
    return;
  }

  const socketUrl = currentSocketUrl();
  protocolPill.textContent = `Protocol: ${socketUrl}`;
  setStatus("connecting");
  socket = new WebSocket(socketUrl);

  socket.addEventListener("open", () => {
    setStatus("open");
    appendLog("[open] websocket connected");
    socket.send(JSON.stringify({
      event: "hello",
      payload: {
        message: "Hello from ws demo",
        sentAt: new Date().toISOString(),
      },
    }));
  });

  socket.addEventListener("message", (event) => {
    try {
      const json = JSON.parse(event.data);
      appendLog(`[message] ${JSON.stringify(json, null, 2)}`);
    } catch (_err) {
      appendLog(`[message] ${event.data}`);
    }
  });

  socket.addEventListener("close", (event) => {
    setStatus(`closed (${event.code})`);
    appendLog(`[close] code=${event.code} reason=${event.reason || "none"}`);
    socket = null;
  });

  socket.addEventListener("error", () => {
    setStatus("error");
    appendLog("[error] websocket error");
  });
}

function disconnectSocket() {
  if (!socket) {
    appendLog("[warn] socket is not open");
    return;
  }

  socket.close(1000, "client disconnect");
}

sendJsonButton.addEventListener("click", () => {
  if (!ensureSocketOpen()) {
    return;
  }

  socket.send(JSON.stringify({
    event: "manual_json",
    payload: {
      source: "button",
      sentAt: new Date().toISOString(),
    },
  }));
});

sendBinaryButton.addEventListener("click", () => {
  if (!ensureSocketOpen()) {
    return;
  }

  const blob = new Blob(
    [JSON.stringify({ event: "manual_blob", payload: { source: "button" } }, null, 2)],
    { type: "application/json" },
  );
  socket.send(blob);
});

signinButton.addEventListener("click", () => {
  signInWithSupabase();
});

connectButton.addEventListener("click", () => {
  connectSocket();
});

disconnectButton.addEventListener("click", () => {
  disconnectSocket();
});

setStatus("disconnected");
setAuth("waiting for token");
protocolPill.textContent = "Protocol: connect after sign-in";
