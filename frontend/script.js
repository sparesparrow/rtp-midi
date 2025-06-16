// script.js

const statusSpan = document.getElementById('status');
const messagesDiv = document.getElementById('messages');
const connectButton = document.getElementById('connectButton');
const peerIdInput = document.getElementById('peerIdInput');

const wsUrl = 'ws://127.0.0.1:8080'; // Replace with your server address if different
let websocket;

let peerConnection = null;
let dataChannel = null;
let myClientId = 'client-' + Math.random().toString(36).substring(7);

function connectWebSocket() {
    statusSpan.textContent = 'Connecting...';
    websocket = new WebSocket(wsUrl);

    websocket.onopen = (event) => {
        statusSpan.textContent = 'Connected';
        console.log('WebSocket connected:', event);
        // Send registration message
        sendSignalingMessage('register', 'server', {
            peer_type: 'ClientApp',
            client_id: myClientId
        });
    };

    websocket.onmessage = (event) => {
        console.log('Message from server:', event.data);
        const messageElement = document.createElement('div');
        messageElement.classList.add('message');
        messageElement.textContent = event.data;
        messagesDiv.appendChild(messageElement);
        messagesDiv.scrollTop = messagesDiv.scrollHeight; // Auto-scroll to the latest message

        try {
            const signalingMessage = JSON.parse(event.data);
            handleSignalingMessage(signalingMessage);
        } catch (e) {
            console.error('Failed to parse signaling message:', e);
        }
    };

    websocket.onerror = (event) => {
        statusSpan.textContent = 'Error';
        console.error('WebSocket error:', event);
    };

    websocket.onclose = (event) => {
        statusSpan.textContent = 'Disconnected';
        console.log('WebSocket closed:', event);
        // Attempt to reconnect after a delay
        setTimeout(connectWebSocket, 5000);
    };
}

function sendSignalingMessage(messageType, receiverId, payload) {
    const message = {
        message_type: messageType,
        sender_id: myClientId,
        receiver_id: receiverId,
        payload: payload
    };
    websocket.send(JSON.stringify(message));
}

function handleSignalingMessage(message) {
    switch (message.message_type) {
        case 'register_success':
            console.log('Registration successful. My client ID:', message.payload.registered_id);
            myClientId = message.payload.registered_id; // Use the ID assigned by the server
            // You can now list peers or wait for offers
            break;
        case 'offer':
            console.log('Received offer from', message.sender_id);
            handleOffer(message.payload.offer, message.sender_id);
            break;
        case 'answer':
            console.log('Received answer from', message.sender_id);
            handleAnswer(message.payload.answer);
            break;
        case 'ice_candidate':
            console.log('Received ICE candidate from', message.sender_id);
            handleIceCandidate(message.payload.candidate);
            break;
        case 'peer_list':
             console.log('Received peer list:', message.payload.peers);
             // TODO: Display the peer list in the UI
             break;
        case 'new_client':
             console.log('New client connected:', message.payload.client_id);
             // TODO: Update the peer list in the UI
             break;
        default:
            console.warn('Unknown signaling message type:', message.message_type);
    }
}

function createPeerConnection() {
    if (peerConnection) {
        console.log('PeerConnection already exists.');
        return;
    }

    // Use Google's public STUN server for now
    const configuration = { 'iceServers': [{ 'urls': 'stun:stun.l.google.com:19302' }] };

    peerConnection = new RTCPeerConnection(configuration);

    // Listen for ICE candidates and send them to the signaling server
    peerConnection.onicecandidate = (event) => {
        if (event.candidate) {
            console.log('Generated ICE candidate:', event.candidate);
            sendSignalingMessage('ice_candidate', peerIdInput.value, { // Send to the specified peer
                candidate: event.candidate
            });
        }
    };

    // Listen for connection state changes
    peerConnection.onconnectionstatechange = (event) => {
        console.log('ICE connection state change:', peerConnection.connectionState);
        // TODO: Update UI based on connection state
    };

    // Listen for data channel
    peerConnection.ondatachannel = (event) => {
        console.log('Data Channel received:', event.channel);
        dataChannel = event.channel;
        configureDataChannel(dataChannel);
    };

    console.log('PeerConnection created.');
}

async function createOffer(receiverId) {
    createPeerConnection();
    
    // Create a data channel
    const midiDataChannel = peerConnection.createDataChannel("midi");
    configureDataChannel(midiDataChannel);
    dataChannel = midiDataChannel;

    try {
        const offer = await peerConnection.createOffer();
        await peerConnection.setLocalDescription(offer);
        sendSignalingMessage('offer', receiverId, { offer: offer });
        console.log('Offer sent to', receiverId);
    } catch (error) {
        console.error('Error creating offer:', error);
    }
}

async function handleOffer(offer, senderId) {
    createPeerConnection();
    
    try {
        await peerConnection.setRemoteDescription(new RTCSessionDescription(offer));
        const answer = await peerConnection.createAnswer();
        await peerConnection.setLocalDescription(answer);
        sendSignalingMessage('answer', senderId, { answer: answer });
        console.log('Answer sent to', senderId);
    } catch (error) {
        console.error('Error handling offer:', error);
    }
}

async function handleAnswer(answer) {
    if (!peerConnection) {
        console.error('PeerConnection not established when receiving answer.');
        return;
    }

    try {
        await peerConnection.setRemoteDescription(new RTCSessionDescription(answer));
        console.log('Answer set as remote description.');
    } catch (error) {
        console.error('Error handling answer:', error);
    }
}

async function handleIceCandidate(candidate) {
    if (!peerConnection) {
        console.error('PeerConnection not established when receiving ICE candidate.');
        return;
    }

    try {
        await peerConnection.addIceCandidate(new RTCIceCandidate(candidate));
        console.log('ICE candidate added.');
    } catch (error) {
        console.error('Error adding ICE candidate:', error);
    }
}

function configureDataChannel(channel) {
    channel.onopen = (event) => {
        console.log('Data channel opened');
        // TODO: Indicate data channel is ready for MIDI data
    };

    channel.onmessage = (event) => {
        console.log('Data channel message received:', event.data);
        // TODO: Process incoming MIDI data
    };

    channel.onerror = (event) => {
        console.error('Data channel error:', event);
    };

    channel.onclose = (event) => {
        console.log('Data channel closed');
        // TODO: Handle data channel closure
    };
}

// Event listener for the connect button (will trigger offer creation)
connectButton.addEventListener('click', () => {
    const targetPeerId = peerIdInput.value;
    if (targetPeerId) {
        createOffer(targetPeerId);
    } else {
        alert('Please enter a target peer ID.');
    }
});

// Initial connection attempt
connectWebSocket(); 