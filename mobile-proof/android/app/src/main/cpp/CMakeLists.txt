# Build libfluxcore_mobile_probe.so for the Android runtime proof.
#
# This CMake project is invoked by Build/build-rust-android.sh for each target
# ABI. It links the prebuilt libfluxcore.a static archive produced by cargo and
# the small JNI bridge defined in jni-bridge.cpp.

cmake_minimum_required(VERSION 3.22.1)
project("fluxcore_mobile_probe")

# The parent build script sets FLUXCORE_ARCHIVE to the ABI-specific static
# archive and FLUXCORE_HEADER_DIR to the directory containing libfluxcore.h.
if(NOT DEFINED FLUXCORE_ARCHIVE)
    message(FATAL_ERROR "FLUXCORE_ARCHIVE must point to the prebuilt libfluxcore.a")
endif()
if(NOT DEFINED FLUXCORE_HEADER_DIR)
    message(FATAL_ERROR "FLUXCORE_HEADER_DIR must point to the C header directory")
endif()

add_library(fluxcore_mobile_probe SHARED jni-bridge.cpp)

target_include_directories(fluxcore_mobile_probe PRIVATE ${FLUXCORE_HEADER_DIR})

find_library(android android)
find_library(log log)

target_link_libraries(
    fluxcore_mobile_probe
    ${FLUXCORE_ARCHIVE}
    ${android}
    ${log}
)
