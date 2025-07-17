package com.example.rtpmidi

import android.app.*
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import androidx.core.app.NotificationCompat
import android.content.Context

class MidiHubService : Service() {
    private val binder = LocalBinder()
    private var isRunning = false
    
    inner class LocalBinder : Binder() {
        fun getService(): MidiHubService = this@MidiHubService
    }
    
    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startService()
            ACTION_STOP -> stopService()
        }
        return START_STICKY
    }
    
    override fun onBind(intent: Intent?): IBinder {
        return binder
    }
    
    private fun startService() {
        if (isRunning) return
        
        // Start foreground service
        val notification = createNotification()
        startForeground(NOTIFICATION_ID, notification)
        
        // Initialize native service
        initializeNativeService()
        
        isRunning = true
    }
    
    private fun stopService() {
        if (!isRunning) return
        
        // Stop native service
        stopNativeService()
        
        // Stop foreground service
        stopForeground(true)
        stopSelf()
        
        isRunning = false
    }
    
    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            "MIDI Hub Service",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "RTP-MIDI Hub Service"
            setShowBadge(false)
        }
        
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.createNotificationChannel(channel)
    }
    
    private fun createNotification(): Notification {
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("RTP-MIDI Hub")
            .setContentText("Service is running")
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setOngoing(true)
            .build()
    }
    
    private external fun initializeNativeService()
    private external fun stopNativeService()
    
    companion object {
        private const val CHANNEL_ID = "midi_hub_channel"
        private const val NOTIFICATION_ID = 1
        
        const val ACTION_START = "com.example.rtpmidi.START"
        const val ACTION_STOP = "com.example.rtpmidi.STOP"
        
        fun startService(context: Context) {
            val intent = Intent(context, MidiHubService::class.java).apply {
                action = ACTION_START
            }
            context.startForegroundService(intent)
        }
        
        fun stopService(context: Context) {
            val intent = Intent(context, MidiHubService::class.java).apply {
                action = ACTION_STOP
            }
            context.startService(intent)
        }
    }
} 