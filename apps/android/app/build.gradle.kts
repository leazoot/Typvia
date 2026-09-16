// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The main application. It assembles screens out of the design system and
// talks to the core through the store; it holds no rules of its own.

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val repoRoot: File = projectDir.parentFile.parentFile.parentFile

android {
    namespace = "dev.typvia.mobile"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.typvia.mobile"
        // Product OS baseline: Android 9.0 / API 28.
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }

    testOptions {
        unitTests.all {
            // Host-JVM tests exercise the real core through the host dylib —
            // the same loading path the FFI smoke uses. Without this they load
            // nothing and fail inside JNA rather than in the code under test.
            it.systemProperty(
                "jna.library.path",
                File(repoRoot, "target/release").absolutePath,
            )
        }
    }
}

dependencies {
    implementation(project(":core-ui"))
    implementation(project(":core-ffi"))
    // The keyboard ships inside this APK rather than beside it: sharing the
    // app's UID is what lets the input method read the snapshot at all.
    implementation(project(":ime"))
    // The share sheet ships here too: it writes into this app's data dir, so
    // it has to be this app.
    implementation(project(":share"))
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.core:core-ktx:1.15.0")
    testImplementation("junit:junit:4.13.2")
    // The plain jar, not the @aar: a host-JVM test loads the desktop dylib.
    testImplementation("net.java.dev.jna:jna:5.17.0")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
}
