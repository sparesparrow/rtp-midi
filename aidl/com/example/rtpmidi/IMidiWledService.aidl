// aidl/com/example/rtpmidi/IMidiWledService.aidl
package com.example.rtpmidi;

/**
 * Interface for the Rust MIDI/WLED Service.
 * Allows an Android application to control the background service.
 * Toto rozhraní definuje kontrakt mezi Android aplikací a Rust službou.
 */
interface IMidiWledService {
    /**
     * Starts the main service loop that listens for audio/MIDI.
     * Spustí hlavní smyčku služby.
     * @return true if started successfully or was already running.
     */
    boolean startListener();

    /**
     * Stops the main service loop.
     * Zastaví hlavní smyčku služby.
     */
    void stopListener();

    /**
     * Sets a WLED preset by its ID.
     * Nastaví WLED preset podle zadaného ID.
     */
    void setWledPreset(int presetId);

    /**
     * Gets the current status of the service (e.g., "Running", "Stopped").
     * Získá aktuální stav služby.
     */
    String getStatus();

    /**
     * Checks if the service is currently running.
     * Zjistí, zda služba aktuálně běží.
     */
    boolean isRunning();
}
