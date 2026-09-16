// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Formal Typvia share-target library module. It ships inside the main app APK
// so the ACTION_SEND Activity shares the app's UID and can write straight into
// the app-private share inbox the host drains. Deliberately dependency-free at
// runtime: the module only renders a native card and lands one JSON file — no
// Rust bindings, no AndroidX. Like the IME module beside it, it has no
// including root project at the moment; the native Android app will bring one.

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
