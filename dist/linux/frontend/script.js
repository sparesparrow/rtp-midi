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

// === Settings Panel Logic ===
const settingsBtn = document.getElementById('settingsBtn');
const settingsPanel = document.getElementById('settingsPanel');
const closeSettingsBtn = document.getElementById('closeSettingsBtn');
const saveSettingsBtn = document.getElementById('saveSettingsBtn');
const ledCountInput = document.getElementById('ledCountInput');
const mappingPresetInput = document.getElementById('mappingPresetInput');

function loadSettings() {
    const settings = JSON.parse(localStorage.getItem('rtpmidi_settings') || '{}');
    ledCountInput.value = settings.ledCount || 60;
    mappingPresetInput.value = settings.mappingPreset || 'spectrum';
}

function saveSettings() {
    const settings = {
        ledCount: parseInt(ledCountInput.value, 10),
        mappingPreset: mappingPresetInput.value
    };
    localStorage.setItem('rtpmidi_settings', JSON.stringify(settings));
    addMessage('Settings saved.');
    settingsPanel.style.display = 'none';
    // Optionally: send settings to backend or reload UI as needed
}

settingsBtn.onclick = () => {
    loadSettings();
    settingsPanel.style.display = 'block';
};
closeSettingsBtn.onclick = () => {
    settingsPanel.style.display = 'none';
};
saveSettingsBtn.onclick = saveSettings;

// Load settings on page load
loadSettings();

// Utility: Set status badge
function setStatus(status, text) {
    statusSpan.textContent = text;
    statusSpan.className = 'status-badge ' + status;
}

