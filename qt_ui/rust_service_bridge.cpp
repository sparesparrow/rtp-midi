#include "rust_service_bridge.h"
#include <QDebug>
#include <QThread>

// Deklarace funkcí z naší Rust knihovny
extern "C" {
    ServiceHandle* create_service(const char* config_path);
    void start_service(ServiceHandle* handle);
    void stop_service(ServiceHandle* handle);
    void destroy_service(ServiceHandle* handle);
    void set_wled_preset(ServiceHandle* handle, int32_t preset_id);
    char* get_wled_ip(ServiceHandle* handle);
    void free_string(char* s);
}

RustServiceBridge::RustServiceBridge(QObject *parent)
    : QObject(parent), m_serviceHandle(nullptr), m_isRunning(false)
{
    // Cesta ke konfiguračnímu souboru. Pro jednoduchost ji zde máme natvrdo.
    // V reálné aplikaci by mohla být konfigurovatelná.
    const char* config_path = "../config.toml";
    m_serviceHandle = create_service(config_path);

    if (!m_serviceHandle) {
        qWarning() << "Failed to create Rust service handle. Check if config.toml exists.";
        emit errorOccurred("Failed to initialize service. Check config.");
    } else {
        // Získání počátečních hodnot z Rustu
        updateStatus();
    }
}

RustServiceBridge::~RustServiceBridge()
{
    if (m_serviceHandle) {
        stop(); // Ujistíme se, že služba je zastavená
        destroy_service(m_serviceHandle);
        m_serviceHandle = nullptr;
    }
}

bool RustServiceBridge::isRunning() const
{
    return m_isRunning;
}

QString RustServiceBridge::wledIp() const
{
    return m_wledIp;
}

void RustServiceBridge::start()
{
    if (!m_serviceHandle || m_isRunning) return;

    qInfo() << "Attempting to start service...";
    // Spouštíme službu v jiném vlákně, aby neblokovala UI
    QThread* thread = QThread::create([this](){
        start_service(m_serviceHandle);
        // Po dokončení (zastavení) Rust funkce aktualizujeme stav
        QMetaObject::invokeMethod(this, "updateStatus", Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QThread::deleteLater);
    thread->start();

    // Předpokládáme, že start je okamžitý, a aktualizujeme stav
    m_isRunning = true;
    emit isRunningChanged();
}

void RustServiceBridge::stop()
{
    if (!m_serviceHandle || !m_isRunning) return;

    qInfo() << "Stopping service...";
    stop_service(m_serviceHandle);
    m_isRunning = false;
    emit isRunningChanged();
}

void RustServiceBridge::setWledPreset(int presetId)
{
    if (!m_serviceHandle) return;
    qInfo() << "Setting WLED preset to" << presetId;
    set_wled_preset(m_serviceHandle, presetId);
}

void RustServiceBridge::updateStatus()
{
    if (!m_serviceHandle) return;

    // Aktualizace IP adresy
    char* ip_c_str = get_wled_ip(m_serviceHandle);
    if (ip_c_str) {
        QString newIp = QString::fromUtf8(ip_c_str);
        free_string(ip_c_str);
        if (newIp != m_wledIp) {
            m_wledIp = newIp;
            emit wledIpChanged();
        }
    }
}
