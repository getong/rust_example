const wsProtocol = window.location.protocol === "https:" ? "wss" : "ws";
const socketUrl = `${wsProtocol}://${window.location.host}/ws`;

const logEl = document.getElementById("log");
const statusPill = document.getElementById("status-pill");
const protocolPill = document.getElementById("protocol-pill");
const sendJsonButton = document.getElementById("send-json");
const sendBinaryButton = document.getElementById("send-binary");

protocolPill.textContent = `Protocol: ${socketUrl}`;

function appendLog(message) {
  logEl.textContent += `${message}\n`;
  logEl.scrollTop = logEl.scrollHeight;
}

function setStatus(message) {
  statusPill.textContent = `Socket: ${message}`;
}

const socket = new WebSocket(socketUrl);

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
});

socket.addEventListener("error", () => {
  setStatus("error");
  appendLog("[error] websocket error");
});

sendJsonButton.addEventListener("click", () => {
  if (socket.readyState !== WebSocket.OPEN) {
    appendLog("[warn] socket is not open");
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
  if (socket.readyState !== WebSocket.OPEN) {
    appendLog("[warn] socket is not open");
    return;
  }

  const blob = new Blob(
    [JSON.stringify({ event: "manual_blob", payload: { source: "button" } }, null, 2)],
    { type: "application/json" },
  );
  socket.send(blob);
});
