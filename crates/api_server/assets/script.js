const socket = new WebSocket("ws://localhost:3000/ws");

socket.addEventListener('open', (event) => {
    socket.send('Hello server!');
});

socket.addEventListener('message', (event) => {
    let element = document.getElementById('responses');
    element.innerText += event.data + '\n';
});