// Utility: Add message with timestamp
function addMessage(text) {
    const messageElement = document.createElement('div');
    messageElement.classList.add('message');
    const now = new Date();
    const time = now.toLocaleTimeString();
    messageElement.innerHTML = `<span style="color:#888;font-size:0.92em;">[${time}]</span> ${text}`;
    messagesDiv.appendChild(messageElement);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

// Peer list UI
const peerListDiv = document.getElementById('peerList');
let currentPeers = [];
function updatePeerList(peers) {
    currentPeers = peers;
    if (!peers || peers.length === 0) {
        peerListDiv.textContent = 'No peers available.';
        return;
    }
    peerListDiv.innerHTML = '';
    peers.forEach(peerId => {
        if (peerId === myClientId) return; // Don't show self
        const peerElem = document.createElement('span');
        peerElem.className = 'peer';
        peerElem.textContent = peerId;
        peerElem.onclick = () => {
            peerIdInput.value = peerId;
            createOffer(peerId);
        };
        peerListDiv.appendChild(peerElem);
    });
}

// MIDI Events Visualization
const midiEventsBody = document.getElementById('midiEventsBody');
function addMidiEvent({direction, type, channel, noteOrControl, velocityOrValue}) {
    const now = new Date();
    const time = now.toLocaleTimeString();
    const row = document.createElement('tr');
    row.innerHTML = `
        <td style="padding:2px 8px;color:#888;">${time}</td>
        <td style="padding:2px 8px;">${direction}</td>
        <td style="padding:2px 8px;">${type}</td>
        <td style="padding:2px 8px;">${channel ?? ''}</td>
        <td style="padding:2px 8px;">${noteOrControl ?? ''}</td>
        <td style="padding:2px 8px;">${velocityOrValue ?? ''}</td>
    `;
    if (type === 'Note On' && velocityOrValue > 0) {
        row.style.background = '#e3ffe3';
    } else if (type === 'Note Off' || (type === 'Note On' && velocityOrValue === 0)) {
        row.style.background = '#ffe3e3';
    }
    midiEventsBody.appendChild(row);
    // Keep only last 100 events
    while (midiEventsBody.children.length > 100) {
        midiEventsBody.removeChild(midiEventsBody.firstChild);
    }
}

// Helper: Parse MIDI message bytes (basic)
function parseMidiMessage(bytes) {
    if (!bytes || bytes.length < 1) return null;
    const status = bytes[0];
    const typeNibble = status & 0xF0;
    const channel = (status & 0x0F) + 1;
    switch (typeNibble) {
        case 0x80:
            return { type: 'Note Off', channel, noteOrControl: bytes[1], velocityOrValue: bytes[2] };
        case 0x90:
            return { type: 'Note On', channel, noteOrControl: bytes[1], velocityOrValue: bytes[2] };
        case 0xB0:
            return { type: 'Control Change', channel, noteOrControl: bytes[1], velocityOrValue: bytes[2] };
        default:
            return { type: 'Other', channel, noteOrControl: bytes[1], velocityOrValue: bytes[2] };
    }
}

function connectWebSocket() {
    setStatus('connecting', 'Connecting...');
    websocket = new WebSocket(wsUrl);

    websocket.onopen = (event) => {
        setStatus('connected', 'Connected');
        addMessage('WebSocket connected.');
        console.log('WebSocket connected:', event);
        // Send registration message
        sendSignalingMessage('register', 'server', {
            peer_type: 'ClientApp',
            client_id: myClientId
        });
    };

    websocket.onmessage = (event) => {
        addMessage(event.data);
        console.log('Message from server:', event.data);

        try {
            const signalingMessage = JSON.parse(event.data);
            handleSignalingMessage(signalingMessage);
        } catch (e) {
            console.error('Failed to parse signaling message:', e);
        }
    };

    websocket.onerror = (event) => {
        setStatus('error', 'Error');
        addMessage('WebSocket error.');
        console.error('WebSocket error:', event);
    };

    websocket.onclose = (event) => {
        setStatus('disconnected', 'Disconnected');
        addMessage('WebSocket closed. Reconnecting in 5s...');
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
            updatePeerList(message.payload.peers);
            break;
        case 'new_client':
            if (message.payload && message.payload.client_id) {
                if (!currentPeers.includes(message.payload.client_id)) {
                    currentPeers.push(message.payload.client_id);
                    updatePeerList(currentPeers);
                }
            }
            break;
        case 'midi':
            // Incoming MIDI event from server/peer
            if (message.payload && message.payload.data) {
                const midiBytes = message.payload.data;
                const midi = parseMidiMessage(midiBytes);
                if (midi) addMidiEvent({direction: 'In', ...midi});
            }
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
        addMessage('ICE connection state: ' + peerConnection.connectionState);
        if (peerConnection.connectionState === 'connected') {
            setStatus('connected', 'Peer Connected');
        } else if (peerConnection.connectionState === 'disconnected' || peerConnection.connectionState === 'failed') {
            setStatus('disconnected', 'Peer Disconnected');
        }
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
        setStatus('connected', 'Data Channel Ready');
        addMessage('Data channel is open and ready for MIDI data.');
    };

    channel.onmessage = (event) => {
        console.log('Data channel message received:', event.data);
        // Assume MIDI data is sent as Uint8Array or Array of bytes (JSON-encoded)
        let midiBytes;
        if (event.data instanceof ArrayBuffer) {
            midiBytes = new Uint8Array(event.data);
        } else if (typeof event.data === 'string') {
            try {
                // Try to parse as JSON array
                const arr = JSON.parse(event.data);
                if (Array.isArray(arr)) {
                    midiBytes = Uint8Array.from(arr);
                }
            } catch (e) {
                console.warn('Data channel message is not valid JSON:', event.data);
            }
        }
        if (midiBytes && midiBytes.length > 0) {
            const midi = parseMidiMessage(midiBytes);
            if (midi) {
                addMidiEvent({direction: 'In (DataChannel)', ...midi});
            } else {
                addMessage('Received unknown MIDI data on data channel: ' + midiBytes);
            }
        } else {
            addMessage('Received non-MIDI or malformed data on data channel.');
        }
    };

    channel.onerror = (event) => {
        console.error('Data channel error:', event);
        setStatus('error', 'Data Channel Error');
        addMessage('Data channel error occurred.');
    };

    channel.onclose = (event) => {
        console.log('Data channel closed');
        setStatus('disconnected', 'Data Channel Closed');
        addMessage('Data channel has been closed. MIDI data will not be received.');
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

// === Simple Frontend Test Harness ===
function runFrontendTests() {
    let passed = 0, failed = 0;
    function assert(cond, msg) {
        if (cond) { console.log('PASS:', msg); passed++; }
        else { console.error('FAIL:', msg); failed++; }
    }
    // Test: Peer list rendering
    updatePeerList(['peer1', 'peer2', myClientId]);
    assert(peerListDiv.children.length === 2, 'Peer list renders correct number of peers (excluding self)');
    // Test: Status badge update
    setStatus('connected', 'Connected');
    assert(statusSpan.classList.contains('connected'), 'Status badge class updated to connected');
    setStatus('error', 'Error');
    assert(statusSpan.classList.contains('error'), 'Status badge class updated to error');
    // Test: Message log
    const msgCountBefore = messagesDiv.children.length;
    addMessage('Test message');
    assert(messagesDiv.children.length === msgCountBefore + 1, 'Message log appends new message');
    // Summary
    console.log(`Frontend tests: ${passed} passed, ${failed} failed.`);
}
// Uncomment to run tests in browser console:
// runFrontendTests(); 