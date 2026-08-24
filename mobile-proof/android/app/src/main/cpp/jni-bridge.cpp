// Minimal JNI shim for the FluxBar mobile runtime proof.
//
// This file is compiled into libfluxcore_mobile_probe.so. It:
//   - converts Kotlin/Java strings into borrowed UTF-8 for FluxCoreRequest;
//   - invokes the existing two-symbol Rust C ABI (FluxCoreRequest / FluxCoreFree);
//   - copies the response back into a Java String and frees the Rust-owned pointer;
//   - forwards the Android Context to a feature-gated Rust helper that initializes
//     rustls-platform-verifier before any HTTPS-capable request.
//
// No raw Rust pointer crosses the Kotlin boundary.

#include <jni.h>
#include <cstdint>
#include <cstring>
#include <string>

// Declared by the Rust static archive. Caller provides a null-terminated UTF-8
// string; Rust returns a Rust-owned null-terminated UTF-8 response.
extern "C" char *FluxCoreRequest(char *request);
extern "C" void FluxCoreFree(char *value);

// Feature-gated Rust helper that calls rustls_platform_verifier::android::init_hosted.
// Returns 0 on success, non-zero on failure.
extern "C" int fluxbar_mobile_proof_init_android_verifier(void *env, void *context);

extern "C" JNIEXPORT jstring JNICALL
Java_com_fluxbar_mobileproof_FluxCoreBridge_request(JNIEnv *env, jclass /* clazz */,
                                                    jstring request) {
    if (request == nullptr) {
        // The Swift/Kotlin wrapper is contractually required to pass a non-null
        // string, but guard defensively and return the existing null-request error.
        const char *null_error = R"({"ok":false,"error":"null request"})";
        return env->NewStringUTF(null_error);
    }

    const char *request_utf8 = env->GetStringUTFChars(request, nullptr);
    if (request_utf8 == nullptr) {
        return env->NewStringUTF(R"({"ok":false,"error":"failed to read request string"})");
    }

    // FluxCoreRequest borrows the input for the duration of the call. We make a
    // mutable copy because the C ABI takes a non-const pointer for historical
    // compatibility with the Go core contract.
    size_t request_len = std::strlen(request_utf8);
    char *request_copy = static_cast<char *>(std::malloc(request_len + 1));
    if (request_copy == nullptr) {
        env->ReleaseStringUTFChars(request, request_utf8);
        return env->NewStringUTF(R"({"ok":false,"error":"out of memory copying request"})");
    }
    std::memcpy(request_copy, request_utf8, request_len + 1);
    env->ReleaseStringUTFChars(request, request_utf8);

    char *response = FluxCoreRequest(request_copy);
    std::free(request_copy);

    if (response == nullptr) {
        return env->NewStringUTF(R"({"ok":false,"error":"null response from core"})");
    }

    jstring result = env->NewStringUTF(response);
    FluxCoreFree(response);
    return result;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_fluxbar_mobileproof_FluxCoreBridge_initVerifierNative(JNIEnv *env, jclass /* clazz */,
                                                               jobject context) {
    if (context == nullptr) {
        jclass ex = env->FindClass("java/lang/IllegalArgumentException");
        if (ex != nullptr) {
            env->ThrowNew(ex, "context must not be null");
        }
        return JNI_FALSE;
    }

    int status = fluxbar_mobile_proof_init_android_verifier(env, context);
    return status == 0 ? JNI_TRUE : JNI_FALSE;
}
