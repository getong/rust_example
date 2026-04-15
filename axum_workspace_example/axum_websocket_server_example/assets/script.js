const socket = new WebSocket('ws://localhost:3000/ws');

socket.addEventListener('open', function (event) {
    socket.send('Hello Server!');
});

socket.addEventListener('message', function (event) {
    console.log('Message from server ', event.data);
});

socket.addEventListener('close', function (event) {
    console.log('WebSocket closed', event.code, event.reason);
});

socket.addEventListener('error', function (event) {
    console.log('WebSocket error', event);
});


setTimeout(() => {
    const obj = { hello: "world" };
    const blob = new Blob([JSON.stringify(obj, null, 2)], {
      type: "application/json",
    });
    console.log("Sending blob over websocket");
    socket.send(blob);
}, 1000);
