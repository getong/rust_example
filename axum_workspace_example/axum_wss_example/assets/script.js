const wsProtocol = window.location.protocol === "https:" ? "wss" : "ws";
const socket = new WebSocket(`${wsProtocol}://${window.location.host}/ws`);

socket.addEventListener("open", function () {
    socket.send(JSON.stringify({
        event: "hello",
        payload: {
            message: "Hello Server!",
            sentAt: new Date().toISOString(),
        },
    }));
});

socket.addEventListener("message", function (event) {
    try {
        const json = JSON.parse(event.data);
        console.log("JSON message from server:", json);
    } catch (_err) {
        console.log("Message from server:", event.data);
    }
});


setTimeout(() => {
    const obj = { hello: "world" };
    const blob = new Blob([JSON.stringify(obj, null, 2)], {
      type: "application/json",
    });
    console.log("Sending JSON blob over websocket");
    socket.send(blob);
}, 1000);

setTimeout(() => {
    socket.send(JSON.stringify({
        event: "done",
        payload: {
            message: "About done here...",
        },
    }));
    console.log("Sending close over websocket");
    socket.close(3000, "Crash and Burn!");
}, 3000);
