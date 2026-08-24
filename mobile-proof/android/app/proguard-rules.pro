# ProGuard/R8 rules for the FluxBar Android runtime proof host.

# rustls-platform-verifier calls into Kotlin via JNI. R8 cannot see JNI usage,
# so the verifier component must be kept explicitly.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }

# Keep the proof host bridge and Activity so R8 does not remove them.
-keep class com.fluxbar.mobileproof.** { *; }
