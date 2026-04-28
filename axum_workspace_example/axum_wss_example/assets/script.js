const wsProtocol = window.location.protocol === "https:" ? "wss" : "ws";
const socket = new WebSocket(`${wsProtocol}://${window.location.host}/ws`);
const statusNode = document.getElementById("status");
const usernameInput = document.getElementById("username");
const messageInput = document.getElementById("message-input");
const sendButton = document.getElementById("send-button");
const messagesNode = document.getElementById("messages");
const eventsNode = document.getElementById("events");

usernameInput.value = window.localStorage.getItem("chat_username") || "";

usernameInput.addEventListener("change", function () {
    window.localStorage.setItem("chat_username", usernameInput.value);
});

socket.addEventListener("open", function () {
    statusNode.textContent = "Connected";
    socket.send(JSON.stringify({
        event: "hello",
        payload: {
            message: "Hello Server!",
            sentAt: new Date().toISOString(),
        },
    }));
});

socket.addEventListener("close", function () {
    statusNode.textContent = "Disconnected";
});

socket.addEventListener("error", function () {
    statusNode.textContent = "Connection error";
});

socket.addEventListener("message", function (event) {
    try {
        const json = JSON.parse(event.data);

        if (json.event === "chat_message") {
            appendChatMessage(json.payload);
            return;
        }

        appendEvent(json.event || "message", json.payload || json);
    } catch (_err) {
        appendEvent("raw", event.data);
    }
});

sendButton.addEventListener("click", sendChatMessage);

messageInput.addEventListener("keydown", function (event) {
    if (event.key === "Enter") {
        sendChatMessage();
    }
});

function sendChatMessage() {
    const msg = messageInput.value.trim();
    if (!msg || socket.readyState !== WebSocket.OPEN) {
        return;
    }

    socket.send(JSON.stringify({
        event: "chat_message",
        payload: {
            user: usernameInput.value.trim() || "Anonymous",
            msg,
        },
    }));

    messageInput.value = "";
}

function appendChatMessage(payload) {
    const li = document.createElement("li");
    const currentUser = usernameInput.value.trim() || "Anonymous";
    li.className = payload.user && payload.user.startsWith(currentUser) ? "me" : "other";
    li.innerHTML = `<strong>${escapeHtml(payload.user || "Unknown")}</strong><div>${escapeHtml(payload.msg || "")}</div>`;
    messagesNode.appendChild(li);
    messagesNode.scrollTop = messagesNode.scrollHeight;
}

function appendEvent(name, payload) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>${escapeHtml(name)}</strong><pre>${escapeHtml(JSON.stringify(payload, null, 2))}</pre>`;
    eventsNode.appendChild(li);
    eventsNode.scrollTop = eventsNode.scrollHeight;
}

function escapeHtml(value) {
    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}

setTimeout(() => {
    const obj = { hello: "world" };
    const blob = new Blob([JSON.stringify(obj, null, 2)], {
      type: "application/json",
    });
    console.log("Sending JSON blob over websocket");
    socket.send(blob);
}, 1000);
