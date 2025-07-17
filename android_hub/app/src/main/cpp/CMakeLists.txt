cmake_minimum_required(VERSION 3.22.1)

project("rtpmidi")

# Set the path to the Rust library
set(RUST_LIB_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../../rust-libs)

# Add the Rust library
add_library(rtp_midi_lib SHARED IMPORTED)
set_target_properties(rtp_midi_lib PROPERTIES IMPORTED_LOCATION
    ${RUST_LIB_DIR}/${ANDROID_ABI}/librtp_midi_lib.so)

# Create the JNI wrapper library
add_library(midi_hub_jni SHARED
    midi_hub_jni.cpp)

# Link against the Rust library
target_link_libraries(midi_hub_jni
    rtp_midi_lib
    android
    log)

# Include directories
target_include_directories(midi_hub_jni PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../rust-libs/include) 