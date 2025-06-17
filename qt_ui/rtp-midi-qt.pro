TEMPLATE = app
TARGET = rtp-midi-qt

QT += qml quick core gui

CONFIG += c++17

# Definice pro různé platformy
win32 {
    RUST_LIB_PATH = ../target/release
    RUST_LIB_NAME = rtp_midi_lib.dll
    LIBS += -L$$RUST_LIB_PATH -lrtp_midi_lib
}
unix:!macx {
    RUST_LIB_PATH = ../target/release
    RUST_LIB_NAME = librtp_midi_lib.so
    LIBS += -L$$RUST_LIB_PATH -lrtp_midi_lib
    QMAKE_RPATHDIR += $$RUST_LIB_PATH
}
macx {
    RUST_LIB_PATH = ../target/release
    RUST_LIB_NAME = librtp_midi_lib.dylib
    LIBS += -L$$RUST_LIB_PATH -lrtp_midi_lib
    QMAKE_RPATHDIR += $$RUST_LIB_PATH
}

# Zkopíruje Rust knihovnu do výstupního adresáře při sestavení
# To je užitečné hlavně pro Windows, aby našel DLL
copy_rust_lib.target = $$OUT_PWD/$$RUST_LIB_NAME
copy_rust_lib.commands = $$(COPY_FILE) $$RUST_LIB_PATH/$$RUST_LIB_NAME $$OUT_PWD
QMAKE_EXTRA_TARGETS += copy_rust_lib
PRE_TARGETDEPS += $$copy_rust_lib.target

SOURCES += \
    main.cpp \
    rust_service_bridge.cpp

HEADERS += \
    rust_service_bridge.h

RESOURCES += qml.qrc

# Default rules for deployment.
qnx: target.path = /tmp/$${TARGET}/bin
else: unix:!android: target.path = /opt/$${TARGET}/bin
!isEmpty(target.path): INSTALLS += target
