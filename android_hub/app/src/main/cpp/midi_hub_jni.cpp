#include <jni.h>
#include <android/log.h>
#include <string>

#define LOG_TAG "MidiHubJNI"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

// External Rust functions
extern "C" {
    void* create_service(const char* config_path);
    void start_android_hub_service(void* handle, const char* esp32_ip, uint16_t esp32_port, 
                                  const char* daw_ip, uint16_t daw_port);
    void stop_service(void* handle);
    void destroy_service(void* handle);
}

// Global service handle
static void* g_service_handle = nullptr;

extern "C" JNIEXPORT void JNICALL
Java_com_example_rtpmidi_MidiHubService_initializeNativeService(JNIEnv* env, jobject thiz) {
    LOGI("Initializing native service");
    
    if (g_service_handle != nullptr) {
        LOGE("Service already initialized");
        return;
    }
    
    // Create service with default config
    g_service_handle = create_service(nullptr);
    if (g_service_handle == nullptr) {
        LOGE("Failed to create service");
        return;
    }
    
    LOGI("Native service initialized successfully");
}

extern "C" JNIEXPORT void JNICALL
Java_com_example_rtpmidi_MidiHubService_stopNativeService(JNIEnv* env, jobject thiz) {
    LOGI("Stopping native service");
    
    if (g_service_handle != nullptr) {
        stop_service(g_service_handle);
        destroy_service(g_service_handle);
        g_service_handle = nullptr;
        LOGI("Native service stopped");
    }
}

extern "C" JNIEXPORT void JNICALL
Java_com_example_rtpmidi_MidiHubViewModel_startNativeService(JNIEnv* env, jobject thiz) {
    LOGI("Starting native service from ViewModel");
    
    if (g_service_handle == nullptr) {
        LOGE("Service not initialized");
        return;
    }
    
    // Start service with discovered device addresses
    // For now, use default addresses - these would be passed from the ViewModel
    start_android_hub_service(g_service_handle, 
                             "192.168.1.100", 8000,  // ESP32 default
                             "192.168.1.50", 5004);  // DAW default
    
    LOGI("Native service started");
}

extern "C" JNIEXPORT void JNICALL
Java_com_example_rtpmidi_MidiHubViewModel_stopNativeService(JNIEnv* env, jobject thiz) {
    LOGI("Stopping native service from ViewModel");
    
    if (g_service_handle != nullptr) {
        stop_service(g_service_handle);
        LOGI("Native service stopped");
    }
}

// Function to start service with specific device addresses
extern "C" JNIEXPORT void JNICALL
Java_com_example_rtpmidi_MidiHubViewModel_startServiceWithDevices(
    JNIEnv* env, jobject thiz, 
    jstring esp32_ip, jint esp32_port,
    jstring daw_ip, jint daw_port) {
    
    if (g_service_handle == nullptr) {
        LOGE("Service not initialized");
        return;
    }
    
    const char* esp32_ip_str = env->GetStringUTFChars(esp32_ip, nullptr);
    const char* daw_ip_str = env->GetStringUTFChars(daw_ip, nullptr);
    
    start_android_hub_service(g_service_handle, 
                             esp32_ip_str, (uint16_t)esp32_port,
                             daw_ip_str, (uint16_t)daw_port);
    
    env->ReleaseStringUTFChars(esp32_ip, esp32_ip_str);
    env->ReleaseStringUTFChars(daw_ip, daw_ip_str);
    
    LOGI("Native service started with specific devices");
} 