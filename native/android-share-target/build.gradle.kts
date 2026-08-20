// Formal Typvia share-target library module. Included by the
// committed apps/mobile/src-tauri/gen/android/settings.gradle so the
// ACTION_SEND Activity ships inside the main app APK (same UID -> direct
// write into the app-private share inbox the host drains; iOS counterpart is
// native/ios-share-ext). Deliberately dependency-free at runtime: the module
// only renders a native card and lands one JSON file — no Rust bindings, no
// AndroidX. AGP/Kotlin plugin versions come from the including root
// project's buildscript classpath.

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.typvia.share"
    compileSdk = 36

    defaultConfig {
        // Product OS baseline: Android 9.0 / API 28, same as app.
        minSdk = 28
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    // Host-JVM unit tests for the pure file/JSON layer (ShareInbox).
    testImplementation("junit:junit:4.13.2")
}
