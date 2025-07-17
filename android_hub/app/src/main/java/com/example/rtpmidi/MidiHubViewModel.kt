package com.example.rtpmidi

import android.app.Application
import android.content.Context
import android.media.midi.MidiDevice
import android.media.midi.MidiDeviceInfo
import android.media.midi.MidiManager
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class DiscoveredDevice(
    val name: String,
    val address: String,
    val port: Int,
    val type: String
)

data class MidiHubUiState(
    val isServiceRunning: Boolean = false,
    val discoveredDevices: List<DiscoveredDevice> = emptyList(),
    val midiDeviceName: String? = null,
    val errorMessage: String? = null
)

class MidiHubViewModel(application: Application) : AndroidViewModel(application) {
    private val _uiState = MutableStateFlow(MidiHubUiState())
    val uiState: StateFlow<MidiHubUiState> = _uiState.asStateFlow()
    
    private lateinit var midiManager: MidiManager
    private lateinit var nsdManager: NsdManager
    private var currentMidiDevice: MidiDevice? = null
    
    init {
        initializeManagers()
        startDeviceDiscovery()
    }
    
    private fun initializeManagers() {
        val context = getApplication<Application>()
        midiManager = context.getSystemService(Context.MIDI_SERVICE) as MidiManager
        nsdManager = context.getSystemService(Context.NSD_SERVICE) as NsdManager
    }
    
    fun startService() {
        viewModelScope.launch {
            try {
                // Start the native service
                startNativeService()
                _uiState.value = _uiState.value.copy(isServiceRunning = true)
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    errorMessage = "Failed to start service: ${e.message}"
                )
            }
        }
    }
    
    fun stopService() {
        viewModelScope.launch {
            try {
                // Stop the native service
                stopNativeService()
                _uiState.value = _uiState.value.copy(isServiceRunning = false)
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    errorMessage = "Failed to stop service: ${e.message}"
                )
            }
        }
    }
    
    private fun startDeviceDiscovery() {
        // Discover OSC services (ESP32)
        discoverOscServices()
        
        // Discover AppleMIDI services (DAWs)
        discoverAppleMidiServices()
        
        // Discover MIDI devices
        discoverMidiDevices()
    }
    
    private fun discoverOscServices() {
        val discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String?, errorCode: Int) {
                // Handle error
            }
            
            override fun onStopDiscoveryFailed(serviceType: String?, errorCode: Int) {
                // Handle error
            }
            
            override fun onDiscoveryStarted(serviceType: String?) {
                // Discovery started
            }
            
            override fun onDiscoveryStopped(serviceType: String?) {
                // Discovery stopped
            }
            
            override fun onServiceFound(serviceInfo: NsdServiceInfo?) {
                serviceInfo?.let { info ->
                    nsdManager.resolveService(info, object : NsdManager.ResolveListener {
                        override fun onResolveFailed(serviceInfo: NsdServiceInfo?, errorCode: Int) {
                            // Handle resolve error
                        }
                        
                        override fun onServiceResolved(serviceInfo: NsdServiceInfo?) {
                            serviceInfo?.let { resolvedInfo ->
                                val device = DiscoveredDevice(
                                    name = resolvedInfo.serviceName,
                                    address = resolvedInfo.host.hostAddress ?: "",
                                    port = resolvedInfo.port,
                                    type = "OSC"
                                )
                                addDiscoveredDevice(device)
                            }
                        }
                    })
                }
            }
            
            override fun onServiceLost(serviceInfo: NsdServiceInfo?) {
                // Handle service lost
            }
        }
        
        nsdManager.discoverServices("_osc._udp.local.", NsdManager.PROTOCOL_DNS_SD, discoveryListener)
    }
    
    private fun discoverAppleMidiServices() {
        val discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String?, errorCode: Int) {
                // Handle error
            }
            
            override fun onStopDiscoveryFailed(serviceType: String?, errorCode: Int) {
                // Handle error
            }
            
            override fun onDiscoveryStarted(serviceType: String?) {
                // Discovery started
            }
            
            override fun onDiscoveryStopped(serviceType: String?) {
                // Discovery stopped
            }
            
            override fun onServiceFound(serviceInfo: NsdServiceInfo?) {
                serviceInfo?.let { info ->
                    nsdManager.resolveService(info, object : NsdManager.ResolveListener {
                        override fun onResolveFailed(serviceInfo: NsdServiceInfo?, errorCode: Int) {
                            // Handle resolve error
                        }
                        
                        override fun onServiceResolved(serviceInfo: NsdServiceInfo?) {
                            serviceInfo?.let { resolvedInfo ->
                                val device = DiscoveredDevice(
                                    name = resolvedInfo.serviceName,
                                    address = resolvedInfo.host.hostAddress ?: "",
                                    port = resolvedInfo.port,
                                    type = "AppleMIDI"
                                )
                                addDiscoveredDevice(device)
                            }
                        }
                    })
                }
            }
            
            override fun onServiceLost(serviceInfo: NsdServiceInfo?) {
                // Handle service lost
            }
        }
        
        nsdManager.discoverServices("_apple-midi._udp.local.", NsdManager.PROTOCOL_DNS_SD, discoveryListener)
    }
    
    private fun discoverMidiDevices() {
        val devices = midiManager.devices
        devices.forEach { deviceInfo ->
            if (deviceInfo.inputPortCount > 0) {
                // This is a MIDI input device (like Maschine)
                connectToMidiDevice(deviceInfo)
            }
        }
    }
    
    private fun connectToMidiDevice(deviceInfo: MidiDeviceInfo) {
        midiManager.openDevice(deviceInfo, object : MidiManager.OnDeviceOpenedListener {
            override fun onDeviceOpened(device: MidiDevice?) {
                device?.let { midiDevice ->
                    currentMidiDevice = midiDevice
                    _uiState.value = _uiState.value.copy(
                        midiDeviceName = deviceInfo.properties.getString(MidiDeviceInfo.PROPERTY_NAME)
                    )
                    
                    // Start MIDI input monitoring
                    startMidiInputMonitoring(midiDevice)
                }
            }
        }, null)
    }
    
    private fun startMidiInputMonitoring(device: MidiDevice) {
        // Implementation for monitoring MIDI input
        // This would involve setting up a MidiReceiver to handle incoming MIDI data
    }
    
    private fun addDiscoveredDevice(device: DiscoveredDevice) {
        val currentDevices = _uiState.value.discoveredDevices.toMutableList()
        val existingIndex = currentDevices.indexOfFirst { it.name == device.name }
        
        if (existingIndex >= 0) {
            currentDevices[existingIndex] = device
        } else {
            currentDevices.add(device)
        }
        
        _uiState.value = _uiState.value.copy(discoveredDevices = currentDevices)
    }
    
    private external fun startNativeService()
    private external fun stopNativeService()
    
    companion object {
        init {
            System.loadLibrary("rtp_midi_lib")
        }
    }
} 