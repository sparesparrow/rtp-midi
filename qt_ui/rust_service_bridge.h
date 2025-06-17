#ifndef RUST_SERVICE_BRIDGE_H
#define RUST_SERVICE_BRIDGE_H

#include <QObject>
#include <QString>

// Opaque pointer k Rust struktuře
struct ServiceHandle;

class RustServiceBridge : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool isRunning READ isRunning NOTIFY isRunningChanged)
    Q_PROPERTY(QString wledIp READ wledIp NOTIFY wledIpChanged)

public:
    explicit RustServiceBridge(QObject *parent = nullptr);
    ~RustServiceBridge();

    bool isRunning() const;
    QString wledIp() const;

public slots:
    void start();
    void stop();
    void setWledPreset(int presetId);

signals:
    void isRunningChanged();
    void wledIpChanged();
    void errorOccurred(const QString& message);

private:
    // Ukazatel na handle Rust služby
    ServiceHandle* m_serviceHandle;
    bool m_isRunning;
    QString m_wledIp;

    void updateStatus();
};

#endif // RUST_SERVICE_BRIDGE_H
