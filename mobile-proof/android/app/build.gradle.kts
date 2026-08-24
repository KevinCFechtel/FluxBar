plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.fluxbar.mobileproof"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.fluxbar.mobileproof"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isMinifyEnabled = false
            isPseudoLocalesEnabled = false
        }
        create("releaseTest") {
            initWith(buildTypes.getByName("release"))
            signingConfig = signingConfigs.getByName("debug")
            matchingFallbacks += listOf("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    packaging {
        jniLibs {
            // Pick up the prebuilt libfluxcore_mobile_probe.so produced by
            // Build/build-rust-android.sh with CARGO_FEATURES=mobile-runtime-proof.
            // The path is relative to the app module directory.
            pickFirsts += setOf("libfluxcore_mobile_probe.so")
        }
    }

    sourceSets["main"].jniLibs.srcDir("${rootProject.projectDir}/../../.build/mobile/android/jniLibs")
}

// Allow the build to override the artifact directory via environment variable.
val fluxAndroidArtifacts = providers.environmentVariable("FLUX_ANDROID_ARTIFACTS")
    .orElse("${rootProject.projectDir}/../../.build/mobile/android")
    .get()

android.sourceSets["main"].jniLibs.srcDir("${fluxAndroidArtifacts}/jniLibs")

dependencies {
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.constraintlayout:constraintlayout:2.2.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")

    // Pinned rustls-platform-verifier Android component. The AAR is produced by
    // Build/build-rust-android.sh and copied from the locked cargo registry
    // package; it is not fetched from Maven Central with latest.release.
    implementation(files("${fluxAndroidArtifacts}/verifier/rustls-platform-verifier-0.1.1.aar"))

    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test:rules:1.6.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.1")
}
